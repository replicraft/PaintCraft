//! Paid scalar requirement inspection only. No source domain or authority escapes.
use super::*;
impl ClosedSource8 {
    pub(in crate::source8) fn service_noise_checked(
        &self,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<bool> {
        spend(8192)?;
        let payload = self.payload.0.as_deref().ok_or_else(incomplete)?;
        let requirements = payload._adopted.requirements();
        let count = requirements.requirements().len();
        spend((count as u64).checked_mul(16384).ok_or_else(extent)?)?;
        requirements.requires_noise()
    }
}
