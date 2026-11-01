//! Source6 wire only. No source adoption, execution counter or runtime owner.
use super::programs::{Fact as BaseFact, pairs, parse_control};
use super::*;
use paintcraft_core::types::policy::hex;
use paintcraft_formats::export::{
    noise_workload::Workload,
    prepared_capabilities::{self as capabilities, RegistryContract, Requirements},
};
mod begin;
mod fact;
mod hello;
pub use begin::{Begin, begin};
pub use fact::Fact;
pub use hello::hello;
pub const OPEN_BYTES_MAX: usize = 64 << 10;
pub const CAPABILITY_BYTES_MAX: usize = 128 << 10;
pub const BEGIN_BYTES_MAX: usize = 8 << 10;
/// Wire/parser inventories only. Runtime must additionally price its new retained
/// service/frame/replacement handles and actual NoiseAdmission control quote.
pub const CONTROL_BYTES: u64 = (4 * OPEN_BYTES_MAX + 4 * CAPABILITY_BYTES_MAX + 4 * BEGIN_BYTES_MAX)
    as u64
    + 4 * std::mem::size_of::<Requirements>() as u64
    + capabilities::CONTROL_BYTES
    + std::mem::size_of::<Profile>() as u64
    + std::mem::size_of::<Begin>() as u64;
#[derive(Clone, Copy, Debug)]
pub struct Profile {
    pub workload: Workload,
    pub source: Fact,
}
const BASE_NAMES: [&str; 14] = [
    "owner",
    "draft",
    "document",
    "sourceSha256",
    "size",
    "layers",
    "residentBytes",
    "metadataBytes",
    "metadataCharge",
    "filterDigest",
    "maskCount",
    "maskBands",
    "maskRuns",
    "maskDecodeBytes",
];
pub fn open(control: &str, check: &dyn Fn() -> Result<()>) -> Result<(Source, Profile)> {
    let value = parse_control(control, OPEN_BYTES_MAX)?;
    object(
        &value,
        &[
            "owner",
            "draft",
            "document",
            "sourceSha256",
            "size",
            "layers",
            "residentBytes",
            "metadataBytes",
            "metadataCharge",
            "filterDigest",
            "maskCount",
            "maskBands",
            "maskRuns",
            "maskDecodeBytes",
            "protocol",
            "registryContract",
            "sourceRequirements",
            "workload",
        ],
    )?;
    header(&value)?;
    check()?;
    let workload = Workload::parse_browser(field(&value, "workload")?)?;
    let source = Fact::parse(field(&value, "sourceRequirements")?, 256, true)?;
    let declared = requirements_checked(
        field(field(&value, "sourceRequirements")?, "pairs")?,
        256,
        check,
    )?;
    source.matches(&declared, workload, check)?;
    let mut base = std::collections::BTreeMap::new();
    for name in BASE_NAMES {
        base.insert(name.into(), field(&value, name)?.clone());
    }
    let mut parsed = Source::parse(Value::Object(base))?;
    parsed.quote = parsed
        .quote
        .checked_add(workload.limits().bytes)
        .and_then(|n| n.checked_add(CONTROL_BYTES))
        .filter(|n| *n <= LINEAR_MEMORY_BYTES - (32 << 20))
        .ok_or_else(|| resource("Scoped source exceeds linear-memory profile"))?;
    parsed.value = value;
    Ok((parsed, Profile { workload, source }))
}
/// Strict ScopedNoise2 subset. The bounded count is checked before visiting rows.
/// Does not call the legacy Source5 requirements helper, which is now closed.
pub fn requirements(value: &Value, max: usize) -> Result<Requirements> {
    requirements_checked(value, max, &|| Ok(()))
}
fn requirements_checked(
    value: &Value,
    max: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Requirements> {
    if max > 257 {
        return Err(resource("Scoped requirement bound"));
    }
    let mut result = Requirements::default();
    pairs(value, max, |profile, codec| {
        check()?;
        capabilities::resolve_for(RegistryContract::ScopedNoise2, profile, codec)?;
        result.insert(profile, codec)
    })?;
    Ok(result)
}
fn header(value: &Value) -> Result<()> {
    let revision = integer(field(value, "registryContract")?)?;
    if integer(field(value, "protocol")?)? != 6 || revision != 2 {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Source6 requires scoped registry contract 2",
        ));
    }
    RegistryContract::try_from(revision as u16)?;
    Ok(())
}
pub(crate) fn hash(value: &Value) -> Result<[u8; 32]> {
    let text = digest(value)?;
    let mut result = [0; 32];
    for (byte, chunk) in result.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
        *byte = u8::from_str_radix(
            std::str::from_utf8(chunk).map_err(|_| bad("Scoped hash"))?,
            16,
        )
        .map_err(|_| bad("Scoped hash"))?;
    }
    Ok(result)
}
#[cfg(test)]
mod tests;
