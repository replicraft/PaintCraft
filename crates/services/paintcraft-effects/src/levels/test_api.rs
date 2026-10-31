//! One test suite invokes either typed public API; no production unit conversion.
use super::*;
pub(super) trait Backend {
    type Scratch: Default;
    const NAME: &'static str;
    fn quote(p: LevelsRgbV1) -> Quote;
    fn build(
        p: LevelsRgbV1,
        s: &mut Self::Scratch,
        out: &mut [u8],
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<WorkUsed>;
    fn capacity_controls();
}
pub(super) struct Reference;
pub(super) struct Wide;
impl Backend for Reference {
    type Scratch = Scratch;
    const NAME: &'static str = "u32 reference units";
    fn quote(p: LevelsRgbV1) -> Quote {
        super::quote(p)
    }
    fn build(
        p: LevelsRgbV1,
        s: &mut Scratch,
        out: &mut [u8],
        l: Limits,
        c: &dyn Fn() -> Result<()>,
    ) -> Result<WorkUsed> {
        super::build_lut(p, s, out, l, c)
    }
    fn capacity_controls() {
        capacity::<word::U32, 320>();
    }
}
impl Backend for Wide {
    type Scratch = u64::Scratch;
    const NAME: &'static str = "u64 wide units";
    fn quote(p: LevelsRgbV1) -> Quote {
        let q = u64::quote(p);
        Quote {
            scratch_bytes: q.scratch_bytes,
            work: q.work,
        }
    }
    fn build(
        p: LevelsRgbV1,
        s: &mut u64::Scratch,
        out: &mut [u8],
        l: Limits,
        c: &dyn Fn() -> Result<()>,
    ) -> Result<WorkUsed> {
        // This test-only record is tagged by Backend::NAME; raw counts are never
        // compared across widths, summed together, or used as a speed metric.
        u64::build_lut(
            p,
            s,
            out,
            u64::Limits {
                scratch_bytes: l.scratch_bytes,
                work: l.work,
            },
            c,
        )
        .map(|v| WorkUsed { units: v.units })
    }
    fn capacity_controls() {
        capacity::<word::U64, 160>();
    }
}
fn capacity<W: word::Word, const N: usize>() {
    use std::cmp::Ordering;
    let mut scratch = engine::Workspace::<W, N>::default();
    let mut budget = engine::Budget {
        used: 0,
        limit: std::primitive::u64::MAX,
        check: &|| Ok(()),
    };
    scratch.left.pow(0, 999, &mut budget).unwrap();
    assert_eq!(
        scratch
            .left
            .compare(&integer::Integer::<W, N>::zero(), &mut budget)
            .unwrap(),
        Ordering::Equal
    );
    scratch.left.pow(1, 999, &mut budget).unwrap();
    scratch.right.pow(2, 0, &mut budget).unwrap();
    assert_eq!(
        scratch.left.compare(&scratch.right, &mut budget).unwrap(),
        Ordering::Equal
    );
    scratch.left.pow(2, 10239, &mut budget).unwrap();
    scratch.right.pow(2, 1, &mut budget).unwrap();
    assert_eq!(
        scratch
            .temporary
            .product(&scratch.left, &scratch.right, &mut budget)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        scratch.left.pow(2, 10240, &mut budget).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}
