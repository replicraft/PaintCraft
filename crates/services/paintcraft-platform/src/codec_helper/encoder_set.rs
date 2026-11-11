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
    validate_contracts(contracts)?;
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

fn validate_contracts(contracts: &[EncoderContract<'_>]) -> Result<()> {
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
    Ok(())
}

/// Descriptive availability observation for the installed application only.
/// Absence of an expected helper/manifest is None; existing but malformed or
/// unqualified installations still pass through the unchanged exact verifier.
/// # Errors
/// Preserves filesystem permission/storage, contract and cancellation failures.
pub fn observe_encoder_set(
    contracts: &[EncoderContract<'_>],
    check: &dyn Fn() -> Result<()>,
) -> Result<Option<VerifiedEncoderHelper>> {
    check()?;
    let application = std::env::current_exe().map_err(storage)?;
    observe_for_application(&application, contracts, Target::current()?, check)
}
fn observe_for_application(
    application: &Path,
    contracts: &[EncoderContract<'_>],
    target: Target,
    check: &dyn Fn() -> Result<()>,
) -> Result<Option<VerifiedEncoderHelper>> {
    check()?;
    validate_contracts(contracts)?;
    let directory = application
        .parent()
        .ok_or_else(|| unavailable("Application has no installation directory"))?;
    let manifest = layout::resolve(directory, target)?.path;
    if !layout::exists(&directory.join(target.executable()))? || !layout::exists(&manifest)? {
        check()?;
        return Ok(None);
    }
    let helper = verify_encoder_set(application, contracts, target, check)?;
    check()?;
    Ok(Some(helper))
}

#[cfg(test)]
mod tests;
