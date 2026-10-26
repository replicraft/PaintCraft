//! Private conversion preview stays bound to the original browser document.
use super::*;
use paintcraft_core::types::resources::{BudgetAccount, SharedBudget};
use paintcraft_formats::csh::placement::{self, Policy, Preview, Rasterized};
use paintcraft_ui::shape_placement::{self, Config, Limits, Response};
#[derive(Debug)]
struct Prepared {
    raster: Rasterized,
    _preview: Preview,
    _copy: BudgetAccount,
}
#[derive(Debug)]
pub(super) struct Draft {
    source: Arc<Loaded>,
    index: usize,
    resource_epoch: uuid::Uuid,
    base: Arc<paintcraft_document::Document>,
    ticket: paintcraft_editor::session::Ticket,
    target: Option<uuid::Uuid>,
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
    fn current(&self, app: &WebApp) -> bool {
        app.frontend == Frontend::Studio
            && self.resource_epoch == app.shapes.epoch
            && self.index == app.shapes.selected
            && app
                .shapes
                .loaded
                .as_ref()
                .is_some_and(|loaded| Arc::ptr_eq(loaded, &self.source))
            && self.ticket == app.editor.session.ticket()
            && self.target == app.editor.target
            && Arc::ptr_eq(&self.base, &app.editor.session.snapshot())
            && !app.editor.tools.gesture_in_progress()
            && !app.editor.tools.is_editing_text()
            && !app.paste.is_placing()
    }
}
impl WebApp {
    pub(super) fn begin_shape_placement(&mut self) -> Result<()> {
        if self.frontend != Frontend::Studio || self.paste.is_placing() {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Finish the current editing operation before placing a shape",
            ));
        }
        self.editor.shape_target()?;
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
            color: self.controls.primary,
        };
        self.shapes.invalidate();
        self.shapes.placement = Some(Draft {
            source,
            index,
            resource_epoch: self.shapes.epoch,
            base: self.editor.session.snapshot(),
            ticket: self.editor.session.ticket(),
            target: self.editor.target,
            budget: SharedBudget::new(96 << 20),
            state: shape_placement::State::new(config),
        });
        Ok(())
    }
    pub(super) fn shape_placement_dialog(&mut self, ctx: &egui::Context) {
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
                let result: Result<()> = (|| {
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
                        .install(ctx, config, prepared, |p| p._preview.rgba())
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
                    match self.editor.place_shape_mask(
                        prepared.raster,
                        config,
                        draft.budget.clone(),
                    ) {
                        Ok(()) => {
                            self.shapes.open = false;
                            self.canvas.invalidate();
                            self.message = t!(ctx, "Shape placed.");
                            return;
                        }
                        Err(error) => draft.state.fail(t!(
                            ctx,
                            "Could not place the shape: { $detail }",
                            detail = error
                        )),
                    }
                }
            }
        }
        self.shapes.placement = Some(draft);
    }
}
#[cfg(test)]
mod tests;
