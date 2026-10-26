//! Existing resolved-mask execution with an explicit brush history intention.
use super::*;

impl Preparation {
    /// Prepare a static sampled brush using the existing mask command. Labels
    /// are presentation metadata; admission, cancellation, history retention and
    /// guarded publication use the same path as every other atomic batch.
    pub fn apply_mask_stroke(
        self,
        batch: &Batch,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        let admission = Admission::new(self.session.staged_pixels);
        self.apply_mask_stroke_with_admission(batch, admission, check)
    }
    pub fn apply_mask_stroke_with_admission(
        mut self,
        batch: &Batch,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        check()?;
        if batch
            .operations
            .iter()
            .any(|operation| !matches!(operation, paintcraft_commands::Op::MaskSpans { .. }))
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "sampled stroke preparation requires resolved mask operations",
            ));
        }
        let receipt = self.session.apply_checked_with_admission_label(
            batch,
            admission,
            paintcraft_history::Label::Brush,
            &check,
        )?;
        self.finish_edit(receipt, check)
    }
}
