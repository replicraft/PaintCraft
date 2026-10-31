//! Explicit 160-u64/u128 backend; no automatic selection or speed claim.
//! Work units count wide-word operations and differ from the reference units.
use super::{ExposureSrgbV1, Result, engine, word};
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
pub fn quote(p: ExposureSrgbV1) -> Quote {
    Quote {
        scratch_bytes: std::mem::size_of::<Scratch>() as u64,
        work: engine::quote_work::<word::U64>(p),
    }
}
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
