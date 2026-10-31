//! Explicit 160-u64/u128 exact backend. No runtime selection occurs here.
//! Work units count u64 limbs; they are not reference u32 units or time.
use super::{LevelsRgbV1, Result, engine, word};
#[derive(Debug, Default)]
#[repr(transparent)]
pub struct Scratch {
    inner: engine::Workspace<word::U64, 160>,
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
pub fn quote(p: LevelsRgbV1) -> Quote {
    Quote {
        scratch_bytes: std::mem::size_of::<Scratch>() as u64,
        work: engine::quote_work::<word::U64>(p),
    }
}
/// Exact full-table publication; caller-owned scratch/output and typed u64 limits.
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
