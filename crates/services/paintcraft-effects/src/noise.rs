//! Standalone SOURCE-coordinate encoded-RGB noise; no Descriptor registration.
//! Successful arithmetic uses bounded scalar stack locals and caller row output,
//! not heap, LUT ownership or mutable PRNG state. Typed diagnostics/callbacks may
//! allocate; scalar/library stack, static constants and caller output are excluded
//! from scratch_bytes. Gaussian's pinned binary64 graph is NOT ideal-real rounding.
use paintcraft_core::types::{
    Error, ErrorCode, Result, Rgba,
    coordinates::SourcePixel,
    filters::{AddNoiseRgbV1, NoiseDistribution},
};
mod engine;
/// Reviewed conservative source-action allowances; not instructions or timing.
const UNIFORM_WORK: u64 = 512;
const GAUSSIAN_WORK: u64 = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub scratch_bytes: u64,
    pub work: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quote {
    pub scratch_bytes: u64,
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
/// Conservative COLOR tuple allowance also applies to monochrome/amount0.
/// Source-action units are not instructions, CPU time or a benchmark claim.
pub fn quote(policy: AddNoiseRgbV1, length: usize) -> Result<Quote> {
    let unit = match policy.distribution() {
        NoiseDistribution::Uniform => UNIFORM_WORK,
        NoiseDistribution::Gaussian => GAUSSIAN_WORK,
    };
    let n = u64::try_from(length).map_err(|_| resource("Noise row length exceeds work domain"))?;
    Ok(Quote {
        scratch_bytes: 0,
        work: n
            .checked_mul(unit)
            .ok_or_else(|| resource("Noise row work overflow"))?,
    })
}
/// Coordinates MUST be SOURCE-local. Layer bounds/placement are the caller's
/// separate proof. Hidden RGB changes; alpha is exact; mix instance A8 once AFTER.
pub fn evaluate_pixel(policy: AddNoiseRgbV1, point: SourcePixel, incoming: Rgba) -> Result<Rgba> {
    engine::pixel(policy, point, incoming)
}
/// Geometry, checked coordinate end<=2^32 and quota precede callbacks/writes.
/// The last valid pixel is x=u32::MAX. Empty rows run initial+final checks.
/// Then each <=64 tuple chunk checks cancellation; final check follows writes.
/// On any cancellation discard PRIVATE partial output; never publish it. Safe
/// source/output borrows are disjoint. No source pixel is mutated.
pub fn evaluate_row(
    policy: AddNoiseRgbV1,
    origin: SourcePixel,
    source: &[Rgba],
    output: &mut [Rgba],
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<WorkUsed> {
    if source.len() != output.len() {
        return Err(schema("Noise row lengths differ"));
    }
    let length =
        u64::try_from(source.len()).map_err(|_| schema("Noise row coordinates overflow"))?;
    if u64::from(origin.x())
        .checked_add(length)
        .is_none_or(|end| end > 1u64 << 32)
    {
        return Err(schema(
            "Noise row exceeds unsigned32-bit source coordinates",
        ));
    }
    let q = quote(policy, source.len())?;
    if limits.work < q.work || limits.scratch_bytes < q.scratch_bytes {
        return Err(resource("Noise row exceeds admitted limits"));
    }
    check()?;
    for (chunk, (input, output)) in source.chunks(64).zip(output.chunks_mut(64)).enumerate() {
        check()?;
        for (offset, (incoming, slot)) in input.iter().zip(output).enumerate() {
            // The complete extent was checked before callbacks; these checked
            // conversions cannot fail for an admitted row, even on Wasm32.
            let index = chunk
                .checked_mul(64)
                .and_then(|v| v.checked_add(offset))
                .and_then(|v| u32::try_from(v).ok())
                .ok_or_else(|| schema("Noise row index overflow"))?;
            let x = origin
                .x()
                .checked_add(index)
                .ok_or_else(|| schema("Noise source coordinate overflow"))?;
            *slot = evaluate_pixel(policy, SourcePixel::new(x, origin.y()), *incoming)?;
        }
    }
    check()?;
    Ok(WorkUsed { units: q.work })
}
fn schema(message: &'static str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
fn resource(message: &'static str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod oracles;
