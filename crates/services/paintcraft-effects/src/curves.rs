//! Standalone exact five-knot encoded-RGB Curves compiler, not a Descriptor.
//! Successful arithmetic/storage uses caller-owned buffers, checked i64 and no
//! heap, float, unsafe, cache or dynamic backend. Callback behavior and ordinary
//! allocating typed-error diagnostics are excluded from that statement.
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba, filters::CurvesRgbV1};
mod engine;

/// Caller-owned private candidate LUT. Partial compilation is not published.
#[repr(transparent)]
pub struct Scratch {
    table: [u8; 256],
}
impl Default for Scratch {
    fn default() -> Self {
        Self { table: [0; 256] }
    }
}
impl std::fmt::Debug for Scratch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CurvesScratch").finish_non_exhaustive()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub scratch_bytes: u64,
    pub work: u64,
}
/// Scratch is `size_of::<Scratch>()` only. Output and small fixed scalar
/// stack/control values are excluded; this is not an allocator or RSS bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quote {
    pub scratch_bytes: u64,
    /// Fixed modeled source-step ceiling:128 units/input,32768/table. These are
    /// neither measured cycles/time nor actual compiler primitive counts.
    pub work: u64,
}
impl From<Quote> for Limits {
    fn from(value: Quote) -> Self {
        Self {
            scratch_bytes: value.scratch_bytes,
            work: value.work,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkUsed {
    pub units: u64,
}
pub const fn quote(_parameters: CurvesRgbV1) -> Quote {
    Quote {
        scratch_bytes: std::mem::size_of::<Scratch>() as u64,
        work: engine::TABLE_WORK,
    }
}
/// Geometry and quota refusal precedes every callback/write. Initial, per-input
/// and final checks gate one final output copy. Any error leaves caller output
/// unchanged; private scratch is reusable and otherwise unspecified on error.
pub fn build_lut(
    parameters: CurvesRgbV1,
    scratch: &mut Scratch,
    output: &mut [u8],
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<WorkUsed> {
    engine::build(parameters, scratch, output, limits, check).map(|units| WorkUsed { units })
}
/// Lookup only: transform hidden encoded RGB too, preserving incoming alpha.
/// Per-filter coverage is a separate mix after this candidate byte rounding.
pub fn evaluate_pixel(table: &[u8; 256], incoming: Rgba) -> Rgba {
    Rgba([
        table[usize::from(incoming.0[0])],
        table[usize::from(incoming.0[1])],
        table[usize::from(incoming.0[2])],
        incoming.0[3],
    ])
}
fn resource(message: &'static str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
#[cfg(test)]
mod oracles;
#[cfg(test)]
mod tests;
