//! Explicit paired job authority for the separately negotiated Noise workload.
//! Existing workspace/quote types retain their layouts and legacy semantics.
use paintcraft_core::types::{Error, ErrorCode, Result};
mod admission;
mod binding;
mod context;
mod owner;
mod quote;
mod work;
pub use admission::{NoiseAdmission, NoiseControlQuote, NoiseJobPolicy, NoiseLimits};
pub use context::{
    capture_current_noise_workspace, capture_noise_or_current, with_noise_workspace,
};
pub(crate) use context::{captured, current, current_for, required};
pub use quote::{ScopedLayerQuote, quote_direct, quote_layer};
pub(crate) use quote::{check_plan, preflight, reserve_plan};
pub use work::WorkComponents;
pub const WORKLOAD_CLASS_ID: &str = "noise-source-work-v1";
pub const WORKLOAD_CLASS_REVISION: u32 = 1;
pub const NOISE_WORK_MAX: u64 = 1 << 35;
pub const BYTE_MAX: u64 = 64 << 20;
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Scoped Noise job exceeds caller admission",
    )
}
pub(crate) fn missing() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Noise appearance requires a matching scoped job admission",
    )
}
#[cfg(test)]
mod tests;

pub use context::capture_required_noise_workspace;
