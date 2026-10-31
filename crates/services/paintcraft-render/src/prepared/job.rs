//! Eager immutable job-local reuse. No global cache or compilation on lookup.
use super::{PreparedLevels, WorkspaceAdmission, limit, quote_levels};
use paintcraft_core::types::{Result, filters::LevelsRgbV1, resources::BudgetAccount};
use std::sync::Arc;

/// One frozen job owns every unique complete typed policy exactly once.
/// Callers may share an Arc of the whole job; borrowed programs cannot outlive it.
/// The job retains no admission factory or unused reservation capacity.
///
/// Logical payload includes actual Arc-pointer vector capacity and each existing
/// 256-byte table account. Allocator/Arc/Rust bookkeeping retains the baseline
/// exclusion. Bounded index bookkeeping is not LUT compile-work or elapsed time.
#[derive(Debug)]
pub struct PreparedLevelsJob {
    // Programs and pointer storage end before their index declaration releases.
    programs: Vec<Arc<PreparedLevels>>,
    _index_charge: BudgetAccount,
}

/// Requested exact pointer capacity plus unique compile/peak payload in first
/// occurrence order. Actual allocator capacity is reconciled before compilation.
/// This describes a bounded plan; constructing a quote grants no admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelsJobQuote {
    pub unique_programs: usize,
    pub index_bytes: u64,
    pub peak_bytes: u64,
    pub compile_work: u64,
}
fn pointer_bytes(capacity: usize) -> Result<u64> {
    u64::try_from(capacity)
        .ok()
        .and_then(|n| n.checked_mul(std::mem::size_of::<Arc<PreparedLevels>>() as u64))
        .ok_or_else(|| limit("Prepared Levels job index size overflows"))
}
pub fn quote_levels_job(policies: &[LevelsRgbV1]) -> Result<LevelsJobQuote> {
    if policies.len() > PreparedLevelsJob::MAX_PROGRAMS {
        return Err(limit(
            "Prepared Levels job input count exceeds its host limit",
        ));
    }
    let mut unique = 0usize;
    let mut retained = 0u64;
    let mut peak = 0u64;
    let mut work = 0u64;
    // <=256^2 complete-policy comparisons, no staging allocation or hash keys.
    // Equality preserves all five fields even when two LUTs would be identical.
    for (index, policy) in policies.iter().enumerate() {
        if policies[..index].contains(policy) {
            continue;
        }
        let quote = quote_levels(*policy);
        peak = peak.max(
            retained
                .checked_add(quote.peak_bytes)
                .ok_or_else(|| limit("Prepared Levels job peak overflows"))?,
        );
        retained = retained
            .checked_add(quote.retained_bytes)
            .ok_or_else(|| limit("Prepared Levels job retained size overflows"))?;
        work = work
            .checked_add(quote.compile_work)
            .ok_or_else(|| limit("Prepared Levels job compile work overflows"))?;
        unique += 1;
    }
    let index_bytes = pointer_bytes(unique)?;
    Ok(LevelsJobQuote {
        unique_programs: unique,
        index_bytes,
        peak_bytes: peak
            .checked_add(index_bytes)
            .ok_or_else(|| limit("Prepared Levels job index/peak sum overflows"))?,
        compile_work: work,
    })
}
impl PreparedLevelsJob {
    /// A first-slice host bound on both submitted and unique policies. This is
    /// not a restriction on documents, which do not yet store Levels policies.
    pub const MAX_PROGRAMS: usize = 256;

    /// Prepare privately and return one immutable job only after its final check.
    /// Aggregate peak/work is checked before any allocation or compilation.
    /// Each unique program uses the SAME cumulative admission; no work is reset.
    /// Concurrent spending by a factory clone can still refuse a later private
    /// program. Failed/cancelled compilation work remains spent, as in PreparedLevels::new().
    pub fn new(
        policies: &[LevelsRgbV1],
        admission: WorkspaceAdmission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        Self::new_with_tail(policies, admission, check, 0)
    }
    pub(super) fn new_with_tail(
        policies: &[LevelsRgbV1],
        admission: WorkspaceAdmission,
        check: &dyn Fn() -> Result<()>,
        following_peak: u64,
    ) -> Result<Self> {
        let plan = quote_levels_job(policies)?;
        Self::new_with_capacity(
            policies,
            admission,
            check,
            following_peak,
            plan,
            plan.unique_programs,
        )
    }
    // Private allocation seam: production always requests exactly the plan. A
    // test can deliberately request a larger real Vec to exercise reconciliation
    // without depending on accidental allocator over-allocation or global hooks.
    fn new_with_capacity(
        policies: &[LevelsRgbV1],
        admission: WorkspaceAdmission,
        check: &dyn Fn() -> Result<()>,
        following_peak: u64,
        plan: LevelsJobQuote,
        requested_capacity: usize,
    ) -> Result<Self> {
        if plan.peak_bytes > admission.bytes_limit()
            || admission
                .work_used()
                .checked_add(plan.compile_work)
                .is_none_or(|work| work > admission.work_limit())
        {
            return Err(limit("Prepared Levels job exceeds its aggregate allowance"));
        }
        check()?;
        let mut charge = admission.scope().account();
        charge.resize(plan.index_bytes)?;
        let mut programs = Vec::new();
        programs
            .try_reserve_exact(requested_capacity)
            .map_err(|_| limit("Prepared Levels job index allocation failed"))?;
        // Rust may grant more capacity than requested. Reconcile privately,
        // before compiling any program or publishing any borrowed lookup.
        let actual_index = pointer_bytes(programs.capacity())?;
        let actual_peak = plan
            .peak_bytes
            .checked_sub(plan.index_bytes)
            .and_then(|peak| peak.checked_add(actual_index))
            .ok_or_else(|| limit("Prepared Levels job actual peak overflows"))?;
        if following_peak != 0 {
            // Price every actual pointer slot while all captured/index payloads
            // coexist, before checking the following family or compiling.
            charge.resize(actual_index)?;
            check_following_capacity(&plan, actual_index, following_peak, admission.bytes_limit())?;
        }
        if actual_peak > admission.bytes_limit() {
            return Err(limit(
                "Prepared Levels job actual index exceeds its allowance",
            ));
        }
        charge.resize(actual_index)?;
        let mut job = Self {
            programs,
            _index_charge: charge,
        };
        for policy in policies {
            check()?;
            if job.get(*policy).is_some() {
                continue;
            }
            if job.programs.len() == job.programs.capacity() {
                return Err(limit("Prepared Levels job exhausted its admitted index"));
            }
            let program = PreparedLevels::new(*policy, &admission, check)?;
            check()?;
            job.programs.push(Arc::new(program));
        }
        debug_assert_eq!(job.programs.len(), plan.unique_programs);
        check()?;
        Ok(job)
    }

    /// Bounded read-only lookup. Never allocates, compiles or spends job work.
    pub fn get(&self, policy: LevelsRgbV1) -> Option<&PreparedLevels> {
        self.programs
            .iter()
            .find(|program| program.policy() == policy)
            .map(Arc::as_ref)
    }
    /// Actual retained logical index/table payload; allocator/Arc bookkeeping is
    /// excluded as in the quote. Construction proved this sum fits admission.
    pub fn retained_bytes(&self) -> u64 {
        self._index_charge.bytes() + self.programs.len() as u64 * super::LEVELS_TABLE_BYTES
    }
    pub fn len(&self) -> usize {
        self.programs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.programs.is_empty()
    }
}
#[cfg(test)]
#[path = "job_tests.rs"]
mod tests;

fn check_following_capacity(
    plan: &LevelsJobQuote,
    actual_index: u64,
    tail: u64,
    limit_bytes: u64,
) -> Result<()> {
    let retained = actual_index
        .checked_add(plan.unique_programs as u64 * super::LEVELS_TABLE_BYTES)
        .ok_or_else(|| limit("Prepared Levels actual following retention overflows"))?;
    if retained.checked_add(tail).is_none_or(|n| n > limit_bytes) {
        return Err(limit(
            "Prepared Levels actual following phase exceeds allowance",
        ));
    }
    Ok(())
}
