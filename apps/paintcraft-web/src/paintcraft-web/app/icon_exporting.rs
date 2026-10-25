//! Browser asset previews retain private pixels until an immediate download.
use super::*;
use paintcraft_formats::icons;
use paintcraft_ui::icon_export as widget;
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct State {
    source: Arc<paintcraft_document::Document>,
    key: String,
    draft: widget::Draft,
    cache: widget::Cache<widget::Config, icons::Prepared>,
}
fn key(app: &WebApp) -> String {
    format!(
        "{:?}/{:?}/{:?}/{}",
        app.editor.session.history_ticket(),
        app.editor.tools.preview_revision(),
        app.editor.tools.pending_text_action(),
        app.text.preedit
    )
}
impl State {
    fn current(&self, app: &WebApp) -> bool {
        app.frontend == Frontend::Studio
            && Arc::ptr_eq(&self.source, &app.editor.session.snapshot())
            && self.key == key(app)
    }
}
fn prepare(app: &WebApp, config: &widget::Config) -> Result<icons::Prepared, String> {
    if !app.text.preedit.is_empty() {
        return Err("text composition pending".into());
    }
    let doc = app
        .editor
        .copy_snapshot()
        .map_err(|error| error.to_string())?;
    let kind = if config.kind == widget::Kind::Icon {
        icons::Kind::Icon
    } else {
        icons::Kind::Cursor
    };
    let specs: Vec<_> = config
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
        .collect();
    icons::prepare_document(
        &doc,
        kind,
        &specs,
        if config.filter == widget::Filter::Nearest {
            icons::Filter::Nearest
        } else {
            icons::Filter::Lanczos3
        },
        icons::WriteLimits::default(),
        &|| Ok(()),
    )
    .map_err(|error| error.to_string())
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
impl WebApp {
    pub(super) fn begin_icon_export(&mut self) {
        if self.frontend != Frontend::Studio
            || self.blocked()
            || !self.floating_file_ready()
            || self.editor.tools.is_constructing_shape()
        {
            return;
        }
        self.editor.tools.cancel_gesture();
        self.icon_export = Some(State {
            source: self.editor.session.snapshot(),
            key: key(self),
            draft: Default::default(),
            cache: Default::default(),
        });
    }
    pub(super) fn icon_export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.icon_export.take() else {
            return;
        };
        if !state.current(self) {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        let response = egui::Modal::new(egui::Id::unique("web-icon-export"))
            .area(
                egui::Modal::default_area(egui::Id::unique("web-icon-export"))
                    .default_height((ctx.content_rect().height() - 80.0).clamp(160.0, 700.0)),
            )
            .show(ctx, |ui| {
                ui.set_width(480.0_f32.min((ctx.content_rect().width() - 48.0).max(160.0)));
                ui.heading(t!(ctx, "Icon and cursor export"));
                let eligible = egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height((ctx.content_rect().height() - 150.0).clamp(160.0, 620.0))
                    .show(ui, |ui| {
                        widget::controls(ui, &mut state.draft);
                        let config = state.draft.config.clone();
                        if state
                            .cache
                            .update(ctx, config.clone(), || prepare(self, &config))
                        {
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
                        ui.add_enabled(
                            eligible,
                            egui::Button::new(t!(
                                ctx,
                                "Download { $format }",
                                format = state.draft.config.kind.name()
                            )),
                        )
                        .clicked(),
                    )
                })
                .inner
            });
        if response.inner.0 || response.should_close() {
            return;
        }
        if response.inner.1 {
            let Some(prepared) = state.cache.value() else {
                return;
            };
            if !state.draft.accepted && !prepared.losses().is_empty() {
                return;
            }
            match prepared.encode(prepared.losses(), &|| Ok(())) {
                Ok(bytes) => {
                    let project = self.download_name(None);
                    let stem = project.strip_suffix(".pcnd").unwrap_or(&project);
                    let name = format!("{stem}.{}", state.draft.config.kind.extension());
                    self.files.download_asset(
                        ctx,
                        &bytes,
                        &name,
                        self.editor.session.history_ticket().owner,
                    );
                }
                Err(error) => self.message = format!("Cannot export: {error}"),
            }
        } else {
            self.icon_export = Some(state);
        }
    }
}

#[cfg(test)]
#[path = "icon_exporting/tests.rs"]
mod tests;
