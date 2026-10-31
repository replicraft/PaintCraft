//! Explicit reference-u32 Exposure ownership. No Levels selector is reused.
use super::{WorkspaceAdmission, limit};
use paintcraft_core::types::{Result, Rgba, filters::ExposureSrgbV1, resources::BudgetAccount};
use paintcraft_effects::exposure;
pub const EXPOSURE_TABLE_BYTES: u64 = 256;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExposureQuote {
    pub peak_bytes: u64,
    pub retained_bytes: u64,
    pub scratch_bytes: u64,
    /// Reference-u32 conservative allowance, not wide units or elapsed time.
    pub compile_work: u64,
}
pub fn quote_exposure(policy: ExposureSrgbV1) -> ExposureQuote {
    let q = exposure::quote(policy);
    ExposureQuote {
        peak_bytes: q.scratch_bytes + EXPOSURE_TABLE_BYTES,
        retained_bytes: EXPOSURE_TABLE_BYTES,
        scratch_bytes: q.scratch_bytes,
        compile_work: q.work,
    }
}
/// An immutable complete-key table. Logical payload excludes allocator/Arc/Rust
/// bookkeeping consistently with PreparedLevels; no admission factory is kept.
#[derive(Debug)]
pub struct PreparedExposure {
    policy: ExposureSrgbV1,
    table: [u8; 256],
    actual_compile_work: exposure::WorkUsed,
    _charge: BudgetAccount,
}
impl PreparedExposure {
    pub fn new(
        policy: ExposureSrgbV1,
        admission: &WorkspaceAdmission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let q = quote_exposure(policy);
        if q.peak_bytes > admission.bytes_limit() || q.compile_work > admission.work_limit() {
            return Err(limit("Prepared Exposure exceeds its byte/work allowance"));
        }
        check()?;
        admission.reserve_work(q.compile_work)?;
        let mut scratch_charge = admission.scope().account();
        scratch_charge.resize(q.scratch_bytes)?;
        let mut table_charge = admission.scope().account();
        table_charge.resize(q.retained_bytes)?;
        check()?;
        let mut table = [0; 256];
        let used = compile(policy, &mut table, check)?;
        drop(scratch_charge);
        check()?;
        Ok(Self {
            policy,
            table,
            actual_compile_work: used,
            _charge: table_charge,
        })
    }
    pub fn policy(&self) -> ExposureSrgbV1 {
        self.policy
    }
    /// Explicit reference-u32 raw work; no backend selection is implied.
    pub fn actual_compile_work(&self) -> exposure::WorkUsed {
        self.actual_compile_work
    }
    pub(crate) fn evaluate_pixel(&self, incoming: Rgba) -> Rgba {
        exposure::evaluate_pixel(&self.table, incoming)
    }
}
// Construct only the selected reference scratch, outside callers' tile loops.
#[inline(never)]
fn compile(
    policy: ExposureSrgbV1,
    table: &mut [u8; 256],
    check: &dyn Fn() -> Result<()>,
) -> Result<exposure::WorkUsed> {
    let mut scratch = exposure::Scratch::default();
    exposure::build_lut(
        policy,
        &mut scratch,
        table,
        exposure::quote(policy).into(),
        check,
    )
}
