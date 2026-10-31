//! Curves-active wrapper. The legacy PreparedPointJob layout is unchanged.
use super::{PreparedCurves, PreparedPointJob, WorkspaceAdmission, limit, quote_curves};
use paintcraft_core::types::{
    Result,
    filters::{CurvesRgbV1, ExposureSrgbV1, LevelsRgbV1},
    resources::BudgetAccount,
};
use std::sync::Arc;
#[derive(Clone, Copy, Debug)]
pub(super) struct Quote {
    pub levels: usize,
    pub exposures: usize,
    pub curves: usize,
    pub index: u64,
    pub tail: u64,
    pub peak: u64,
    pub retained: u64,
    pub work: u64,
}
fn pointers(cap: usize) -> Result<u64> {
    (cap as u64)
        .checked_mul(std::mem::size_of::<Arc<PreparedCurves>>() as u64)
        .ok_or_else(|| limit("Prepared Curves index overflow"))
}
pub(super) fn quote(l: &[LevelsRgbV1], e: &[ExposureSrgbV1], c: &[CurvesRgbV1]) -> Result<Quote> {
    if l.len()
        .checked_add(e.len())
        .and_then(|n| n.checked_add(c.len()))
        .is_none_or(|n| n > PreparedPointJob::MAX_PROGRAMS)
    {
        return Err(limit("Prepared combined submitted key bound"));
    }
    let old = super::point_job::quote(l, e)?;
    let (mut count, mut retained, mut tail, mut work) = (0, 0u64, 0u64, old.compile_work);
    for (i, p) in c.iter().enumerate() {
        if c[..i].contains(p) {
            continue;
        }
        let q = quote_curves(*p);
        tail = tail.max(
            retained
                .checked_add(q.peak_bytes)
                .ok_or_else(|| limit("Prepared Curves tail overflow"))?,
        );
        retained = retained
            .checked_add(q.retained_bytes)
            .ok_or_else(|| limit("Prepared Curves retention overflow"))?;
        work = work
            .checked_add(q.compile_work)
            .ok_or_else(|| limit("Prepared Curves work overflow"))?;
        count += 1;
    }
    let index = pointers(count)?;
    let peak = index
        .checked_add(
            old.peak_bytes.max(
                old.retained_bytes
                    .checked_add(tail)
                    .ok_or_else(|| limit("Prepared Curves coexistence overflow"))?,
            ),
        )
        .ok_or_else(|| limit("Prepared Curves peak overflow"))?;
    let retained = old
        .retained_bytes
        .checked_add(index)
        .and_then(|n| n.checked_add(retained))
        .ok_or_else(|| limit("Prepared Curves retention overflow"))?;
    Ok(Quote {
        levels: old.levels,
        exposures: old.exposures,
        curves: count,
        index,
        tail,
        peak,
        retained,
        work,
    })
}
#[derive(Debug)]
pub struct PreparedCurvesJob {
    legacy: Option<Arc<PreparedPointJob>>,
    curves: Vec<Arc<PreparedCurves>>,
    _index: BudgetAccount,
}
impl PreparedCurvesJob {
    pub(super) fn new(
        l: &[LevelsRgbV1],
        e: &[ExposureSrgbV1],
        c: &[CurvesRgbV1],
        a: WorkspaceAdmission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let q = quote(l, e, c)?;
        if q.peak > a.bytes_limit()
            || a.work_used()
                .checked_add(q.work)
                .is_none_or(|n| n > a.work_limit())
        {
            return Err(limit("Prepared Curves wrapper aggregate allowance"));
        }
        check()?;
        let mut charge = a.scope().account();
        charge.resize(q.index)?;
        let mut curves = Vec::new();
        curves
            .try_reserve_exact(q.curves)
            .map_err(|_| limit("Prepared Curves index allocation"))?;
        let actual = pointers(curves.capacity())?;
        charge.resize(actual)?;
        if q.peak
            .checked_sub(q.index)
            .and_then(|n| n.checked_add(actual))
            .is_none_or(|n| n > a.bytes_limit())
        {
            return Err(limit("Prepared Curves actual index exceeds allowance"));
        }
        let available = a
            .bytes_limit()
            .checked_sub(actual)
            .ok_or_else(|| limit("Prepared Curves index allowance"))?;
        let legacy = if l.is_empty() && e.is_empty() {
            None
        } else {
            Some(Arc::new(PreparedPointJob::new_with_tail(
                l,
                e,
                a.clone().with_byte_limit(available),
                check,
                q.tail,
            )?))
        };
        let old = legacy.as_ref().map_or(0, |j| j.retained_bytes());
        if old.checked_add(q.tail).is_none_or(|n| n > available) {
            return Err(limit("Prepared Curves actual legacy coexistence"));
        }
        let mut job = Self {
            legacy,
            curves,
            _index: charge,
        };
        for p in c {
            check()?;
            if job.get(*p).is_some() {
                continue;
            }
            if job.curves.len() == job.curves.capacity() {
                return Err(limit("Prepared Curves exhausted admitted index"));
            }
            let live = old
                .checked_add(job.curves.len() as u64 * super::curves::CURVES_TABLE_BYTES)
                .ok_or_else(|| limit("Prepared Curves live overflow"))?;
            let bytes = available
                .checked_sub(live)
                .ok_or_else(|| limit("Prepared Curves live allowance"))?;
            let program = PreparedCurves::new(*p, &a.clone().with_byte_limit(bytes), check)?;
            check()?;
            job.curves.push(Arc::new(program));
        }
        check()?;
        Ok(job)
    }
    pub fn legacy(&self) -> Option<&PreparedPointJob> {
        self.legacy.as_deref()
    }
    pub fn get(&self, p: CurvesRgbV1) -> Option<&PreparedCurves> {
        self.curves
            .iter()
            .find(|c| c.policy() == p)
            .map(Arc::as_ref)
    }
    pub fn len(&self) -> usize {
        self.curves.len() + self.legacy().map_or(0, PreparedPointJob::len)
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn retained_bytes(&self) -> u64 {
        self.legacy().map_or(0, PreparedPointJob::retained_bytes)
            + self._index.bytes()
            + self.curves.len() as u64 * super::curves::CURVES_TABLE_BYTES
    }
}
