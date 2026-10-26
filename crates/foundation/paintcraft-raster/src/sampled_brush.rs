//! Immutable sampled tips and deterministic static-mask stroke coverage.
//! This is PaintCraft's explicit static-tip policy, not Photoshop brush parity.
mod coverage;
mod tip;
pub use coverage::{Coverage, coverage};
pub use tip::SampledTip;

pub const TIP_PIXELS_MAX: usize = 4_194_304;
pub const TIP_SIDE_MAX: u32 = 16384;
pub const COVERAGE_PIXELS_MAX: usize = 4_194_304;
pub const SPANS_MAX: usize = 262_144;
pub const DABS_MAX: usize = 65_536;
pub const DAB_PIXELS_MAX: u64 = 67_108_864;
pub const POINTS_MAX: usize = 65_536;
pub const SPACING_Q8_MAX: u32 = 16_777_216;

use paintcraft_core::types::{Error, ErrorCode, Result};
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
fn limit(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
