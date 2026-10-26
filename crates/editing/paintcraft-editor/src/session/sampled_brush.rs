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
        self,
        batch: &Batch,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        self.apply_mask_with_label(batch, admission, paintcraft_history::Label::Brush, check)
    }
    pub(super) fn apply_mask_with_label(
        mut self,
        batch: &Batch,
        admission: Admission,
        label: paintcraft_history::Label,
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
                "shape or brush preparation requires resolved mask operations",
            ));
        }
        let receipt = self
            .session
            .apply_checked_with_admission_label(batch, admission, label, &check)?;
        self.finish_edit(receipt, check)
    }
}

impl Preparation {
    /// Atomically create the first transparent canvas-sized layer and paint its
    /// resolved sampled mask. An entirely ineffective mask must not leave an
    /// empty layer, change dirty state or consume a history entry.
    pub fn apply_initial_mask_stroke(
        self,
        batch: &Batch,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        let admission = Admission::new(self.session.staged_pixels);
        self.apply_initial_mask_stroke_with_admission(batch, admission, check)
    }
    pub fn apply_initial_mask_stroke_with_admission(
        self,
        batch: &Batch,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        self.apply_initial_mask_with_label(
            batch,
            admission,
            paintcraft_history::Label::Brush,
            check,
        )
    }
    pub(super) fn apply_initial_mask_with_label(
        mut self,
        batch: &Batch,
        admission: Admission,
        label: paintcraft_history::Label,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        use paintcraft_commands::{LayerRef, Op};
        use paintcraft_core::types::Rgba;
        check()?;
        self.session.validate_batch_base(batch)?;
        let [
            Op::LayerAdd {
                alias,
                width,
                height,
                origin,
                default,
                ..
            },
            masks @ ..,
        ] = batch.operations.as_slice()
        else {
            return Err(initial_schema());
        };
        if batch.version != 2 || !self.session.doc.layers.is_empty() || !self.session.doc.roots.is_empty()
            || (*width,*height) != self.session.doc.canvas || *origin != (0,0)
            || *default != Rgba::TRANSPARENT || masks.is_empty()
            || masks.iter().any(|op| !matches!(op, Op::MaskSpans {layer: LayerRef::Alias(target),..} if target == alias)) {
            return Err(initial_schema());
        }
        let staged = execute_with_admission_checked(
            &self.session.doc,
            self.session.selection.clone(),
            batch,
            admission.constrain_pixels(self.session.staged_pixels),
            &check,
        )?;
        check()?;
        // Every admitted mask is source-over onto a newly transparent layer.
        // Alpha cannot decrease; zero stored tiles means no effective paint.
        // No second renderer or approximate coverage test decides this outcome.
        if staged
            .doc
            .layers
            .values()
            .all(|layer| layer.raster.resident_tiles() == 0)
        {
            return self.no_change(check);
        }
        let receipt = self
            .session
            .commit_staged(staged, &batch.digest, label, &check)?;
        self.finish_edit(receipt, check)
    }
}
fn initial_schema() -> Error {
    Error::new(
        ErrorCode::InputSchema,
        "initial resolved mask requires one transparent canvas-sized first layer followed only by masks for its alias",
    )
}
