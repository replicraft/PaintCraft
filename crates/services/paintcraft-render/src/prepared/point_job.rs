//! Frozen cross-family ownership. No global cache or compile-on-lookup exists.
use super::{
    PreparedExposure, PreparedLevelsJob, WorkspaceAdmission, limit, quote_exposure,
    quote_levels_job,
};
use paintcraft_core::types::{
    Result,
    filters::{ExposureSrgbV1, LevelsRgbV1},
    resources::BudgetAccount,
};
use std::sync::Arc;
#[derive(Clone, Copy, Debug)]
pub(super) struct JobQuote {
    pub levels: usize,
    pub exposures: usize,
    pub index_bytes: u64,
    pub exposure_tail: u64,
    pub peak_bytes: u64,
    pub retained_bytes: u64,
    pub compile_work: u64,
}
fn pointer_bytes(capacity: usize) -> Result<u64> {
    (capacity as u64)
        .checked_mul(std::mem::size_of::<Arc<PreparedExposure>>() as u64)
        .ok_or_else(|| limit("Prepared Exposure index size overflows"))
}
pub(super) fn quote(levels: &[LevelsRgbV1], exposures: &[ExposureSrgbV1]) -> Result<JobQuote> {
    if levels
        .len()
        .checked_add(exposures.len())
        .is_none_or(|n| n > PreparedPointJob::MAX_PROGRAMS)
    {
        return Err(limit(
            "Prepared program submitted key count exceeds host bound",
        ));
    }
    let l = quote_levels_job(levels)?;
    let mut count = 0;
    let mut retained = 0u64;
    let mut tail = 0u64;
    let mut work = l.compile_work;
    for (index, policy) in exposures.iter().enumerate() {
        if exposures[..index].contains(policy) {
            continue;
        }
        let e = quote_exposure(*policy);
        tail = tail.max(
            retained
                .checked_add(e.peak_bytes)
                .ok_or_else(|| limit("Prepared Exposure phase peak overflows"))?,
        );
        retained = retained
            .checked_add(e.retained_bytes)
            .ok_or_else(|| limit("Prepared Exposure retention overflows"))?;
        work = work
            .checked_add(e.compile_work)
            .ok_or_else(|| limit("Prepared program compile allowance overflows"))?;
        count += 1;
    }
    let index_bytes = pointer_bytes(count)?;
    let l_retained = l
        .index_bytes
        .checked_add(l.unique_programs as u64 * super::LEVELS_TABLE_BYTES)
        .ok_or_else(|| limit("Prepared Levels retention overflows"))?;
    let peak = index_bytes
        .checked_add(
            l.peak_bytes.max(
                l_retained
                    .checked_add(tail)
                    .ok_or_else(|| limit("Prepared program following phase overflows"))?,
            ),
        )
        .ok_or_else(|| limit("Prepared program peak overflows"))?;
    Ok(JobQuote {
        levels: l.unique_programs,
        exposures: count,
        index_bytes,
        exposure_tail: tail,
        peak_bytes: peak,
        retained_bytes: l_retained
            .checked_add(index_bytes)
            .and_then(|v| v.checked_add(retained))
            .ok_or_else(|| limit("Prepared program retention overflows"))?,
        compile_work: work,
    })
}
#[derive(Debug)]
pub struct PreparedPointJob {
    levels: Option<Arc<PreparedLevelsJob>>,
    exposures: Vec<Arc<PreparedExposure>>,
    _index_charge: BudgetAccount,
}
impl PreparedPointJob {
    /// Combined complete-key host bound, not a document restriction.
    pub const MAX_PROGRAMS: usize = 256;
    pub(super) fn new(
        levels: &[LevelsRgbV1],
        exposures: &[ExposureSrgbV1],
        admission: WorkspaceAdmission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        Self::new_with_tail(levels, exposures, admission, check, 0)
    }
    pub(super) fn new_with_tail(
        levels: &[LevelsRgbV1],
        exposures: &[ExposureSrgbV1],
        admission: WorkspaceAdmission,
        check: &dyn Fn() -> Result<()>,
        following_peak: u64,
    ) -> Result<Self> {
        Self::new_with_capacity(levels, exposures, admission, check, following_peak, None)
    }
    // Production requests the exact index. A child test can exercise a larger
    // real allocation without an allocator hook or a fake resource quote.
    fn new_with_capacity(
        levels: &[LevelsRgbV1],
        exposures: &[ExposureSrgbV1],
        admission: WorkspaceAdmission,
        check: &dyn Fn() -> Result<()>,
        following_peak: u64,
        requested: Option<usize>,
    ) -> Result<Self> {
        let plan = quote(levels, exposures)?;
        if plan.peak_bytes > admission.bytes_limit()
            || admission
                .work_used()
                .checked_add(plan.compile_work)
                .is_none_or(|w| w > admission.work_limit())
        {
            return Err(limit("Prepared program job exceeds aggregate allowance"));
        }
        if following_peak != 0
            && plan
                .retained_bytes
                .checked_add(following_peak)
                .is_none_or(|n| n > admission.bytes_limit())
        {
            return Err(limit(
                "Prepared following family exceeds aggregate allowance",
            ));
        }
        check()?;
        let mut charge = admission.scope().account();
        charge.resize(plan.index_bytes)?;
        let mut programs = Vec::new();
        programs
            .try_reserve_exact(requested.unwrap_or(plan.exposures))
            .map_err(|_| limit("Prepared Exposure index allocation failed"))?;
        let actual = pointer_bytes(programs.capacity())?;
        let actual_peak = plan
            .peak_bytes
            .checked_sub(plan.index_bytes)
            .and_then(|n| n.checked_add(actual))
            .ok_or_else(|| limit("Prepared program actual peak overflows"))?;
        if following_peak != 0 {
            charge.resize(actual)?;
            if plan
                .retained_bytes
                .checked_sub(plan.index_bytes)
                .and_then(|n| n.checked_add(actual))
                .and_then(|n| n.checked_add(following_peak))
                .is_none_or(|n| n > admission.bytes_limit())
            {
                return Err(limit(
                    "Prepared actual index leaves no following-family allowance",
                ));
            }
        }
        if actual_peak > admission.bytes_limit() {
            return Err(limit("Prepared program actual index exceeds allowance"));
        }
        charge.resize(actual)?;
        let available = admission
            .bytes_limit()
            .checked_sub(actual)
            .ok_or_else(|| limit("Prepared program index exceeds allowance"))?;
        // Every later family follows final E retention. E/Curves indexes and
        // capture arrays are already live outside this tail and not counted twice.
        let exposure_tail = if following_peak == 0 {
            plan.exposure_tail
        } else {
            plan.exposure_tail.max(
                (plan.exposures as u64)
                    .checked_mul(super::exposure::EXPOSURE_TABLE_BYTES)
                    .and_then(|n| n.checked_add(following_peak))
                    .ok_or_else(|| limit("Prepared following-family tail overflows"))?,
            )
        };
        // E index is already charged/live. The following tail excludes it.
        let levels = if levels.is_empty() {
            None
        } else {
            Some(Arc::new(PreparedLevelsJob::new_with_tail(
                levels,
                admission.clone().with_byte_limit(available),
                check,
                exposure_tail,
            )?))
        };
        let mut job = Self {
            levels,
            exposures: programs,
            _index_charge: charge,
        };
        let live_levels = job.levels().map_or(0, PreparedLevelsJob::retained_bytes);
        if live_levels
            .checked_add(exposure_tail)
            .is_none_or(|n| n > available)
        {
            return Err(limit("Prepared Exposure following phase exceeds allowance"));
        }
        for policy in exposures {
            check()?;
            if job.exposure(*policy).is_some() {
                continue;
            }
            if job.exposures.len() == job.exposures.capacity() {
                return Err(limit("Prepared Exposure exhausted admitted index"));
            }
            let live = live_levels
                .checked_add(job.exposures.len() as u64 * super::exposure::EXPOSURE_TABLE_BYTES)
                .ok_or_else(|| limit("Prepared program live payload overflows"))?;
            let bytes = available
                .checked_sub(live)
                .ok_or_else(|| limit("Prepared Exposure coexistence exceeds allowance"))?;
            let program =
                PreparedExposure::new(*policy, &admission.clone().with_byte_limit(bytes), check)?;
            check()?;
            job.exposures.push(Arc::new(program));
        }
        debug_assert_eq!(job.exposures.len(), plan.exposures);
        check()?;
        Ok(job)
    }
    pub fn levels(&self) -> Option<&PreparedLevelsJob> {
        self.levels.as_deref()
    }
    pub fn exposure(&self, policy: ExposureSrgbV1) -> Option<&PreparedExposure> {
        self.exposures
            .iter()
            .find(|p| p.policy() == policy)
            .map(Arc::as_ref)
    }
    pub fn len(&self) -> usize {
        self.levels().map_or(0, PreparedLevelsJob::len) + self.exposures.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Actual reconciled live table/index payload, excluding Arc bookkeeping.
    pub fn retained_bytes(&self) -> u64 {
        self.levels().map_or(0, PreparedLevelsJob::retained_bytes)
            + self._index_charge.bytes()
            + self.exposures.len() as u64 * super::exposure::EXPOSURE_TABLE_BYTES
    }
}

#[cfg(test)]
mod tail_tests {
    use super::*;
    use paintcraft_core::types::{
        ErrorCode,
        resources::{AllocationClass, SharedBudget},
    };
    #[test]
    fn actual_exposure_index_must_leave_following_curves_tail_before_compilation() {
        let e = ExposureSrgbV1::default();
        let q = quote(&[], &[e]).unwrap();
        let tail = 38 * 256 + 256;
        let bytes = q.retained_bytes + tail;
        let b = SharedBudget::new(bytes);
        let a = WorkspaceAdmission::new(
            b.scope(AllocationClass::SharedBudget),
            bytes,
            q.compile_work,
        );
        assert_eq!(
            PreparedPointJob::new_with_capacity(&[], &[e], a.clone(), &|| Ok(()), tail, Some(2))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(a.work_used(), 0);
        assert_eq!(b.usage().reserved, 0);
    }
}
