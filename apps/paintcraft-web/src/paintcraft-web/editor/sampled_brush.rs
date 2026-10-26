//! Browser sampled work is synchronous and conservatively admitted before rasterization.
use super::*;
use paintcraft_tools::sampled_brush::SampledStroke;
// Includes conservative dab-support visits, not a latency/RSS guarantee.
pub(crate) const SAMPLED_WORK_MAX: u64 = 1_048_576;
impl Editor {
    pub(crate) fn sampled_operations(&self, stroke: &SampledStroke) -> Result<Vec<Value>> {
        stroke.admit_work(SAMPLED_WORK_MAX)?;
        let resolved = stroke.preview(
            self.session.document().canvas_rect(),
            &paintcraft_core::types::resources::SharedBudget::new(32 << 20),
            &|| Ok(()),
        )?;
        if self.session.document().layers.is_empty() {
            resolved.operations_with_first_layer(self.session.document().canvas, &|| Ok(()))
        } else {
            resolved.operations(&|| Ok(()))
        }
    }
    pub(super) fn commit_sampled(&mut self, stroke: SampledStroke) -> Result<()> {
        let operations = self.sampled_operations(&stroke)?;
        if operations.is_empty() {
            return Ok(());
        }
        let batch = self.batch(operations)?;
        let empty = self.session.document().layers.is_empty();
        let preparation = self.session.preparation()?;
        let prepared = if empty {
            preparation.apply_initial_mask_stroke(&batch, || Ok(()))?
        } else {
            preparation.apply_mask_stroke(&batch, || Ok(()))?
        };
        let receipt = self.session.publish_prepared(prepared, || Ok(()))?;
        if receipt.committed {
            self.target = receipt
                .aliases
                .get("sampled-first")
                .copied()
                .or(self.target);
            self.retarget();
        }
        Ok(())
    }
}
