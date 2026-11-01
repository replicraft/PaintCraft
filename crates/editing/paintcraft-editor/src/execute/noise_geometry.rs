//! Until editable coordinate maps exist, refuse source-coordinate remapping.
use super::*;
impl Stage<'_> {
    pub(super) fn refuse_noise_geometry(&self, id: Uuid) -> Result<()> {
        if self.doc.layers[&id].filters.iter().any(|f| {
            matches!(
                f.descriptor(),
                paintcraft_core::types::filters::Descriptor::AddNoiseRgbV1(_)
            )
        }) {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Source-coordinate transforms of stored Noise require an explicit bake",
            ));
        }
        Ok(())
    }
}
