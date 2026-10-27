//! Bounded synchronous pen edit path; no asynchronous cancellation claim.
use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_input::tablet::{Phase, Sample};
use paintcraft_tools::pen_stroke::PenStroke;
impl Editor {
    pub(crate) fn cancel_pen_capture(&mut self) {
        self.tools.cancel_gesture();
        self.pen_base = None;
    }
    pub(crate) fn pen_sample(&mut self, sample: Sample, budget: &SharedBudget) -> Result<bool> {
        if matches!(sample.phase, Phase::Cancel | Phase::Leave) {
            self.tools.cancel_gesture();
            self.pen_base = None;
            return Ok(true);
        }
        if sample.phase == Phase::Down {
            if self.tools.is_editing_text()
                || self.tools.floating_selection().is_some()
                || self.tools.is_constructing_shape()
            {
                return Err(Error::new(
                    ErrorCode::TargetInvalid,
                    "Finish the current draft before pen painting",
                ));
            }
            if !self.tools.pressure_eligible(sample) {
                return Ok(false);
            }
            let layer = self.target.ok_or_else(|| {
                Error::new(
                    ErrorCode::TargetInvalid,
                    "Choose a visible raster layer before pen painting",
                )
            })?;
            if !self
                .session
                .document()
                .layers
                .get(&layer)
                .is_some_and(|l| l.visible)
            {
                return Err(Error::new(
                    ErrorCode::TargetInvalid,
                    "Choose a visible raster layer before pen painting",
                ));
            }
            if self.tools.begin_pen(layer, sample, budget)? {
                self.pen_base = Some(self.session.ticket());
                return Ok(true);
            }
            return Ok(false);
        }
        if !self.tools.pen_in_progress() {
            return Ok(false);
        }
        if self.pen_base != Some(self.session.ticket()) {
            self.tools.cancel_gesture();
            self.pen_base = None;
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Pen source document changed during capture",
            ));
        }
        if let Some((_, action)) = self.tools.append_pen(sample)? {
            self.pen_base = None;
            self.apply(action)?;
        }
        Ok(true)
    }
    pub(super) fn commit_pen(&mut self, stroke: PenStroke) -> Result<()> {
        let resolved =
            stroke.resolve_captured(self.session.document().canvas_rect(), &|| Ok(()))?;
        let operations = resolved.operations(&|| Ok(()))?;
        if operations.is_empty() {
            return Ok(());
        }
        let batch = self.batch(operations)?;
        let prepared = self.session.preparation()?.apply_pen_mask_stroke(
            &batch,
            stroke.style().intent,
            || Ok(()),
        )?;
        let before = self.session.ticket();
        let receipt = self.session.publish_prepared(prepared, || Ok(()))?;
        self.record_publication(before, &receipt);
        if receipt.committed {
            self.retarget();
        }
        drop((stroke, resolved));
        Ok(())
    }
}

#[cfg(test)]
mod tests;
