//! Browser asset previews retain private pixels until an immediate download.
use super::exporting::Frozen;
use super::*;
use paintcraft_formats::export::metadata_policy::{Intent, OMITTED_REASON, Policy, Target};
use paintcraft_formats::{
    icns, icons,
    registry::{self, Family, Operation, Platform, Request},
};
use paintcraft_ui::{icns_export as widget, icon_export as preview};

#[derive(Debug)]
pub(super) struct State {
    frozen: Frozen,
    config: widget::Config,
    omit_metadata: bool,
    policy: Option<Policy>,
    other_losses: Vec<String>,
    draft: widget::Draft,
    cache: preview::Cache<(widget::Config, bool), icns::Prepared>,
}
impl State {
    fn current(&self, app: &WebApp) -> bool {
        app.frontend == Frontend::Studio && self.frozen.current(app)
    }
    fn target(&self) -> Target {
        Target::new(Family::Icns, Operation::ExportAsset)
    }
    fn retire(&mut self) {
        self.cache = Default::default();
        self.policy = None;
        self.other_losses.clear();
        self.draft.accepted = false;
    }
    fn authorized(&self) -> bool {
        if self.frozen.document.source_metadata.is_some() && !self.omit_metadata {
            return false;
        }
        self.policy.is_some_and(|policy| {
            policy
                .validate(&self.frozen.document, self.target())
                .is_ok()
        })
    }
}
fn prepare(
    doc: &paintcraft_document::Document,
    config: &widget::Config,
    policy: Policy,
) -> Result<icns::Prepared, String> {
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
    icns::prepare_document_with_metadata_policy(
        doc,
        &specs,
        if config.filter == preview::Filter::Nearest {
            icons::Filter::Nearest
        } else {
            icons::Filter::Lanczos3
        },
        &|| Ok(()),
        policy,
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
        let frozen = match Frozen::capture(self) {
            Ok(frozen) => frozen,
            Err(error) => {
                self.message = error.to_string();
                return;
            }
        };
        let draft: widget::Draft = Default::default();
        self.icns_export = Some(State {
            frozen,
            config: draft.config.clone(),
            omit_metadata: false,
            policy: None,
            other_losses: Vec::new(),
            draft,
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
                        if state.config != config {
                            state.config = config.clone();
                            state.omit_metadata = false;
                            state.retire();
                        }
                        let has_metadata = state.frozen.document.source_metadata.is_some();
                        if has_metadata {
                            paintcraft_ui::source_metadata_export::controls(
                                ui,
                                &mut state.omit_metadata,
                            );
                        }
                        if has_metadata && !state.omit_metadata {
                            state.retire();
                            return false;
                        }
                        let target = state.target();
                        let intent = if has_metadata {
                            Intent::Omit
                        } else {
                            Intent::Refuse
                        };
                        let document = &state.frozen.document;
                        let policy = &mut state.policy;
                        if state
                            .cache
                            .update(ctx, (config.clone(), state.omit_metadata), || {
                                let captured = Policy::capture(document, target, intent)
                                    .map_err(|error| error.to_string())?;
                                let prepared = prepare(document, &config, captured)?;
                                *policy = Some(captured);
                                Ok(prepared)
                            })
                        {
                            state.draft.accepted = false;
                            state.other_losses =
                                state.cache.value().map_or_else(Vec::new, |prepared| {
                                    prepared
                                        .losses()
                                        .iter()
                                        .filter(|loss| loss.as_str() != OMITTED_REASON)
                                        .cloned()
                                        .collect()
                                });
                        }
                        let ready = config.valid()
                            && state.cache.show_bounded(
                                ui,
                                state.draft.selected,
                                1024,
                                "Export preview",
                                frame,
                            );
                        let consent = state.cache.value().is_some()
                            && preview::consent(ui, &state.other_losses, &mut state.draft.accepted);
                        ui.label(t!(
                            ctx,
                            "Save a PaintCraft document to keep editable layers."
                        ));
                        ready && consent && state.authorized()
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
            if !state.current(self) || !state.authorized() {
                return;
            }
            let Some(prepared) = state.cache.value() else {
                return;
            };
            if !state.draft.accepted && !state.other_losses.is_empty() {
                return;
            }
            match prepared.encode(&state.other_losses, &|| Ok(())) {
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

#[cfg(test)]
#[path = "icns_exporting/metadata_tests.rs"]
mod metadata_tests;
