//! Upfront exact selection for at most two host-owned schema3 encoder profiles.
use super::*;
pub fn discover_encoder_set(
    contracts: &[EncoderContract<'_>],
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedEncoderHelper> {
    let application = std::env::current_exe().map_err(storage)?;
    for_application_encoder_set(&application, contracts, check)
}
pub fn for_application_encoder_set(
    application: &Path,
    contracts: &[EncoderContract<'_>],
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedEncoderHelper> {
    verify_encoder_set(application, contracts, Target::current()?, check)
}
pub(super) fn verify_encoder_set(
    application: &Path,
    contracts: &[EncoderContract<'_>],
    target: Target,
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedEncoderHelper> {
    if contracts.is_empty() || contracts.len() > 2 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Invalid bounded encoder contract set",
        ));
    }
    for (index, contract) in contracts.iter().enumerate() {
        contract.validate()?;
        let first = contracts[0];
        if contract.decode.application_version != first.decode.application_version
            || contract.decode.protocol_version != first.decode.protocol_version
            || contract.decode.requested_heap_ceiling_bytes
                != first.decode.requested_heap_ceiling_bytes
            || contracts[..index]
                .iter()
                .any(|old| old.decode.profile == contract.decode.profile)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Incompatible encoder contracts",
            ));
        }
    }
    let verified = verify_selected(application, target, check, |value| {
        let profile = value.get("profile").and_then(Value::as_str);
        let selected = contracts
            .iter()
            .find(|contract| profile == Some(contract.decode.profile))
            .ok_or_else(|| unavailable("Installed encoder profile is not a known host contract"))?;
        selected.select(value)
    })?;
    Ok(VerifiedEncoderHelper::from_verified(verified))
}
