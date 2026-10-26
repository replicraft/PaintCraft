//! Browser asset previews retain private pixels until an immediate download.
use super::*;
use paintcraft_formats::{
    icns, icons,
    registry::{self, Family, Operation, Platform, Request},
};
use paintcraft_ui::{icns_export as widget, icon_export as preview};
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct State {
    source: Arc<paintcraft_document::Document>,
    key: String,
    draft: widget::Draft,
    cache: preview::Cache<widget::Config, icns::Prepared>,
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
fn prepare(app: &WebApp, config: &widget::Config) -> Result<icns::Prepared, String> {
    if !app.text.preedit.is_empty() {
        return Err("text composition pending".into());
    }
    let doc = app
        .editor
        .copy_snapshot()
        .map_err(|error| error.to_string())?;
    let mut request = Request::new(
        registry::Frontend::Studio,
        Operation::SaveAsCopy,
        Family::Icns,
        Platform::Web,
    );
    request.extension = Some("icns");
    request.variant = Some("icon-fresh-entry-set");
    match registry::admission(request) {
        registry::Decision::Allowed
        | registry::Decision::AllowedWithPreflight(registry::Reason::ConvertRequiresConsent) => {}
        decision => decision
            .require_allowed(request)
            .map_err(|e| e.to_string())?,
    };
    let specs: Vec<_> = config
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
        .collect();
    icns::prepare_document(
        &doc,
        &specs,
        if config.filter == preview::Filter::Nearest {
            icons::Filter::Nearest
        } else {
            icons::Filter::Lanczos3
        },
        &|| Ok(()),
    )
    .map_err(|error| error.to_string())
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
impl WebApp {
    pub(super) fn begin_icns_export(&mut self) {
        if self.frontend != Frontend::Studio
            || self.blocked()
            || !self.floating_file_ready()
            || self.editor.tools.is_constructing_shape()
        {
            return;
        }
        self.editor.tools.cancel_gesture();
        self.icns_export = Some(State {
            source: self.editor.session.snapshot(),
            key: key(self),
            draft: Default::default(),
            cache: Default::default(),
        });
    }
    pub(super) fn icns_export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.icns_export.take() else {
            return;
        };
        if !state.current(self) {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        let response = egui::Modal::new(egui::Id::unique("web-icns-export"))
            .area(
                egui::Modal::default_area(egui::Id::unique("web-icns-export"))
                    .default_height((ctx.content_rect().height() - 80.0).clamp(160.0, 700.0)),
            )
            .show(ctx, |ui| {
                ui.set_width(480.0_f32.min((ctx.content_rect().width() - 48.0).max(160.0)));
                ui.heading(t!(ctx, "Apple icon export"));
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
                        let ready = config.valid()
                            && state.cache.show_bounded(
                                ui,
                                state.draft.selected,
                                1024,
                                "Export preview",
                                frame,
                            );
                        let consent = state.cache.value().is_some_and(|prepared| {
                            preview::consent(ui, prepared.losses(), &mut state.draft.accepted)
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
                            egui::Button::new(t!(ctx, "Download { $format }", format = "ICNS")),
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
                    let name = format!("{stem}.icns");
                    self.files.download_asset_as(
                        ctx,
                        &bytes,
                        &name,
                        "application/octet-stream",
                        self.editor.session.history_ticket().owner,
                    );
                }
                Err(error) => self.message = format!("Cannot export: {error}"),
            }
        } else {
            self.icns_export = Some(state);
        }
    }
}
#[cfg(test)]
#[path = "icns_exporting/tests.rs"]
mod tests;
