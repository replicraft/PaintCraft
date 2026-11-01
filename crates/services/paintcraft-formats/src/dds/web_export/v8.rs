//! Explicit scoped-Noise DDS consumer; old4..7 structs and wire remain closed.
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
pub const EXPORT_VARIANT: &str = "web-dds-texture-worker-v8";
pub const PROFILE: &str = "web-dds-pcnd32mib-4mp-scoped-noise-v8";
/// New retained Header values plus wire copies, outside the appearance workspace.
fn control_bytes() -> Result<u64> {
    (std::mem::size_of::<Header>() as u64)
        .checked_mul(2)
        .and_then(|n| n.checked_add((REQUEST_HEADER + RESPONSE_HEADER) as u64))
        .ok_or_else(|| resource("DDS8 control size"))
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
        return Err(invalid("DDS8 workload extent"));
    }
    let n = |at| u64::from_be_bytes(b[at..at + 8].try_into().unwrap());
    let value = Workload::browser(NoiseLimits {
        bytes: n(0),
        base_work: n(8),
        noise_work: n(16),
    })?;
    if n(24) != value.limits().checked_total()? || b[32..] != value.digest()? {
        return Err(invalid("DDS8 workload binding"));
    }
    Ok(value)
}

/// Fixed endpoint policy declaration. This descriptive value grants no counter.
pub fn default_workload() -> Result<Workload> {
    Workload::browser(NoiseJobPolicy::BrowserV1.max_limits())
}
