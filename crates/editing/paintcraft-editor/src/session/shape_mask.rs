//! Shape-mask history labels reuse the same admitted atomic mask executor.
use super::*;
impl Preparation {
    pub fn apply_shape_mask(
        self,
        batch: &Batch,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        let admission = Admission::new(self.session.staged_pixels);
        self.apply_shape_mask_with_admission(batch, admission, check)
    }
    pub fn apply_shape_mask_with_admission(
        self,
        batch: &Batch,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        self.apply_mask_with_label(batch, admission, paintcraft_history::Label::Shape, check)
    }
    pub fn apply_initial_shape_mask(
        self,
        batch: &Batch,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        let admission = Admission::new(self.session.staged_pixels);
        self.apply_initial_shape_mask_with_admission(batch, admission, check)
    }
    pub fn apply_initial_shape_mask_with_admission(
        self,
        batch: &Batch,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        self.apply_initial_mask_with_label(
            batch,
            admission,
            paintcraft_history::Label::Shape,
            check,
        )
    }
}
