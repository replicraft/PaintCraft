//! Source5: separate bounded controls and priced registry requirements.
use super::*;
use paintcraft_core::types::policy::hex;
use paintcraft_formats::export::prepared_capabilities::{self as capabilities, Requirements};
pub const OPEN_BYTES_MAX: usize = 64 << 10;
pub const CAPABILITY_BYTES_MAX: usize = 128 << 10;
pub const BEGIN_BYTES_MAX: usize = 8 << 10;
pub const CONTROL_BYTES: u64 = (4 * OPEN_BYTES_MAX + 4 * CAPABILITY_BYTES_MAX + 4 * BEGIN_BYTES_MAX)
    as u64
    + 4 * std::mem::size_of::<Requirements>() as u64
    + capabilities::CONTROL_BYTES;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fact {
    pub count: usize,
    pub required: [u8; 32],
    pub resolution: [u8; 32],
}
impl Fact {
    pub fn capture(requirements: &Requirements, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        let (required, resolution) = requirements.digests(check)?;
        Ok(Self {
            count: requirements.len(),
            required,
            resolution,
        })
    }
    pub fn parse(value: &Value, max: usize, full: bool) -> Result<Self> {
        object(
            value,
            if full {
                &["count", "pairs", "requiredSha256", "resolutionSha256"]
            } else {
                &["count", "requiredSha256", "resolutionSha256"]
            },
        )?;
        let count = usize::try_from(integer(field(value, "count")?)?)
            .map_err(|_| resource("Prepared requirement count"))?;
        if count > max {
            return Err(resource("Prepared requirement count"));
        }
        Ok(Self {
            count,
            required: hash(field(value, "requiredSha256")?)?,
            resolution: hash(field(value, "resolutionSha256")?)?,
        })
    }
    pub fn value(self) -> Value {
        Value::object([
            ("count", Value::Int(self.count as i64)),
            ("requiredSha256", Value::str(hex(&self.required))),
            ("resolutionSha256", Value::str(hex(&self.resolution))),
        ])
    }
    pub fn matches(
        self,
        requirements: &Requirements,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if self != Self::capture(requirements, check)? {
            return Err(stale());
        }
        Ok(())
    }
}
fn hash(value: &Value) -> Result<[u8; 32]> {
    let text = digest(value)?;
    let mut result = [0; 32];
    for (byte, chunk) in result.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
        *byte = u8::from_str_radix(
            std::str::from_utf8(chunk).map_err(|_| bad("Prepared hash"))?,
            16,
        )
        .map_err(|_| bad("Prepared hash"))?;
    }
    Ok(result)
}
pub fn parse_control(text: &str, max: usize) -> Result<Value> {
    if text.len() > max {
        return Err(resource("Prepared control byte extent"));
    }
    json::parse(text.as_bytes()).map_err(|_| bad("Malformed prepared control"))
}
fn identifier(value: &Value, max: usize) -> Result<&str> {
    let text = string(value)?;
    if text.len() > max {
        return Err(bad("Prepared identifier extent"));
    }
    let (name, version) = text
        .rsplit_once("-v")
        .ok_or_else(|| bad("Prepared identifier version"))?;
    if !name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || version.is_empty()
        || version.starts_with('0')
        || !version.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(bad("Prepared identifier syntax"));
    }
    Ok(text)
}
pub fn pairs(
    value: &Value,
    max: usize,
    mut visit: impl FnMut(&str, &str) -> Result<()>,
) -> Result<usize> {
    let rows = value.as_array().ok_or_else(|| bad("Prepared pair list"))?;
    if rows.len() > max {
        return Err(resource("Prepared pair count"));
    }
    let mut previous = None;
    for row in rows {
        object(row, &["profile_id", "codec_id"])?;
        let pair = (
            identifier(field(row, "profile_id")?, 96)?,
            identifier(field(row, "codec_id")?, 64)?,
        );
        if previous.is_some_and(|p| p >= pair) {
            return Err(bad("Prepared pairs must be sorted unique"));
        }
        visit(pair.0, pair.1)?;
        previous = Some(pair);
    }
    Ok(rows.len())
}
pub fn requirements(value: &Value, max: usize) -> Result<Requirements> {
    let mut result = Requirements::default();
    pairs(value, max, |p, c| result.insert(p, c))?;
    Ok(result)
}
#[derive(Clone, Copy, Debug)]
pub struct Profile {
    pub bytes: u64,
    pub work: u64,
    pub source: Fact,
}
pub fn open(control: &str) -> Result<(Source, Profile)> {
    let value = parse_control(control, OPEN_BYTES_MAX)?;
    let base_names = [
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
    let mut names = base_names.to_vec();
    names.extend([
        "protocol",
        "registryContract",
        "workspaceBytes",
        "workLimit",
        "sourceRequirements",
    ]);
    object(&value, &names)?;
    if integer(field(&value, "protocol")?)? != 5
        || integer(field(&value, "registryContract")?)? != 1
    {
        return Err(bad("Prepared source contract"));
    }
    let bytes = integer(field(&value, "workspaceBytes")?)?;
    let work = integer(field(&value, "workLimit")?)?;
    if bytes > spatial::WORKSPACE_BYTES_MAX || work > spatial::WORK_MAX {
        return Err(resource("Prepared source workspace declaration"));
    }
    let source = Fact::parse(field(&value, "sourceRequirements")?, 256, true)?;
    if pairs(
        field(field(&value, "sourceRequirements")?, "pairs")?,
        256,
        |_, _| Ok(()),
    )? != source.count
    {
        return Err(bad("Prepared source count differs from pairs"));
    }
    let mut base = std::collections::BTreeMap::new();
    for name in base_names {
        base.insert(name.into(), field(&value, name)?.clone());
    }
    let mut parsed = Source::parse(Value::Object(base))?;
    parsed.quote = parsed
        .quote
        .checked_add(bytes)
        .and_then(|v| v.checked_add(CONTROL_BYTES))
        .filter(|v| *v <= LINEAR_MEMORY_BYTES - (32 << 20))
        .ok_or_else(|| resource("Prepared source exceeds linear-memory profile"))?;
    parsed.value = value;
    Ok((
        parsed,
        Profile {
            bytes,
            work,
            source,
        },
    ))
}
pub fn hello(control: &str) -> Result<Value> {
    let value = parse_control(control, CAPABILITY_BYTES_MAX)?;
    object(&value, &["registryContract", "pairs"])?;
    if integer(field(&value, "registryContract")?)? != 1 {
        return Err(bad("Prepared registry contract"));
    }
    let pairs_value = field(&value, "pairs")?;
    let requirements = requirements(pairs_value, 257)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(requirements.len())
        .map_err(|_| resource("Prepared capability reply"))?;
    for (profile, codec) in requirements.pairs() {
        let entry = capabilities::resolve(profile, codec)?;
        let (resource, revision) = entry.resource_contract();
        entries.push(Value::object([
            ("profile_id", Value::str(profile)),
            ("codec_id", Value::str(codec)),
            ("resource_contract_id", Value::str(resource)),
            (
                "resource_contract_revision",
                Value::Int(i64::from(revision)),
            ),
        ]));
    }
    let fact = Fact::capture(&requirements, &|| Ok(()))?;
    let reply = Value::object([
        ("kind", Value::str("capabilities")),
        ("protocol", Value::Int(5)),
        ("registryContract", Value::Int(1)),
        ("entries", Value::Array(entries)),
        ("requiredSha256", Value::str(hex(&fact.required))),
        ("resolutionSha256", Value::str(hex(&fact.resolution))),
    ]);
    if text(&reply).len() > CAPABILITY_BYTES_MAX {
        return Err(resource("Prepared capability reply extent"));
    }
    Ok(reply)
}
