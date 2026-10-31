//! Standalone whole-tuple encoded-RGB Vibrance. No Descriptor registration.
//! Successful arithmetic uses bounded scalar stack locals and caller row output,
//! with no heap, LUT, float or unsafe. Typed-error diagnostics and callbacks may
//! allocate. Resource figures exclude scalar stack bookkeeping and caller output;
//! they are not physical RSS bounds or CPU/time measurements.
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba, filters::VibranceRgbV1};
mod engine;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub scratch_bytes: u64,
    pub work: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quote {
    /// No caller scratch buffer. Bounded scalar call-stack locals are excluded.
    pub scratch_bytes: u64,
    /// Conservative modeled64 units/tuple, even for identity and grayscale.
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
/// Checked geometry-independent work admission; this quote grants no execution.
pub fn quote(length: usize) -> Result<Quote> {
    let length = u64::try_from(length).map_err(|_| resource("Vibrance row length overflow"))?;
    let work = length
        .checked_mul(64)
        .ok_or_else(|| resource("Vibrance row work overflow"))?;
    Ok(Quote {
        scratch_bytes: 0,
        work,
    })
}
/// One whole candidate, including hidden RGB, with exact incoming alpha.
/// Coverage must be mixed separately once after this candidate rounding.
pub fn evaluate_pixel(policy: VibranceRgbV1, incoming: Rgba) -> Result<Rgba> {
    engine::pixel(policy, incoming)
}
/// Geometry/quota refusal precedes every callback/write. Then initial, per<=64
/// tuple chunk, and final checks run. Empty rows have initial and final checks.
/// Cancellation during/following work may leave private output partially or
/// fully written: discard it; publication is the caller's separate operation.
/// Safe Rust requires disjoint source/output borrows. No source is mutated.
pub fn evaluate_row(
    policy: VibranceRgbV1,
    source: &[Rgba],
    output: &mut [Rgba],
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<WorkUsed> {
    if source.len() != output.len() {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Vibrance row lengths differ",
        ));
    }
    let q = quote(source.len())?;
    if limits.work < q.work || limits.scratch_bytes < q.scratch_bytes {
        return Err(resource("Vibrance row exceeds admitted limits"));
    }
    check()?;
    for (input, output) in source.chunks(64).zip(output.chunks_mut(64)) {
        check()?;
        for (incoming, slot) in input.iter().zip(output) {
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
