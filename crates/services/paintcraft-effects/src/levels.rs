//! Complete exact Levels LUT foundation, not a registered filter descriptor.
//! Successful LUT arithmetic/storage uses caller buffers, no heap or powf.
//! Caller callbacks and ordinary allocating typed-error diagnostics are excluded
//! from that storage claim; no shared error API or product parser is changed.
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba, filters::LevelsRgbV1};
mod engine;
mod integer;
pub mod u64;
mod word;
/// Explicit caller-owned reference u32 scratch; no automatic backend switch.
#[derive(Debug, Default)]
#[repr(transparent)]
pub struct Scratch {
    inner: engine::Workspace<word::U32, 320>,
}
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
fn resource(message: &'static str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
/// Original u32-word admission quote; its semantics and units are unchanged.
pub fn quote(p: LevelsRgbV1) -> Quote {
    Quote {
        scratch_bytes: std::mem::size_of::<Scratch>() as u64,
        work: engine::quote_work::<word::U32>(p),
    }
}
/// Explicit reference u32 kernel. Publishes only after the final successful check.
/// On any error the output remains unchanged; scratch is private/unspecified.
pub fn build_lut(
    p: LevelsRgbV1,
    scratch: &mut Scratch,
    output: &mut [u8],
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<WorkUsed> {
    engine::build_lut(
        p,
        &mut scratch.inner,
        output,
        limits.scratch_bytes,
        limits.work,
        check,
    )
    .map(|units| WorkUsed { units })
}
/// Table lookup preserves alpha and transforms RGB hidden under alpha zero.
/// This standalone helper grants no Descriptor, document or wire capability.
pub fn evaluate_pixel(table: &[u8; 256], pixel: Rgba) -> Rgba {
    let [r, g, b, a] = pixel.0;
    Rgba([
        table[usize::from(r)],
        table[usize::from(g)],
        table[usize::from(b)],
        a,
    ])
}
#[cfg(test)]
mod oracles;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod backends;

#[cfg(test)]
mod test_api;
