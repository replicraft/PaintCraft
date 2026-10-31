//! Source6/Export8 workload bindings. Hashes bind facts; they do not grant work.
//! The endpoint selects BrowserV1, validates limits, and owns the actual scope.
use paintcraft_core::types::{Error, ErrorCode, Result, json::Value, policy::hex};
use paintcraft_render::noise_work::{
    NoiseJobPolicy, NoiseLimits, WORKLOAD_CLASS_ID, WORKLOAD_CLASS_REVISION, WorkComponents,
};
use sha2::{Digest, Sha256};

/// Only a validated browser/export declaration can construct this wire binding.
/// Native job roots select their own policy directly; wire cannot select it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Workload {
    limits: NoiseLimits,
}
impl Workload {
    pub fn browser(limits: NoiseLimits) -> Result<Self> {
        NoiseJobPolicy::BrowserV1.validate(limits)?;
        Ok(Self { limits })
    }
    pub fn limits(self) -> NoiseLimits {
        self.limits
    }
    /// Explicit binary grammar, independent of JSON field order or Rust layout:
    /// domain, class-id length u16BE, UTF8 id, revision u32BE, then four u64BE
    /// values: bytes, base limit, Noise limit, checked combined limit.
    pub fn digest(self) -> Result<[u8; 32]> {
        let mut hash = Sha256::new();
        hash.update(b"paintcraft.noise.workload/v1\0");
        let length = u16::try_from(WORKLOAD_CLASS_ID.len()).map_err(|_| malformed())?;
        hash.update(length.to_be_bytes());
        hash.update(WORKLOAD_CLASS_ID.as_bytes());
        hash.update(WORKLOAD_CLASS_REVISION.to_be_bytes());
        for value in [
            self.limits.bytes,
            self.limits.base_work,
            self.limits.noise_work,
            self.limits.checked_total()?,
        ] {
            hash.update(value.to_be_bytes());
        }
        Ok(hash.finalize().into())
    }
    /// The caller owns bounded, admitted metadata storage for these JSON values.
    pub fn value(self) -> Result<Value> {
        Ok(Value::object([
            ("id", Value::str(WORKLOAD_CLASS_ID)),
            ("revision", Value::Int(i64::from(WORKLOAD_CLASS_REVISION))),
            ("workspaceBytes", integer(self.limits.bytes)?),
            ("baseWorkLimit", integer(self.limits.base_work)?),
            ("noiseWorkLimit", integer(self.limits.noise_work)?),
            ("totalWorkLimit", integer(self.limits.checked_total()?)?),
            ("sha256", Value::str(hex(&self.digest()?))),
        ]))
    }
    pub fn parse_browser(value: &Value) -> Result<Self> {
        shape(
            value,
            &[
                "id",
                "revision",
                "workspaceBytes",
                "baseWorkLimit",
                "noiseWorkLimit",
                "totalWorkLimit",
                "sha256",
            ],
        )?;
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(malformed)?;
        let revision = unsigned(value, "revision")?;
        if id != WORKLOAD_CLASS_ID || revision != u64::from(WORKLOAD_CLASS_REVISION) {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Unknown Noise workload contract",
            ));
        }
        let workload = Self::browser(NoiseLimits {
            bytes: unsigned(value, "workspaceBytes")?,
            base_work: unsigned(value, "baseWorkLimit")?,
            noise_work: unsigned(value, "noiseWorkLimit")?,
        })?;
        if unsigned(value, "totalWorkLimit")? != workload.limits.checked_total()?
            || value.get("sha256").and_then(Value::as_str)
                != Some(hex(&workload.digest()?).as_str())
        {
            return Err(malformed());
        }
        Ok(workload)
    }
    /// Validate typed quote facts against this exact binding, without allocating
    /// counters, compiling programs, spending work or publishing frame pixels.
    pub fn check(self, quote: WorkComponents) -> Result<()> {
        quote.checked_total()?;
        if quote.base > self.limits.base_work || quote.noise > self.limits.noise_work {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Noise quote exceeds declared workload",
            ));
        }
        Ok(())
    }
    pub fn quote_value(self, quote: WorkComponents) -> Result<Value> {
        self.check(quote)?;
        Ok(Value::object([
            ("workloadSha256", Value::str(hex(&self.digest()?))),
            ("base", integer(quote.base)?),
            ("noise", integer(quote.noise)?),
            ("total", integer(quote.checked_total()?)?),
        ]))
    }
    pub fn parse_quote(self, value: &Value) -> Result<WorkComponents> {
        shape(value, &["workloadSha256", "base", "noise", "total"])?;
        let quote = WorkComponents {
            base: unsigned(value, "base")?,
            noise: unsigned(value, "noise")?,
        };
        self.check(quote)?;
        if unsigned(value, "total")? != quote.checked_total()?
            || value.get("workloadSha256").and_then(Value::as_str)
                != Some(hex(&self.digest()?).as_str())
        {
            return Err(malformed());
        }
        Ok(quote)
    }
}
fn integer(value: u64) -> Result<Value> {
    i64::try_from(value)
        .map(Value::Int)
        .map_err(|_| malformed())
}
fn unsigned(value: &Value, key: &str) -> Result<u64> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(malformed)
}
fn shape(value: &Value, keys: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or_else(malformed)?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(malformed());
    }
    Ok(())
}
fn malformed() -> Error {
    Error::new(ErrorCode::InputSchema, "Malformed Noise workload binding")
}
#[cfg(test)]
mod tests;
