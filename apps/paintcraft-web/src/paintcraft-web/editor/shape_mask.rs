//! Browser shapes use the same admitted Shape mask transaction as desktop.
use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_formats::csh::placement::Rasterized;
use paintcraft_ui::shape_placement::Config;
impl Editor {
    pub(crate) fn shape_target(&self) -> Result<Option<Uuid>> {
        if self.tools.gesture_in_progress() || self.tools.is_editing_text() {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Finish the current editing operation before placing a shape",
            ));
        }
        if self.session.document().layers.is_empty() && self.session.document().roots.is_empty() {
            return Ok(None);
        }
        let target = self
            .target
            .filter(|id| {
                self.session
                    .document()
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
        if raster.dimensions() != (config.width, config.height) {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Shape placement dimensions changed",
            ));
        }
        let coverage = raster.translated_spans(
            config.at,
            self.session.document().canvas_rect(),
            &budget,
            &|| Ok(()),
        )?;
        let operations = match target {
            Some(layer) => {
                commands::mask_operations(layer, config.color, coverage.spans(), &|| Ok(()))?
            }
            None => commands::first_layer_mask_operations(
                self.session.document().canvas,
                config.color,
                coverage.spans(),
                "shape-first",
                "Shape",
                &|| Ok(()),
            )?,
        };
        if operations.is_empty() {
            return Ok(());
        }
        let batch = self.batch(operations)?;
        let preparation = self.session.preparation()?;
        let prepared = if target.is_none() {
            preparation.apply_initial_shape_mask(&batch, || Ok(()))?
        } else {
            preparation.apply_shape_mask(&batch, || Ok(()))?
        };
        let receipt = self.session.publish_prepared(prepared, || Ok(()))?;
        if receipt.committed {
            self.target = receipt.aliases.get("shape-first").copied().or(self.target);
            self.retarget();
        }
        Ok(())
    }
}
