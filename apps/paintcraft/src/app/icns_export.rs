//! Studio asset export retains private previews through a parented native picker.
use super::*;
use export_options::{AssetCopy, other_losses};
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::{
    icns, icons,
    registry::{self, Family, Operation, Platform, Request},
};
use paintcraft_ui::{icns_export as widget, icon_export as preview, t};

pub(super) struct State {
    owner: Uuid,
    source: Arc<paintcraft_document::Document>,
    stamp: controller::PreviewStamp,
    copy: AssetCopy<widget::Config>,
    draft: widget::Draft,
    cache: preview::Cache<(widget::Config, bool), icns::Prepared>,
    bytes: Option<Vec<u8>>,
    destination: Option<(PathBuf, Option<save::Expectation>)>,
}
impl State {
    fn current(&self, app: &App) -> bool {
        app.experience() == Experience::Studio
            && app.active_tab_id() == self.owner
            && Arc::ptr_eq(&self.source, &app.editor.document_snapshot())
            && self.stamp == app.editor.preview_stamp()
            && self.copy.current(app)
    }
    fn target(&self) -> export::metadata_policy::Target {
        export::metadata_policy::Target::new(Family::Icns, Operation::ExportAsset)
    }
    fn prepare(&mut self, ctx: &egui::Context) {
        if !self.copy.ready() {
            self.copy.retire();
            self.cache = Default::default();
            self.bytes = None;
            self.draft.accepted = false;
            return;
        }
        let config = self.draft.config.clone();
        let target = self.target();
        if self
            .cache
            .update(ctx, (config.clone(), self.copy.omit), || {
                let policy = self
                    .copy
                    .prepare(config.clone(), target)
                    .map_err(|e| e.to_string())?;
                icns::prepare_document_with_metadata_policy(
                    &self.copy.frozen.document,
                    &specs(&config),
                    if config.filter == preview::Filter::Nearest {
                        icons::Filter::Nearest
                    } else {
                        icons::Filter::Lanczos3
                    },
                    &|| Ok(()),
                    policy,
                )
                .map_err(|e| e.to_string())
            })
        {
            self.draft.accepted = false;
            self.bytes = None;
        }
    }
}
fn specs(config: &widget::Config) -> Vec<icns::IconType> {
    config
        .kinds
        .iter()
        .map(|kind| match kind {
            widget::Kind::Size128 => icns::IconType::Size128,
            widget::Kind::Size256 => icns::IconType::Size256,
            widget::Kind::Size512 => icns::IconType::Size512,
            widget::Kind::Retina16 => icns::IconType::Retina16,
            widget::Kind::Retina32 => icns::IconType::Retina32,
            widget::Kind::Retina128 => icns::IconType::Retina128,
            widget::Kind::Retina256 => icns::IconType::Retina256,
            widget::Kind::Retina512 => icns::IconType::Retina512,
        })
        .collect()
}
fn frame(prepared: &icns::Prepared, index: usize) -> Option<preview::Frame<'_>> {
    let entry = prepared.entries().get(index)?;
    Some(preview::Frame {
        width: entry.kind.pixel_size(),
        height: entry.kind.pixel_size(),
        rgba: entry.rgba(),
        hotspot: None,
    })
}

impl App {
    pub(super) fn begin_icns_export(&mut self) {
        if self.experience() != Experience::Studio
            || self.document_count() == 0
            || !self.editor.shape_file_ready()
            || self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
        {
            return;
        }
        self.editor.cancel_gesture();
        self.editor.run_pending();
        if !self.editor.can_capture_export() {
            if self.editor.failed_text_retained() {
                return;
            }
            self.defer_file(
                editing::FileRequest::ExportIcns,
                token(
                    &self.context,
                    paintcraft_ui::actions::Action::ExportIcns.activation_code(),
                ),
            );
            return;
        }
        self.capture_icns_export();
    }
    pub(super) fn capture_icns_export(&mut self) {
        let copy = match AssetCopy::capture(self) {
            Ok(copy) => copy,
            Err(error) => {
                self.editor.set_status(format!("Cannot export: {error}"));
                return;
            }
        };
        self.icns_export = Some(State {
            owner: self.active_tab_id(),
            source: self.editor.document_snapshot(),
            stamp: self.editor.preview_stamp(),
            copy,
            draft: Default::default(),
            cache: Default::default(),
            bytes: None,
            destination: None,
        });
        self.confirm = Some(Confirm::IcnsExport);
    }
    pub(super) fn icns_export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.icns_export.take() else {
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
            "native-icns-export",
            state.destination.is_some(),
        )))
        .area(
            egui::Modal::default_area(egui::Id::unique((
                "native-icns-export",
                state.destination.is_some(),
            )))
            .default_height((ctx.content_rect().height() - 80.0).clamp(160.0, 700.0)),
        )
        .show(ctx, |ui| {
            ui.set_width(480.0_f32.min((ctx.content_rect().width() - 48.0).max(160.0)));
            ui.heading(t!(ctx, "Apple icon export"));
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
                .max_height((ctx.content_rect().height() - 150.0).clamp(160.0, 620.0))
                .show(ui, |ui| {
                    widget::controls(ui, &mut state.draft);
                    state.copy.controls(ui);
                    let config = state.draft.config.clone();
                    state.prepare(ctx);
                    let ready = config.valid()
                        && state.cache.show_bounded(
                            ui,
                            state.draft.selected,
                            1024,
                            "Export preview",
                            frame,
                        );
                    let consent = state.cache.value().is_some_and(|prepared| {
                        preview::consent(
                            ui,
                            &other_losses(prepared.losses()),
                            &mut state.draft.accepted,
                        )
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
        self.icns_export = Some(state);
        if response.inner.1 {
            if review {
                self.finish_icns_export(ctx);
            } else {
                self.choose_icns_destination(ctx);
            }
        }
    }
    pub(super) fn choose_icns_destination(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.icns_export.take() else {
            return;
        };
        if !state.current(self) {
            self.confirm = None;
            return;
        }
        if let Err(error) = state.copy.validate(&state.draft.config, state.target()) {
            self.editor.set_status(format!("Cannot export: {error}"));
            self.icns_export = Some(state);
            return;
        }
        let Some(prepared) = state.cache.value() else {
            self.icns_export = Some(state);
            return;
        };
        let losses = other_losses(prepared.losses());
        if !losses.is_empty() && !state.draft.accepted {
            self.icns_export = Some(state);
            return;
        }
        match prepared.encode(&losses, &|| Ok(())) {
            Ok(bytes) => state.bytes = Some(bytes),
            Err(error) => {
                self.editor.set_status(format!("Cannot export: {error}"));
                self.icns_export = Some(state);
                return;
            }
        }
        let extension = "icns";
        let suggested = Path::new(&self.path_text).with_extension(extension);
        self.icns_export = Some(state);
        self.confirm = None;
        if let Some(dialogs) = &mut self.dialogs {
            let wake = ctx.clone();
            dialogs.request_with_filters(
                Kind::ExportIcns,
                Some(&suggested),
                &[paintcraft_platform::dialogs::Filter {
                    name: "Apple icon export".into(),
                    extensions: vec![extension.into()],
                }],
                |source| t!(ctx, source),
                move || wake.request_repaint(),
            );
        } else {
            self.complete_icns_picker(ctx, Some(suggested));
        }
    }
    pub(super) fn complete_icns_picker(&mut self, _ctx: &egui::Context, path: Option<PathBuf>) {
        let Some(mut state) = self.icns_export.take() else {
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
            state.copy.validate(&state.draft.config, state.target())?;
            self.guard_export_destination(&path)?;
            let mut request = Request::new(
                self.format_frontend(),
                Operation::SaveAsCopy,
                Family::Icns,
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
            match registry::admission(request) {
                registry::Decision::Allowed
                | registry::Decision::AllowedWithPreflight(
                    registry::Reason::ConvertRequiresConsent,
                ) => {}
                decision => decision.require_allowed(request)?,
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
                self.icns_export = Some(state);
                self.confirm = Some(Confirm::IcnsExport);
            }
            Err(error) => self.editor.set_status(format!("Cannot export: {error}")),
        }
    }
    pub(super) fn finish_icns_export(&mut self, ctx: &egui::Context) {
        let Some(state) = self.icns_export.take() else {
            return;
        };
        self.confirm = None;
        if !state.current(self) {
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let target = state.target();
        if let Err(error) = state.copy.validate(&state.draft.config, target) {
            self.editor.set_status(format!("Cannot export: {error}"));
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
            icns::BYTES_MAX as u64,
            expected.as_ref(),
            |sink| {
                std::io::Write::write_all(sink, &bytes)
                    .map_err(|_| Error::new(ErrorCode::StorageFailed, "cannot stage icon output"))
            },
            |outcome| {
                state.copy.validate(&state.draft.config, target)?;
                self.guard_export_destination(&path)?;
                if let Some(resolved) = outcome.resolved_destination() {
                    self.guard_export_destination(resolved)?;
                }
                Ok(())
            },
        );
        self.editor.set_status(if outcome.is_success() {
            format!("Exported {}", path.display())
        } else {
            format!("Cannot export: {}", outcome.reasons.join("; "))
        });
    }
}
#[cfg(test)]
#[path = "icns_export/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "icns_export/metadata_tests.rs"]
mod metadata_tests;
