//! Test-only normalized records remain labeled by backend; no unit conversion.
use super::*;
pub(super) trait Backend {
    type Scratch: Default;
    const NAME: &'static str;
    const WORK_BOUND: u64;
    fn quote(p: ExposureSrgbV1) -> Quote;
    fn build(
        p: ExposureSrgbV1,
        s: &mut Self::Scratch,
        out: &mut [u8],
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<WorkUsed>;
}
pub(super) struct Reference;
pub(super) struct Wide;
impl Backend for Reference {
    type Scratch = Scratch;
    const NAME: &'static str = "u32 reference";
    const WORK_BOUND: u64 = 26_000_000;
    fn quote(p: ExposureSrgbV1) -> Quote {
        super::quote(p)
    }
    fn build(
        p: ExposureSrgbV1,
        s: &mut Self::Scratch,
        out: &mut [u8],
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<WorkUsed> {
        super::build_lut(p, s, out, limits, check)
    }
}
impl Backend for Wide {
    type Scratch = u64::Scratch;
    const NAME: &'static str = "u64 wide";
    const WORK_BOUND: u64 = 11_000_000;
    fn quote(p: ExposureSrgbV1) -> Quote {
        let q = u64::quote(p);
        Quote {
            scratch_bytes: q.scratch_bytes,
            work: q.work,
        }
    }
    fn build(
        p: ExposureSrgbV1,
        s: &mut Self::Scratch,
        out: &mut [u8],
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<WorkUsed> {
        u64::build_lut(
            p,
            s,
            out,
            u64::Limits {
                scratch_bytes: limits.scratch_bytes,
                work: limits.work,
            },
            check,
        )
        .map(|v| WorkUsed { units: v.units })
    }
}
