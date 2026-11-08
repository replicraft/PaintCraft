//! Explicit layer-properties UI intention over the same atomic setter pair.
use super::*;
fn validate_pair(doc: &Document, batch: &Batch) -> Result<()> {
    use paintcraft_commands::{LayerRef, Op};
    match batch.operations.as_slice() {
        [
            Op::LayerSetProperties {
                layer: LayerRef::Id(layer),
                ..
            },
            Op::LayerSetBlendMode {
                layer: LayerRef::Id(blend_layer),
                ..
            },
        ] if layer == blend_layer
            && doc
                .layers
                .get(layer)
                .is_some_and(|actual| actual.id == *layer) =>
        {
            Ok(())
        }
        _ => Err(Error::new(
            ErrorCode::InputSchema,
            "Layer-properties intention requires its same existing-layer property and blend setter pair",
        )),
    }
}

impl Session {
    /// The native property dialog is one intent even when its blend setter is a
    /// no-op. Generic batches retain their existing derived history labels.
    pub fn apply_layer_properties(&mut self, batch: &Batch) -> Result<Receipt> {
        self.apply_layer_properties_checked(batch, Admission::new(self.staged_pixels), || Ok(()))
    }
    fn apply_layer_properties_checked(
        &mut self,
        batch: &Batch,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<Receipt> {
        check()?;
        validate_pair(&self.doc, batch)?;
        self.apply_checked_with_admission_label(
            batch,
            admission,
            paintcraft_history::Label::LayerProperties,
            check,
        )
    }
}
impl Preparation {
    pub fn apply_layer_properties(
        self,
        batch: &Batch,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        let admission = Admission::new(self.session.staged_pixels);
        self.apply_layer_properties_with_admission(batch, admission, check)
    }
    /// Retains this UI intention through private execution and guarded owner
    /// publication, without modifying command bytes or generic batch semantics.
    pub fn apply_layer_properties_with_admission(
        mut self,
        batch: &Batch,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        let receipt = self
            .session
            .apply_layer_properties_checked(batch, admission, &check)?;
        self.finish_edit(receipt, check)
    }
}
