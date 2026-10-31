//! Immutable Curves LUT in the caller's existing logical payload/work realm.
use super::{WorkspaceAdmission, limit};
use paintcraft_core::types::{Result, Rgba, filters::CurvesRgbV1, resources::BudgetAccount};
use paintcraft_effects::curves;
pub const CURVES_TABLE_BYTES: u64 = 256;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurvesQuote {
    pub peak_bytes: u64,
    pub retained_bytes: u64,
    pub scratch_bytes: u64,
    pub compile_work: u64,
}
pub fn quote_curves(policy: CurvesRgbV1) -> CurvesQuote {
    let q = curves::quote(policy);
    CurvesQuote {
        peak_bytes: q.scratch_bytes + CURVES_TABLE_BYTES,
        retained_bytes: CURVES_TABLE_BYTES,
        scratch_bytes: q.scratch_bytes,
        compile_work: q.work,
    }
}
#[derive(Debug)]
pub struct PreparedCurves {
    policy: CurvesRgbV1,
    table: [u8; 256],
    actual_compile_work: curves::WorkUsed,
    _charge: BudgetAccount,
}
impl PreparedCurves {
    pub fn new(
        policy: CurvesRgbV1,
        admission: &WorkspaceAdmission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let q = quote_curves(policy);
        if q.peak_bytes > admission.bytes_limit() || q.compile_work > admission.work_limit() {
            return Err(limit("Prepared Curves exceeds byte/work allowance"));
        }
        check()?;
        admission.reserve_work(q.compile_work)?;
        let mut scratch = admission.scope().account();
        scratch.resize(q.scratch_bytes)?;
        let mut charge = admission.scope().account();
        charge.resize(q.retained_bytes)?;
        check()?;
        let mut table = [0; 256];
        let used = compile(policy, &mut table, check)?;
        drop(scratch);
        check()?;
        Ok(Self {
            policy,
            table,
            actual_compile_work: used,
            _charge: charge,
        })
    }
    pub fn policy(&self) -> CurvesRgbV1 {
        self.policy
    }
    pub fn actual_compile_work(&self) -> curves::WorkUsed {
        self.actual_compile_work
    }
    pub(crate) fn evaluate_pixel(&self, pixel: Rgba) -> Rgba {
        curves::evaluate_pixel(&self.table, pixel)
    }
}
#[inline(never)]
fn compile(
    policy: CurvesRgbV1,
    table: &mut [u8; 256],
    check: &dyn Fn() -> Result<()>,
) -> Result<curves::WorkUsed> {
    let mut scratch = curves::Scratch::default();
    curves::build_lut(
        policy,
        &mut scratch,
        table,
        curves::quote(policy).into(),
        check,
    )
}
