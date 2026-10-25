//! Studio asset export retains private previews through a parented native picker.
use super::*;
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::{
    icons,
    registry::{self, Family, Operation, Platform, Request},
};
use paintcraft_ui::{icon_export as widget, t};

pub(super) struct State {
    owner: Uuid,
    source: Arc<paintcraft_document::Document>,
    stamp: controller::PreviewStamp,
    draft: widget::Draft,
    cache: widget::Cache<widget::Config, icons::Prepared>,
    bytes: Option<Vec<u8>>,
    destination: Option<(PathBuf, Option<save::Expectation>)>,
}
impl State {
    fn current(&self, app: &App) -> bool {
        app.experience() == Experience::Studio
            && app.active_tab_id() == self.owner
            && Arc::ptr_eq(&self.source, &app.editor.document_snapshot())
            && self.stamp == app.editor.preview_stamp()
    }
}
fn kind(config: &widget::Config) -> icons::Kind {
    if config.kind == widget::Kind::Icon {
        icons::Kind::Icon
    } else {
        icons::Kind::Cursor
    }
}
fn specs(config: &widget::Config) -> Vec<icons::EntrySpec> {
    config
        .entries
        .iter()
        .map(|entry| icons::EntrySpec {
            width: u32::from(entry.width),
            height: u32::from(entry.height),
            encoding: if entry.encoding == widget::Encoding::Png {
                icons::Encoding::Png
            } else {
                icons::Encoding::Bitmap
            },
            hotspot: (config.kind == widget::Kind::Cursor).then_some(entry.hotspot),
        })
        .collect()
}
fn frame(prepared: &icons::Prepared, index: usize) -> Option<widget::Frame<'_>> {
    let entry = prepared.entries().get(index)?;
    Some(widget::Frame {
        width: entry.spec.width,
        height: entry.spec.height,
        rgba: entry.rgba(),
        hotspot: entry.spec.hotspot,
    })
}

impl App {
    pub(super) fn begin_icon_export(&mut self) {
        if self.experience() != Experience::Studio
            || self.document_count() == 0
            || !self.editor.shape_file_ready()
            || self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
        {
            return;
        }
        self.editor.cancel_gesture();
        self.editor.run_pending();
        if self.editor.has_pending_edits() {
            self.defer_file(
                editing::FileRequest::ExportIcons,
                token(
                    &self.context,
                    paintcraft_ui::actions::Action::ExportIcons.activation_code(),
                ),
            );
            return;
        }
        self.capture_icon_export();
    }
    pub(super) fn capture_icon_export(&mut self) {
        self.icon_export = Some(State {
            owner: self.active_tab_id(),
            source: self.editor.document_snapshot(),
            stamp: self.editor.preview_stamp(),
            draft: Default::default(),
            cache: Default::default(),
            bytes: None,
            destination: None,
        });
        self.confirm = Some(Confirm::IconExport);
    }
    pub(super) fn icon_export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.icon_export.take() else {
            self.confirm = None;
            return;
        };
        if !state.current(self) {
            self.confirm = None;
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let response = egui::Modal::new(egui::Id::unique((
            "native-icon-export",
            state.destination.is_some(),
        )))
        .area(
            egui::Modal::default_area(egui::Id::unique((
                "native-icon-export",
                state.destination.is_some(),
            )))
            .default_height((ctx.content_rect().height() - 80.0).max(160.0)),
        )
        .show(ctx, |ui| {
            ui.set_width(480.0_f32.min((ctx.content_rect().width() - 48.0).max(160.0)));
            ui.heading(t!(ctx, "Icon and cursor export"));
            if let Some((path, expected)) = &state.destination {
                ui.label(path.to_string_lossy());
                if expected.is_some() {
                    ui.label(t!(ctx, "The file exists and will be replaced."));
                }
                return ui
                    .horizontal(|ui| {
                        (
                            ui.button(t!(ctx, "Cancel")).clicked(),
                            ui.button(t!(ctx, "Export now")).clicked(),
                        )
                    })
                    .inner;
            }
            let eligible = egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height((ctx.content_rect().height() - 150.0).max(160.0))
                .show(ui, |ui| {
                    widget::controls(ui, &mut state.draft);
                    let config = state.draft.config.clone();
                    if state.cache.update(ctx, config.clone(), || {
                        let doc = self
                            .editor
                            .export_snapshot()
                            .map_err(|error| error.to_string())?;
                        icons::prepare_document(
                            &doc,
                            kind(&config),
                            &specs(&config),
                            if config.filter == widget::Filter::Nearest {
                                icons::Filter::Nearest
                            } else {
                                icons::Filter::Lanczos3
                            },
                            icons::WriteLimits::default(),
                            &|| Ok(()),
                        )
                        .map_err(|error| error.to_string())
                    }) {
                        state.draft.accepted = false;
                    }
                    let ready = state.cache.show(ui, state.draft.selected, frame);
                    let consent = state.cache.value().is_some_and(|prepared| {
                        widget::consent(ui, prepared.losses(), &mut state.draft.accepted)
                    });
                    ui.label(t!(
                        ctx,
                        "Save a PaintCraft document to keep editable layers."
                    ));
                    ready && consent
                })
                .inner;
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                (
                    ui.button(t!(ctx, "Cancel")).clicked(),
                    ui.add_enabled(eligible, egui::Button::new(t!(ctx, "Choose destination…")))
                        .clicked(),
                )
            })
            .inner
        });
        if response.inner.0 || response.should_close() {
            self.confirm = None;
            return;
        }
        let review = state.destination.is_some();
        self.icon_export = Some(state);
        if response.inner.1 {
            if review {
                self.finish_icon_export(ctx);
            } else {
                self.choose_icon_destination(ctx);
            }
        }
    }
    pub(super) fn choose_icon_destination(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.icon_export.take() else {
            return;
        };
        if !state.current(self) {
            self.confirm = None;
            return;
        }
        let Some(prepared) = state.cache.value() else {
            self.icon_export = Some(state);
            return;
        };
        if !prepared.losses().is_empty() && !state.draft.accepted {
            self.icon_export = Some(state);
            return;
        }
        match prepared.encode(prepared.losses(), &|| Ok(())) {
            Ok(bytes) => state.bytes = Some(bytes),
            Err(error) => {
                self.editor.set_status(format!("Cannot export: {error}"));
                self.icon_export = Some(state);
                return;
            }
        }
        let extension = state.draft.config.kind.extension();
        let suggested = Path::new(&self.path_text).with_extension(extension);
        self.icon_export = Some(state);
        self.confirm = None;
        if let Some(dialogs) = &mut self.dialogs {
            let wake = ctx.clone();
            dialogs.request_with_filters(
                Kind::ExportIcons,
                Some(&suggested),
                &[paintcraft_platform::dialogs::Filter {
                    name: "Icon and cursor export".into(),
                    extensions: vec![extension.into()],
                }],
                |source| t!(ctx, source),
                move || wake.request_repaint(),
            );
        } else {
            self.complete_icon_picker(ctx, Some(suggested));
        }
    }
    pub(super) fn complete_icon_picker(&mut self, _ctx: &egui::Context, path: Option<PathBuf>) {
        let Some(mut state) = self.icon_export.take() else {
            return;
        };
        self.confirm = None;
        let Some(path) = path else {
            return;
        };
        let result = (|| {
            if !state.current(self) {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "document changed during icon destination selection",
                ));
            }
            self.guard_export_destination(&path)?;
            let family = if state.draft.config.kind == widget::Kind::Icon {
                Family::Ico
            } else {
                Family::Cur
            };
            let mut request = Request::new(
                self.format_frontend(),
                Operation::SaveAsCopy,
                family,
                Platform::Native,
            );
            request.extension = path.extension().and_then(|value| value.to_str());
            if request.extension.is_none() {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "icon output requires the selected file extension",
                ));
            }
            request.variant = Some("icon-fresh-entry-set");
            let decision = registry::admission(request);
            if !decision.available() {
                decision.require_allowed(request)?;
            }
            let expected = match std::fs::symlink_metadata(&path) {
                Ok(_) => {
                    let resolved = std::fs::canonicalize(&path).map_err(|_| {
                        Error::new(ErrorCode::StorageFailed, "cannot resolve icon destination")
                    })?;
                    let bytes = crate::format_admission::read_source(&resolved)?;
                    Some(save::Expectation::acquired(&resolved, &bytes))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(_) => {
                    return Err(Error::new(
                        ErrorCode::StorageFailed,
                        "cannot inspect icon destination",
                    ));
                }
            };
            Ok(expected)
        })();
        match result {
            Ok(expected) => {
                state.destination = Some((path, expected));
                self.icon_export = Some(state);
                self.confirm = Some(Confirm::IconExport);
            }
            Err(error) => self.editor.set_status(format!("Cannot export: {error}")),
        }
    }
    pub(super) fn finish_icon_export(&mut self, ctx: &egui::Context) {
        let Some(state) = self.icon_export.take() else {
            return;
        };
        self.confirm = None;
        if !state.current(self) {
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let Some((path, expected)) = state.destination else {
            return;
        };
        if let Err(error) = self.guard_export_destination(&path) {
            self.editor.set_status(format!("Cannot export: {error}"));
            return;
        }
        let Some(bytes) = state.bytes else {
            return;
        };
        let outcome = save::save_with_expectation_and_preparation(
            &path,
            expected.is_some(),
            icons::ENCODED_BYTES_MAX as u64,
            expected.as_ref(),
            |sink| {
                std::io::Write::write_all(sink, &bytes)
                    .map_err(|_| Error::new(ErrorCode::StorageFailed, "cannot stage icon output"))
            },
            |_| Ok(()),
        );
        self.editor.set_status(if outcome.is_success() {
            format!("Exported {}", path.display())
        } else {
            format!("Not exported safely: {}", outcome.reasons.join("; "))
        });
    }
}

#[cfg(test)]
#[path = "icon_export/tests.rs"]
mod tests;
