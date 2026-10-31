//! Standalone exact whole-RGB HSL adjustment, not a registered Descriptor.
//! Successful arithmetic/storage uses checked i64, fixed scalar stack locals
//! and caller-owned row output; no heap, LUT, float, unsafe or cache. Callbacks
//! and ordinary allocating typed-error diagnostics are excluded from that claim.
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba, filters::HueSaturationRgbV1};
mod engine;
const WORK_PER_PIXEL: u64 = 128;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub scratch_bytes: u64,
    pub work: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quote {
    /// No caller scratch; scalar stack bookkeeping and caller output excluded.
    pub scratch_bytes: u64,
    /// Fixed conservative source-action units/tuple, not instructions or time.
    pub work: u64,
}
impl From<Quote> for Limits {
    fn from(q: Quote) -> Self {
        Self {
            scratch_bytes: q.scratch_bytes,
            work: q.work,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkUsed {
    pub units: u64,
}
pub fn quote(length: usize) -> Result<Quote> {
    let n = u64::try_from(length).map_err(|_| resource("Hue/Saturation row length overflow"))?;
    Ok(Quote {
        scratch_bytes: 0,
        work: n
            .checked_mul(WORK_PER_PIXEL)
            .ok_or_else(|| resource("Hue/Saturation row work overflow"))?,
    })
}
/// Compute one complete candidate from ORIGINAL RGB, with incoming alpha exact.
/// Hidden RGB is transformed. Apply any instance coverage once afterward.
pub fn evaluate_pixel(policy: HueSaturationRgbV1, incoming: Rgba) -> Result<Rgba> {
    engine::pixel(policy, incoming)
}
/// Geometry/quota refusal precedes all callbacks/writes. Then initial, per<=64
/// tuple chunk and final checks. Empty rows run initial+final. Mid/final cancel
/// may leave private output partially/fully written: discard it, never publish.
/// Safe Rust source/output borrows must be disjoint; input is never mutated.
pub fn evaluate_row(
    policy: HueSaturationRgbV1,
    source: &[Rgba],
    output: &mut [Rgba],
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<WorkUsed> {
    if source.len() != output.len() {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Hue/Saturation row lengths differ",
        ));
    }
    let q = quote(source.len())?;
    if limits.work < q.work || limits.scratch_bytes < q.scratch_bytes {
        return Err(resource("Hue/Saturation row exceeds admitted limits"));
    }
    check()?;
    for (src, dst) in source.chunks(64).zip(output.chunks_mut(64)) {
        check()?;
        for (incoming, slot) in src.iter().zip(dst) {
            *slot = evaluate_pixel(policy, *incoming)?;
        }
    }
    check()?;
    Ok(WorkUsed { units: q.work })
}
fn resource(message: &'static str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
#[cfg(test)]
mod oracles;
#[cfg(test)]
mod tests;
