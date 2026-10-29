//! Bounded upfront selection across exact host-owned schema2/schema3 contracts.
use super::*;
#[derive(Debug)]
enum Selected {
    Schema2(VerifiedHelper),
    Schema3(VerifiedSchema3DecoderHelper),
}
/// Fixed host-contract inventory. No manifest strings or unchecked capability
/// declarations escape into the sealed installation handle.
#[derive(Clone, Copy, Debug)]
struct Imports {
    names: [[u8; 128]; 9],
    lengths: [u8; 9],
    count: usize,
}
impl Imports {
    fn selected(contract: Contract<'_>) -> Self {
        // Every caller contract is validated before acquisition/selection.
        let mut value = Self {
            names: [[0; 128]; 9],
            lengths: [0; 9],
            count: contract.qualified_import_variants.len(),
        };
        for (index, name) in contract.qualified_import_variants.iter().enumerate() {
            value.names[index][..name.len()].copy_from_slice(name.as_bytes());
            value.lengths[index] = name.len() as u8;
        }
        value
    }
    fn supports(&self, name: &str) -> bool {
        (0..self.count)
            .any(|index| &self.names[index][..usize::from(self.lengths[index])] == name.as_bytes())
    }
}
/// Sealed decoder installation; never constructed from an encoder capability.
#[derive(Debug)]
pub struct VerifiedDecoderInstallation {
    selected: Selected,
    imports: Imports,
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
    /// Exact selected host inventory, sealed after full manifest/binary checks.
    /// This does not replace checked_executable's retained integrity checks.
    pub fn supports_import_variant(&self, variant: &str) -> bool {
        self.imports.supports(variant)
    }
}
/// Compatibility wrapper retaining the prior four-schema2/one-schema3 bound.
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
    if legacy.len() > 4 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Too many legacy decoder contracts",
        ));
    }
    verify_set(application, legacy, &[current], target, check)
}
/// At most five exact schema2 and two schema3 host contracts. Select once from
/// one bounded manifest; a selected integrity failure never retries a profile.
pub fn discover_decoder_compatible_set(
    legacy: &[Contract<'_>],
    current: &[EncoderContract<'_>],
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedDecoderInstallation> {
    let application = std::env::current_exe().map_err(storage)?;
    for_application_decoder_compatible_set(&application, legacy, current, check)
}
pub fn for_application_decoder_compatible_set(
    application: &Path,
    legacy: &[Contract<'_>],
    current: &[EncoderContract<'_>],
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedDecoderInstallation> {
    verify_set(application, legacy, current, Target::current()?, check)
}
pub(super) fn verify_set(
    application: &Path,
    legacy: &[Contract<'_>],
    current: &[EncoderContract<'_>],
    target: Target,
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedDecoderInstallation> {
    if legacy.len() > 5 || current.len() > 2 || legacy.is_empty() && current.is_empty() {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Invalid bounded decoder contract set",
        ));
    }
    for contract in current {
        contract.validate()?;
    }
    for contract in legacy {
        contract.validate()?;
    }
    let reference = legacy
        .first()
        .copied()
        .or_else(|| current.first().map(|c| c.decode))
        .unwrap();
    for (index, contract) in legacy
        .iter()
        .copied()
        .chain(current.iter().map(|c| c.decode))
        .enumerate()
    {
        if contract.application_version != reference.application_version
            || contract.protocol_version != reference.protocol_version
            || contract.requested_heap_ceiling_bytes != reference.requested_heap_ceiling_bytes
            || legacy
                .iter()
                .copied()
                .chain(current.iter().map(|c| c.decode))
                .take(index)
                .any(|prior| prior.profile == contract.profile)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Incompatible mixed decoder contracts",
            ));
        }
    }
    let selection = std::cell::Cell::new(None);
    let verified = verify_selected(application, target, check, |value| {
        let profile = value.get("profile").and_then(Value::as_str);
        if let Some(current) = current
            .iter()
            .find(|contract| profile == Some(contract.decode.profile))
        {
            let selected = current.select(value)?;
            selection.set(Some((true, Imports::selected(selected))));
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
        selection.set(Some((false, Imports::selected(selected))));
        Ok(selected)
    })?;
    let (schema3, imports) = selection
        .get()
        .ok_or_else(|| unavailable("Missing selected decoder contract"))?;
    let selected = if schema3 {
        Selected::Schema3(VerifiedSchema3DecoderHelper::from_verified(verified))
    } else {
        Selected::Schema2(verified)
    };
    Ok(VerifiedDecoderInstallation { selected, imports })
}
