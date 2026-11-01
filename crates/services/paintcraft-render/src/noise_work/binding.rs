use super::*;
use paintcraft_document::Document;
impl NoiseAdmission {
    /// Frozen complete prepared keys share this SAME pair; no new Noise owner.
    /// An already frozen owner validates and never extends or recompiles.
    pub fn prepare_document(&self, doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        with_noise_workspace(self.clone(), || {
            let caller = required()?;
            let base =
                crate::prepared::prepare_document_programs(doc, caller.workspace().clone(), check)?;
            caller.for_workspace(&base)
        })
    }
    /// Quote-only validation: unbound callers only quote; bound callers must
    /// contain every key. No new admission, LUT compilation or pixel allocation.
    pub fn validate_document(&self, doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<()> {
        self.check_available(WorkComponents::default())?;
        if self.workspace().has_prepared_programs() {
            for layer in doc.layers.values() {
                check()?;
                crate::prepared::validate_layer_programs(layer, self.workspace(), check)?;
            }
        } else {
            crate::prepared::quote_document_programs(doc, check)?;
        }
        Ok(())
    }
}
