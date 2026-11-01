//! Source7 only. Parsing/hashes grant no adoption, render work or counter reset.
use super::programs::{pairs, parse_control};
use super::scoped_programs::hash;
use super::*;
pub use metadata::{Fact7, Union7};
use paintcraft_core::types::policy::hex;
use paintcraft_formats::export::{
    layer_appearance::{
        self as metadata, Admission7, CoverageAbi, LayerMasks7, Mode7, Requirements7,
        appearance_registration,
    },
    prepared_capabilities::{self as capabilities, RegistryContract, Requirements},
};
mod begin;
mod facts;
mod hello;
pub use begin::{Begin, begin};
pub use hello::hello;
pub const OPEN_BYTES_MAX: usize = 64 << 10;
pub const CAPABILITY_BYTES_MAX: usize = 128 << 10;
pub const BEGIN_BYTES_MAX: usize = 8 << 10;
pub const CONTROL_BYTES: u64 = (4 * OPEN_BYTES_MAX + 4 * CAPABILITY_BYTES_MAX + 4 * BEGIN_BYTES_MAX)
    as u64
    + 4 * std::mem::size_of::<Requirements7>() as u64
    + paintcraft_formats::pcnd::layer_appearance::CONTROL_BYTES
    + std::mem::size_of::<Profile>() as u64
    + std::mem::size_of::<Begin>() as u64;
#[derive(Clone, Debug)]
pub struct Profile {
    pub admission: Admission7,
    pub source: Fact7,
    pub requirements: Requirements7,
}
impl Profile {
    /// Legacy base14 coverage fields remain FILTER-ONLY, unchanged.
    pub fn aggregate_coverage(
        &self,
        base: &Source,
    ) -> Result<paintcraft_formats::pcnd::CoverageFacts> {
        let own = self.source.own;
        if u64::from(own.stored) > base.layers as u64 {
            return Err(resource("Source7 own count"));
        }
        let facts = paintcraft_formats::pcnd::CoverageFacts {
            masks: base
                .masks
                .masks
                .checked_add(u64::from(own.stored))
                .ok_or_else(bound)?,
            bands: base.masks.bands.checked_add(own.bands).ok_or_else(bound)?,
            runs: base.masks.runs.checked_add(own.runs).ok_or_else(bound)?,
            decode_bytes: base
                .masks
                .decode_bytes
                .checked_add(own.decode_upper_bytes)
                .ok_or_else(bound)?,
        };
        let limits = paintcraft_raster::coverage_mask::Limits::default();
        if facts.masks > base.layers as u64 * 257
            || facts.bands > limits.bands as u64
            || facts.runs > limits.runs as u64
        {
            return Err(bound());
        }
        Ok(facts)
    }
    pub fn source_quote(&self, base: &Source) -> Result<u64> {
        self.aggregate_coverage(base)?;
        base.quote
            .checked_add(self.source.own.decode_upper_bytes)
            .and_then(|n| n.checked_add(self.admission.workspace_bytes))
            .and_then(|n| n.checked_add(CONTROL_BYTES))
            .and_then(|n| {
                n.checked_add(
                    paintcraft_formats::pcnd::layer_appearance::transient_quote(base.layers)
                        .ok()?
                        .checked_sub(paintcraft_formats::pcnd::layer_appearance::CONTROL_BYTES)?,
                )
            })
            .filter(|n| *n <= LINEAR_MEMORY_BYTES - (32 << 20))
            .ok_or_else(bound)
    }
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
            "appearance",
            "admission",
            "slice",
        ],
    )?;
    header(&value)?;
    check()?;
    let owner = field(&value, "owner")?;
    for name in ["session", "state"] {
        uuid(field(owner, name)?)?;
    }
    uuid(field(&value, "draft")?)?;
    uuid(field(&value, "document")?)?;
    appearance(field(&value, "appearance")?)?;
    let admission = admission(&value)?;
    let source = facts::fact(field(&value, "sourceRequirements")?, true)?;
    if source.own.stored == 0 {
        return Err(bad("Source7 requires own-mask intent"));
    }
    let requirements = requirements(
        field(field(&value, "sourceRequirements")?, "pairs")?,
        256,
        check,
    )?;
    admission.validate_requirements(&requirements)?;
    let (required, resolution) = requirements.requirements().digests(check)?;
    if source.count != requirements.requirements().len()
        || source.required != required
        || source.resolution != resolution
    {
        return Err(stale());
    }
    source.validate_binding(admission, appearance_registration())?;
    let mut fields = std::collections::BTreeMap::new();
    for name in BASE_NAMES {
        fields.insert(name.into(), field(&value, name)?.clone());
    }
    let mut base = Source::parse(Value::Object(fields))?;
    base.value = value;
    let profile = Profile {
        admission,
        source,
        requirements,
    };
    profile.source_quote(&base)?;
    Ok((base, profile))
}
pub fn requirements(
    value: &Value,
    max: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Requirements7> {
    if max > 257 {
        return Err(bound());
    }
    let mut required = Requirements::default();
    pairs(value, max, |profile, codec| {
        check()?;
        capabilities::resolve_for(RegistryContract::LayerAppearance3, profile, codec)?;
        required.insert(profile, codec)
    })?;
    Requirements7::new(required, appearance_registration(), check)
}
fn header(value: &Value) -> Result<()> {
    if integer(field(value, "protocol")?)? != 7 || integer(field(value, "registryContract")?)? != 3
    {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Source7 requires registry3",
        ));
    }
    Ok(())
}
fn appearance(value: &Value) -> Result<()> {
    object(
        value,
        &[
            "feature_id",
            "resource_contract_id",
            "resource_contract_revision",
            "query_revision",
            "alpha_revision",
            "workload_class_id",
            "workload_class_revision",
        ],
    )?;
    if *value != appearance_registration().value() {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Source7 appearance contract",
        ));
    }
    Ok(())
}
fn admission(value: &Value) -> Result<Admission7> {
    let a = field(value, "admission")?;
    object(
        a,
        &[
            "mode",
            "workspaceBytes",
            "baseWorkLimit",
            "noiseWorkLimit",
            "totalWorkLimit",
        ],
    )?;
    let mode = match field(a, "mode")?.as_str() {
        Some("base") => Mode7::Base,
        Some("noise") => Mode7::Noise,
        _ => return Err(bad("Source7 mode")),
    };
    let result = Admission7 {
        mode,
        workspace_bytes: decimal(field(a, "workspaceBytes")?)?,
        base_work: decimal(field(a, "baseWorkLimit")?)?,
        noise_work: decimal(field(a, "noiseWorkLimit")?)?,
    };
    result.validate()?;
    if decimal(field(a, "totalWorkLimit")?)? != result.total()? {
        return Err(bad("Source7 total work"));
    }
    let slice = field(value, "slice")?;
    object(
        slice,
        &["revision", "nativeEdge", "baseWorkLimit", "noiseWorkLimit"],
    )?;
    if *slice != result.slice_value() {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Source7 compiled slice policy",
        ));
    }
    Ok(result)
}
fn bound() -> Error {
    resource("Source7 metadata/control bound")
}
#[cfg(test)]
mod tests;

// Source7 envelope IDs use the same canonical spelling as its JS transport.
// Legacy protocols retain their existing shared UUID parser unchanged.
fn uuid(value: &Value) -> Result<Uuid> {
    let spelling = string(value)?;
    let id = Uuid::parse_str(spelling).map_err(|_| bad("Source7 UUID"))?;
    let mut buffer = [0; 36];
    let canonical: &str = id.hyphenated().encode_lower(&mut buffer);
    if canonical != spelling {
        return Err(bad(
            "Source7 UUID must be canonical lowercase hyphenated text",
        ));
    }
    Ok(id)
}
