//! Explicit scoped-Noise MSP1/MSP2 consumer; old4..7 structs and wire remain closed.
use super::*;
use export::noise_workload::Workload;
use paintcraft_render::noise_work::{
    NoiseAdmission, NoiseJobPolicy, NoiseLimits, capture_noise_or_current, with_noise_workspace,
};
mod plan;
mod request;
mod response;
#[cfg(test)]
mod tests;
mod worker;
pub use plan::NoiseSourcePlan;
pub use request::Header;
pub use response::{Frame, decode_response};
pub use worker::execute;
pub const PROTOCOL_VERSION: u16 = 8;
pub const REQUEST_HEADER: usize = 704;
pub const RESPONSE_HEADER: usize = 864;
pub const RESPONSE_BYTES_MAX: usize =
    RESPONSE_HEADER + ENCODED_BYTES_MAX + PREVIEW_BYTES_MAX + 4096;
pub const EXPORT_VARIANT_V1: &str = "web-msp1-uncompressed-noise-worker-v8";
pub const EXPORT_VARIANT_V2: &str = "web-msp2-row-rle-noise-worker-v8";
pub fn export_variant(version: Version) -> &'static str {
    match version {
        Version::UncompressedV1 => EXPORT_VARIANT_V1,
        Version::RowRleV2 => EXPORT_VARIANT_V2,
    }
}
fn version_byte(version: Version) -> u8 {
    match version {
        Version::UncompressedV1 => 1,
        Version::RowRleV2 => 2,
    }
}
fn settings(version: Version) -> [u8; 8] {
    [version_byte(version), 0, 0, 0, 0, 0, 0, 0]
}
fn decode_settings(bytes: [u8; 8], size: (u32, u32)) -> Result<Version> {
    if bytes[1..].iter().any(|b| *b != 0) {
        return Err(invalid("MSP8 settings reserved"));
    }
    let version = match bytes[0] {
        1 => Version::UncompressedV1,
        2 => Version::RowRleV2,
        _ => return Err(invalid("MSP8 encoding version")),
    };
    RgbaLayout::new(version, size.0, size.1)?;
    Ok(version)
}
/// Render workspace and codec packed/output allocations are disjoint phases.
fn encode_extra(version: Version, size: (u32, u32)) -> Result<u64> {
    let layout = RgbaLayout::new(version, size.0, size.1)?;
    let packed = layout.packed_bytes() as u64;
    let encoded = match version {
        Version::UncompressedV1 => packed
            .checked_add(32)
            .ok_or_else(|| resource("MSP8 encoded size"))?,
        Version::RowRleV2 => ENCODED_BYTES_MAX as u64,
    };
    if encoded > ENCODED_BYTES_MAX as u64 {
        return Err(resource("MSP8 encoded allowance"));
    }
    packed
        .checked_add(encoded)
        .ok_or_else(|| resource("MSP8 encode phase"))
}
const NOISE_SOURCE_LIMITS: Limits = Limits {
    label: "Browser MSP",
    ..SOURCE_LIMITS
};
pub const PROFILE: &str = "web-msp-pcnd32mib-4mp-scoped-noise-v8";
/// New retained Header values plus wire copies, outside the appearance workspace.
fn control_bytes() -> Result<u64> {
    (std::mem::size_of::<Header>() as u64)
        .checked_mul(2)
        .and_then(|n| n.checked_add((REQUEST_HEADER + RESPONSE_HEADER) as u64))
        .ok_or_else(|| resource("MSP8 control size"))
}
pub fn workload_bytes(workload: Workload) -> Result<[u8; 64]> {
    let l = workload.limits();
    let mut b = [0; 64];
    for (at, n) in [
        (0, l.bytes),
        (8, l.base_work),
        (16, l.noise_work),
        (24, l.checked_total()?),
    ] {
        b[at..at + 8].copy_from_slice(&n.to_be_bytes());
    }
    b[32..].copy_from_slice(&workload.digest()?);
    Ok(b)
}
pub fn read_workload(b: &[u8]) -> Result<Workload> {
    if b.len() != 64 {
        return Err(invalid("MSP8 workload extent"));
    }
    let n = |at| u64::from_be_bytes(b[at..at + 8].try_into().unwrap());
    let value = Workload::browser(NoiseLimits {
        bytes: n(0),
        base_work: n(8),
        noise_work: n(16),
    })?;
    if n(24) != value.limits().checked_total()? || b[32..] != value.digest()? {
        return Err(invalid("MSP8 workload binding"));
    }
    Ok(value)
}

/// Fixed endpoint policy declaration. This descriptive value grants no counter.
pub fn default_workload() -> Result<Workload> {
    Workload::browser(NoiseJobPolicy::BrowserV1.max_limits())
}
