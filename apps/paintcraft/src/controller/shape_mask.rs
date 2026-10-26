//! CSH conversion submits only existing resolved-mask transactions.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, resources::SharedBudget};
use paintcraft_formats::csh::placement::Rasterized;
use paintcraft_ui::shape_placement::Config;
impl Editor {
    pub(crate) fn shape_target(&self) -> Result<Option<Uuid>> {
        if self.has_pending_edits() || self.gesture_in_progress() || self.is_editing_text() {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Finish the current editing operation before placing a shape",
            ));
        }
        if self.document().layers.is_empty() && self.document().roots.is_empty() {
            return Ok(None);
        }
        let target = self
            .target
            .filter(|id| {
                self.document()
                    .layers
                    .get(id)
                    .is_some_and(|layer| layer.visible)
            })
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::TargetInvalid,
                    "Choose a visible raster layer before placing a shape",
                )
            })?;
        Ok(Some(target))
    }
    pub(crate) fn place_shape_mask(
        &mut self,
        raster: Rasterized,
        config: Config,
        budget: SharedBudget,
    ) -> Result<()> {
        let target = self.shape_target()?;
        let canvas = self.document().canvas;
        let clip = self.document().canvas_rect();
        if raster.dimensions() != (config.width, config.height) {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Shape placement dimensions changed",
            ));
        }
        let ticket = self.session.ticket();
        let build = move |check: &dyn Fn() -> Result<()>| -> Result<Option<Batch>> {
            let coverage = raster.translated_spans(config.at, clip, &budget, check)?;
            let operations = match target {
                Some(layer) => paintcraft_tools::commands::mask_operations(
                    layer,
                    config.color,
                    coverage.spans(),
                    &check,
                )?,
                None => paintcraft_tools::commands::first_layer_mask_operations(
                    canvas,
                    config.color,
                    coverage.spans(),
                    "shape-first",
                    "Shape",
                    &check,
                )?,
            };
            if operations.is_empty() {
                return Ok(None);
            }
            Batch::from_value(&Value::object([
                ("schema", Value::str("paintcraft.batch/v2")),
                ("atomic", Value::Bool(true)),
                ("policy", Value::str(POLICY)),
                (
                    "base",
                    Value::object([
                        ("state_id", Value::str(ticket.state_id.to_string())),
                        (
                            "publication_epoch",
                            Value::str(ticket.publication_epoch.to_string()),
                        ),
                    ]),
                ),
                ("operations", Value::Array(operations)),
            ]))
            .map(Some)
        };
        let preparation = self.session.preparation()?;
        if let Some(service) = &self.editing {
            let admission = service.admission(u64::MAX);
            let task = Box::new(move |cancel: paintcraft_runtime::Cancellation| {
                let check = || cancel.check();
                let prepared = match build(&check)? {
                    None => preparation.no_change(check)?,
                    Some(batch) if target.is_none() => preparation
                        .apply_initial_shape_mask_with_admission(&batch, admission, check)?,
                    Some(batch) => {
                        preparation.apply_shape_mask_with_admission(&batch, admission, check)?
                    }
                };
                Ok(background::Completion::Edit(prepared))
            }) as dispatch::Task;
            self.dispatch_initial(
                Ok(task),
                "Shape".into(),
                "",
                target.is_none().then_some("shape-first"),
            );
            Ok(())
        } else {
            let Some(batch) = build(&|| Ok(()))? else {
                return Ok(());
            };
            let prepared = if target.is_none() {
                preparation.apply_initial_shape_mask(&batch, || Ok(()))?
            } else {
                preparation.apply_shape_mask(&batch, || Ok(()))?
            };
            let receipt = self.session.publish_prepared(prepared, || Ok(()))?;
            self.accept_edit(
                &receipt,
                "Shape",
                target.is_none().then_some("shape-first"),
                canvas,
            );
            Ok(())
        }
    }
}
