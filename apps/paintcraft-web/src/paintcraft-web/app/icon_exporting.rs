//! Browser asset previews retain private pixels until an immediate download.
use super::exporting::Frozen;
use super::*;
use paintcraft_formats::export::metadata_policy::{Intent, OMITTED_REASON, Policy, Target};
use paintcraft_formats::icons;
use paintcraft_ui::icon_export as widget;

#[derive(Debug)]
pub(super) struct State {
    frozen: Frozen,
    config: widget::Config,
    omit_metadata: bool,
    bake_filters: bool,
    policy: Option<paintcraft_formats::export::CopyPolicies>,
    other_losses: Vec<String>,
    draft: widget::Draft,
    cache: widget::Cache<(widget::Config, bool, bool), icons::Prepared>,
}
impl State {
    fn current(&self, app: &WebApp) -> bool {
        app.frontend == Frontend::Studio && self.frozen.current(app)
    }
    fn target(&self) -> Target {
        Target::new(
            if self.draft.config.kind == widget::Kind::Icon {
                paintcraft_formats::registry::Family::Ico
            } else {
                paintcraft_formats::registry::Family::Cur
            },
            paintcraft_formats::registry::Operation::ExportAsset,
        )
    }
    fn retire(&mut self) {
        self.cache = Default::default();
        self.policy = None;
        self.other_losses.clear();
        self.draft.accepted = false;
    }
    fn authorized(&self) -> bool {
        if (self.frozen.document.source_metadata.is_some() && !self.omit_metadata)
            || (self.frozen.has_filters && !self.bake_filters)
        {
            return false;
        }
        self.policy.is_some_and(|policy| {
            matches!(
                policy.filter_bake,
                paintcraft_formats::export::filter_bake::Policy::Bake(_)
            ) == self.bake_filters
                && matches!(policy.source_metadata, Policy::Omit(_)) == self.omit_metadata
        })
    }
    // Only at publication: inventory hashing is never part of per-frame readiness.
    fn policies_current(&self) -> bool {
        self.authorized()
            && self.policy.is_some_and(|policy| {
                policy
                    .source_metadata
                    .validate(&self.frozen.document, self.target())
                    .is_ok()
                    && policy
                        .filter_bake
                        .validate(&self.frozen.document, self.target(), &|| Ok(()))
                        .is_ok()
            })
    }
}
fn prepare(
    doc: &paintcraft_document::Document,
    config: &widget::Config,
    policy: paintcraft_formats::export::CopyPolicies,
) -> Result<icons::Prepared, String> {
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
    icons::prepare_document_with_policies(
        doc,
        kind,
        &specs,
        if config.filter == widget::Filter::Nearest {
            icons::Filter::Nearest
        } else {
            icons::Filter::Lanczos3
        },
        icons::WriteLimits::default(),
        &|| Ok(()),
        policy,
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
        let frozen = match Frozen::capture(self) {
            Ok(frozen) => frozen,
            Err(error) => {
                self.message = error.to_string();
                return;
            }
        };
        let draft: widget::Draft = Default::default();
        self.icon_export = Some(State {
            frozen,
            config: draft.config.clone(),
            omit_metadata: false,
            bake_filters: false,
            policy: None,
            other_losses: Vec::new(),
            draft,
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
                        if state.config != config {
                            state.config = config.clone();
                            state.omit_metadata = false;
                            state.bake_filters = false;
                            state.retire();
                        }
                        let has_metadata = state.frozen.document.source_metadata.is_some();
                        if has_metadata {
                            paintcraft_ui::source_metadata_export::controls(
                                ui,
                                &mut state.omit_metadata,
                            );
                        }
                        if state.frozen.has_filters {
                            paintcraft_ui::filter_bake_export::controls(ui, &mut state.bake_filters);
                        }
                        if (has_metadata && !state.omit_metadata) || (state.frozen.has_filters && !state.bake_filters) {
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
                        let bake_filters = state.bake_filters;
                        if state
                            .cache
                            .update(ctx, (config.clone(), state.omit_metadata, state.bake_filters), || {
                                let captured = Policy::capture(document, target, intent)
                                    .map_err(|error| error.to_string())?;
                                let filter_bake = paintcraft_formats::export::filter_bake::Policy::capture(document,target,if bake_filters {paintcraft_formats::export::filter_bake::Intent::Bake} else {paintcraft_formats::export::filter_bake::Intent::Refuse},&||Ok(())).map_err(|e|e.to_string())?;
                                let captured = paintcraft_formats::export::CopyPolicies{source_metadata:captured,filter_bake};
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
                                        .filter(|loss| loss.as_str() != OMITTED_REASON && loss.as_str() != paintcraft_formats::export::filter_bake::BAKED_REASON)
                                        .cloned()
                                        .collect()
                                });
                        }
                        let ready = state.cache.show(ui, state.draft.selected, frame);
                        let consent = state.cache.value().is_some()
                            && widget::consent(ui, &state.other_losses, &mut state.draft.accepted);
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
            if !state.current(self) || !state.policies_current() {
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

#[cfg(test)]
#[path = "icon_exporting/metadata_tests.rs"]
mod metadata_tests;
