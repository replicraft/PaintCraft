//! Schema3 encoder capability; deliberately not convertible into a decoder capability.
use super::*;
#[derive(Clone, Copy, Debug)]
pub struct EncoderContract<'a> {
    pub decode: Contract<'a>,
    pub encode_protocol_version: u16,
    pub qualified_export_variants: &'a [&'a str],
}
impl<'a> EncoderContract<'a> {
    fn validate(self) -> Result<()> {
        self.decode.validate()?;
        if self.encode_protocol_version != 1
            || self.decode.protocol_version != 2
            || self.decode.qualified_import_variants.len() != 7
            || self.qualified_export_variants.len() != 1
            || self
                .qualified_export_variants
                .iter()
                .any(|variant| variant.is_empty() || variant.len() > 128 || !variant.is_ascii())
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Invalid installed encoder contract",
            ));
        }
        Ok(())
    }
    fn select(self, value: &Value) -> Result<Contract<'a>> {
        if value.as_object().is_none_or(|fields| fields.len() != 12)
            || value.get("schema_version").and_then(Value::as_i64) != Some(3)
            || value.get("encode_protocol_version").and_then(Value::as_i64)
                != Some(i64::from(self.encode_protocol_version))
            || value
                .get("qualified_export_variants")
                .and_then(Value::as_array)
                .is_none_or(|variants| {
                    variants.len() != self.qualified_export_variants.len()
                        || variants
                            .iter()
                            .zip(self.qualified_export_variants)
                            .any(|(actual, expected)| actual.as_str() != Some(*expected))
                })
        {
            return Err(unavailable(
                "Installed encoder manifest does not match its exact schema3 contract",
            ));
        }
        Ok(self.decode)
    }
}
#[derive(Debug)]
pub struct VerifiedEncoderHelper {
    verified: VerifiedHelper,
}
impl VerifiedEncoderHelper {
    pub fn checked_executable(&self, check: &dyn Fn() -> Result<()>) -> Result<&Path> {
        self.verified.checked_executable(check)
    }
    pub fn sha256(&self) -> [u8; 32] {
        self.verified.sha256()
    }
}
/// Installed application discovery only; no PATH or selected-helper fallback.
pub fn discover_encoder(
    contract: EncoderContract<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedEncoderHelper> {
    let application = std::env::current_exe().map_err(storage)?;
    for_application_encoder(&application, contract, check)
}
/// Host-owned application path for composition/installation tests, not an imported file.
pub fn for_application_encoder(
    application: &Path,
    contract: EncoderContract<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedEncoderHelper> {
    verify_encoder(application, contract, Target::current()?, check)
}
pub(super) fn verify_encoder(
    application: &Path,
    contract: EncoderContract<'_>,
    target: Target,
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedEncoderHelper> {
    contract.validate()?;
    let verified = verify_selected(application, target, check, |value| contract.select(value))?;
    Ok(VerifiedEncoderHelper { verified })
}
