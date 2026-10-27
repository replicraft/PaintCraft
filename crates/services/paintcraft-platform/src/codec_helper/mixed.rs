//! Bounded upfront selection across exact host-owned schema2/schema3 contracts.
use super::*;
#[derive(Debug)]
enum Selected {
    Schema2(VerifiedHelper),
    Schema3(VerifiedSchema3DecoderHelper),
}
/// Sealed decoder installation; never constructed from an encoder capability.
#[derive(Debug)]
pub struct VerifiedDecoderInstallation {
    selected: Selected,
}
impl VerifiedDecoderInstallation {
    pub fn checked_executable(&self, check: &dyn Fn() -> Result<()>) -> Result<&Path> {
        match &self.selected {
            Selected::Schema2(value) => value.checked_executable(check),
            Selected::Schema3(value) => value.checked_executable(check),
        }
    }
    pub fn sha256(&self) -> [u8; 32] {
        match &self.selected {
            Selected::Schema2(value) => value.sha256(),
            Selected::Schema3(value) => value.sha256(),
        }
    }
}
/// At most four historical schema2 contracts and one complete schema3 contract.
/// Select once from one bounded manifest; integrity failures never retry.
pub fn discover_decoder_compatible(
    legacy: &[Contract<'_>],
    current: EncoderContract<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedDecoderInstallation> {
    let application = std::env::current_exe().map_err(storage)?;
    for_application_decoder_compatible(&application, legacy, current, check)
}
pub fn for_application_decoder_compatible(
    application: &Path,
    legacy: &[Contract<'_>],
    current: EncoderContract<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedDecoderInstallation> {
    verify_mixed(application, legacy, current, Target::current()?, check)
}
pub(super) fn verify_mixed(
    application: &Path,
    legacy: &[Contract<'_>],
    current: EncoderContract<'_>,
    target: Target,
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedDecoderInstallation> {
    current.validate()?;
    if legacy.len() > 4 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Too many legacy decoder contracts",
        ));
    }
    for (index, contract) in legacy.iter().copied().enumerate() {
        contract.validate()?;
        if contract.application_version != current.decode.application_version
            || contract.protocol_version != current.decode.protocol_version
            || contract.requested_heap_ceiling_bytes != current.decode.requested_heap_ceiling_bytes
            || contract.profile == current.decode.profile
            || legacy[..index]
                .iter()
                .any(|other| other.profile == contract.profile)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Incompatible mixed decoder contracts",
            ));
        }
    }
    let schema3 = std::cell::Cell::new(false);
    let verified = verify_selected(application, target, check, |value| {
        let profile = value.get("profile").and_then(Value::as_str);
        if profile == Some(current.decode.profile) {
            let selected = current.select(value)?;
            schema3.set(true);
            return Ok(selected);
        }
        let selected = legacy
            .iter()
            .copied()
            .find(|contract| profile == Some(contract.profile))
            .ok_or_else(|| unavailable("Installed decoder profile is not a known host contract"))?;
        if value.as_object().is_none_or(|fields| fields.len() != 10)
            || value.get("schema_version").and_then(Value::as_i64) != Some(2)
        {
            return Err(unavailable(
                "Installed legacy decoder manifest schema/fields differ",
            ));
        }
        Ok(selected)
    })?;
    let selected = if schema3.get() {
        Selected::Schema3(VerifiedSchema3DecoderHelper::from_verified(verified))
    } else {
        Selected::Schema2(verified)
    };
    Ok(VerifiedDecoderInstallation { selected })
}
