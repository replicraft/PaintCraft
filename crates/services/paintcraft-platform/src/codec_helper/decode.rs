//! Exact schema3 decoder capability, separate from schema2 and encoder handles.
use super::*;
#[derive(Debug)]
pub struct VerifiedSchema3DecoderHelper {
    verified: VerifiedHelper,
}
impl VerifiedSchema3DecoderHelper {
    pub(super) fn from_verified(verified: VerifiedHelper) -> Self {
        Self { verified }
    }
    pub fn checked_executable(&self, check: &dyn Fn() -> Result<()>) -> Result<&Path> {
        self.verified.checked_executable(check)
    }
    pub fn sha256(&self) -> [u8; 32] {
        self.verified.sha256()
    }
}
/// Host-owned complete schema3 contract; no selected-file or PATH fallback.
pub fn discover_decoder(
    contract: EncoderContract<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedSchema3DecoderHelper> {
    let application = std::env::current_exe().map_err(storage)?;
    for_application_decoder(&application, contract, check)
}
pub fn for_application_decoder(
    application: &Path,
    contract: EncoderContract<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedSchema3DecoderHelper> {
    verify_decoder(application, contract, Target::current()?, check)
}
pub(super) fn verify_decoder(
    application: &Path,
    contract: EncoderContract<'_>,
    target: Target,
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedSchema3DecoderHelper> {
    contract.validate()?;
    let verified = verify_selected(application, target, check, |value| contract.select(value))?;
    Ok(VerifiedSchema3DecoderHelper { verified })
}
