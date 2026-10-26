//! Numeric placement is a distinct conversion review, owned by its original doc.
use super::*;
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{BudgetAccount, SharedBudget},
};
use paintcraft_formats::csh::placement::{self, Policy, Preview, Rasterized};
use paintcraft_ui::{
    shape_placement::{self, Config, Limits, Response},
    t,
};
#[derive(Debug)]
struct Prepared {
    raster: Rasterized,
    _preview: Preview,
    _copy: BudgetAccount,
}
#[derive(Debug)]
pub(crate) struct Draft {
    source: Arc<Loaded>,
    index: usize,
    resource_epoch: Uuid,
    owner: Uuid,
    base: Arc<paintcraft_document::Document>,
    ticket: (Uuid, u64),
    target: Option<Uuid>,
    budget: SharedBudget,
    state: shape_placement::State<Prepared>,
}
fn limits() -> Limits {
    Limits {
        side: placement::SIDE_MAX,
        pixels: placement::PIXELS_MAX,
    }
}
impl Draft {
    fn current(&self, app: &App) -> bool {
        app.experience() == Experience::Studio
            && self.resource_epoch == app.shapes.epoch
            && self.index == app.shapes.selected
            && app
                .shapes
                .loaded
                .as_ref()
                .is_some_and(|loaded| Arc::ptr_eq(loaded, &self.source))
            && app.active_tab_id() == self.owner
            && app.editor.render_key() == self.ticket
            && app.editor.target() == self.target
            && Arc::ptr_eq(&self.base, &app.editor.document_snapshot())
            && !app.editor.has_pending_edits()
            && !app.editor.gesture_in_progress()
            && !app.editor.is_editing_text()
    }
    pub(crate) fn source_path(&self) -> &Path {
        &self.source.source
    }
    pub(crate) fn source_identity(&self) -> &crate::destination::SourceIdentity {
        &self.source.identity
    }
}
impl App {
    pub(crate) fn begin_shape_placement(&mut self) -> Result<()> {
        if self.experience() != Experience::Studio
            || self.editor.has_pending_edits()
            || self.editor.gesture_in_progress()
            || self.editor.is_editing_text()
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Finish the current editing operation before placing a shape",
            ));
        }
        let source = self
            .shapes
            .loaded
            .as_ref()
            .cloned()
            .ok_or_else(|| Error::new(ErrorCode::InputSchema, "No shape resource loaded"))?;
        let index = self.shapes.selected;
        let shape = source
            .library
            .shapes()
            .get(index)
            .ok_or_else(|| Error::new(ErrorCode::InputSchema, "Invalid shape selection"))?;
        if !placement::assess(shape, &|| Ok(()))?.qualified() {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "This shape cannot be placed",
            ));
        }
        self.editor.shape_target()?;
        let width = (shape.bounds[3] - shape.bounds[1]).max(1);
        let height = (shape.bounds[2] - shape.bounds[0]).max(1);
        let factor = (placement::SIDE_MAX as f64 / width as f64)
            .min(placement::SIDE_MAX as f64 / height as f64)
            .min((placement::PIXELS_MAX as f64 / (width as f64 * height as f64)).sqrt())
            .min(1.0);
        let config = Config {
            at: (0, 0),
            width: ((width as f64 * factor).floor() as u32).max(1),
            height: ((height as f64 * factor).floor() as u32).max(1),
            color: self.editor.color,
        };
        self.shapes.picker = None;
        self.shapes.epoch = Uuid::new_v4();
        self.shapes.placement = Some(Draft {
            source,
            index,
            resource_epoch: self.shapes.epoch,
            owner: self.active_tab_id(),
            base: self.editor.document_snapshot(),
            ticket: self.editor.render_key(),
            target: self.editor.target(),
            budget: SharedBudget::new(96 << 20),
            state: shape_placement::State::new(config),
        });
        Ok(())
    }
    pub(crate) fn shape_placement_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.shapes.placement.take() else {
            return;
        };
        if !draft.current(self) {
            self.shapes.error = Some(t!(ctx, "The document changed; choose the shape again."));
            return;
        }
        let name = draft.source.library.shapes()[draft.index].name.clone();
        match draft.state.dialog(ctx, &name, limits()) {
            Response::Pending => {}
            Response::Cancel => return,
            Response::Preview(config) => {
                let result = (|| {
                    let raster = placement::rasterize(
                        draft.source.library.clone(),
                        draft.index,
                        config.width,
                        config.height,
                        Policy::CubicFillV1,
                        &draft.budget,
                        &|| Ok(()),
                    )?;
                    let preview = raster.rgba(config.color, &draft.budget, &|| Ok(()))?;
                    let copy = preview.declare_copy(&draft.budget)?;
                    let prepared = Prepared {
                        raster,
                        _preview: preview,
                        _copy: copy,
                    };
                    if !draft
                        .state
                        .install(ctx, config, prepared, |prepared| prepared._preview.rgba())
                    {
                        return Err(Error::new(
                            ErrorCode::ConflictStaleBase,
                            "Shape preview options changed",
                        ));
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    draft.state.fail(t!(
                        ctx,
                        "Could not place the shape: { $detail }",
                        detail = error
                    ));
                }
            }
            Response::Place => {
                let config = draft.state.config;
                if let Some(prepared) = draft.state.take(ctx, limits()) {
                    let result: Result<()> = (|| {
                        self.guard_asset_adoption(
                            &draft.source.source,
                            Some(&draft.source.identity),
                        )?;
                        self.editor.place_shape_mask(
                            prepared.raster,
                            config,
                            draft.budget.clone(),
                        )?;
                        if !self
                            .auxiliary_sources
                            .iter()
                            .any(|old| old.identity.matches(&draft.source.identity))
                        {
                            self.auxiliary_sources
                                .push(crate::destination::SourceGuard {
                                    path: draft.source.source.clone(),
                                    identity: draft.source.identity.clone(),
                                });
                        }
                        self.confirm = None;
                        Ok(())
                    })();
                    if let Err(error) = result {
                        draft.state.fail(t!(
                            ctx,
                            "Could not place the shape: { $detail }",
                            detail = error
                        ));
                    } else {
                        return;
                    }
                }
            }
        }
        self.shapes.placement = Some(draft);
    }
}
#[cfg(test)]
#[path = "placement/tests.rs"]
mod tests;
