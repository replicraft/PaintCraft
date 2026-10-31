//! Standalone exact linear-light sRGB Exposure LUT compiler, not a Descriptor.
//! Successful arithmetic/storage uses caller-owned buffers and no heap or powf.
//! Callback behavior and ordinary allocating typed-error diagnostics are excluded.
use crate::exact::{integer, word};
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba, filters::ExposureSrgbV1};
mod engine;
pub mod u64;
#[derive(Debug, Default)]
#[repr(transparent)]
pub struct Scratch {
    inner: engine::Workspace<word::U32, 320>,
}
/// Reference u32-word operation allowance, not elapsed time or wide-word units.
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
fn resource(message: &'static str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
pub fn quote(p: ExposureSrgbV1) -> Quote {
    Quote {
        scratch_bytes: std::mem::size_of::<Scratch>() as u64,
        work: engine::quote_work::<word::U32>(p),
    }
}
/// Publish all256 entries only after a final successful check. Any typed error
/// leaves output unchanged; scratch contents are private and unspecified.
pub fn build_lut(
    p: ExposureSrgbV1,
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
/// Table lookup changes hidden RGB, preserving alpha; no mask is applied here.
pub fn evaluate_pixel(table: &[u8; 256], p: Rgba) -> Rgba {
    Rgba([
        table[usize::from(p.0[0])],
        table[usize::from(p.0[1])],
        table[usize::from(p.0[2])],
        p.0[3],
    ])
}
#[cfg(test)]
mod oracles;
#[cfg(test)]
mod test_api;
#[cfg(test)]
mod tests;
