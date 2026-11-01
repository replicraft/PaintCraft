//! Immutable finite source-local Layer coverage; not a selection or filter mask.
use paintcraft_raster::coverage_mask::A8Mask;
use sha2::{Digest, Sha256};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerMask {
    enabled: bool,
    coverage: A8Mask,
}
impl LayerMask {
    pub fn new(enabled: bool, coverage: A8Mask) -> Self {
        Self { enabled, coverage }
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn coverage(&self) -> &A8Mask {
        &self.coverage
    }
    pub fn with_enabled(&self, enabled: bool) -> Self {
        Self {
            enabled,
            coverage: self.coverage.clone(),
        }
    }
    pub fn with_coverage(&self, coverage: A8Mask) -> Self {
        Self {
            enabled: self.enabled,
            coverage,
        }
    }
    /// A8 digest already binds the finite domain and canonical visible coverage.
    /// Presence is carried by Layer.mask's Option, not conflated with zero/full.
    pub fn digest(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"paintcraft.layer-mask/v1\0");
        h.update([u8::from(self.enabled)]);
        h.update(self.coverage.digest());
        h.finalize().into()
    }
}
#[cfg(test)]
#[path = "layer_mask/tests.rs"]
mod tests;
