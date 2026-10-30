//! Checked numeric input ownership, independent of Raster's canonical A8 field.
//! JSON/transport buffers belong to the caller; only owned numeric storage is
//! charged here. Canonicalization into retained Raster storage has its own charge.
use super::*;
use crate::types::{
    Rect,
    policy::COORD_ABS_MAX,
    resources::{AllocationClass, BudgetAccount, SharedBudget},
};
use std::{mem::size_of, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct A8Run {
    pub x0: i64,
    pub x1: i64,
    pub coverage: u8,
}
#[derive(Clone, Copy, Debug)]
pub struct A8Band<'a> {
    pub y0: i64,
    pub y1: i64,
    pub runs: &'a [A8Run],
}
#[derive(Clone, Copy, Debug)]
pub struct MaskInputLimits {
    pub work: u64,
    pub bands: usize,
    pub runs: usize,
}
impl Default for MaskInputLimits {
    fn default() -> Self {
        Self {
            work: 67_108_864,
            bands: 262_144,
            runs: 262_144,
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
struct Rows {
    y0: i64,
    y1: i64,
    start: usize,
    len: usize,
}
#[derive(Debug)]
struct Storage {
    rows: Vec<Rows>,
    runs: Vec<A8Run>,
    bytes: u64,
    // Storage drops before its unique account; Arc clones do not reprice it.
    _charge: BudgetAccount,
}
#[derive(Clone, Debug)]
pub struct A8Input {
    domain: Rect,
    storage: Arc<Storage>,
}
impl PartialEq for A8Input {
    fn eq(&self, other: &Self) -> bool {
        self.domain == other.domain
            && (Arc::ptr_eq(&self.storage, &other.storage)
                || self.storage.rows == other.storage.rows
                    && self.storage.runs == other.storage.runs)
    }
}
impl Eq for A8Input {}
impl A8Input {
    pub fn domain(&self) -> Rect {
        self.domain
    }
    pub fn band_count(&self) -> usize {
        self.storage.rows.len()
    }
    pub fn run_count(&self) -> usize {
        self.storage.runs.len()
    }
    pub fn storage_usage(&self) -> (usize, u64) {
        (Arc::as_ptr(&self.storage) as usize, self.storage.bytes)
    }
    pub fn bands(&self) -> impl Clone + ExactSizeIterator<Item = A8Band<'_>> {
        self.storage.rows.iter().map(|r| A8Band {
            y0: r.y0,
            y1: r.y1,
            runs: &self.storage.runs[r.start..r.start + r.len],
        })
    }
}
/// Pre-admit counts, then append borrowed scalar input without temporary arrays.
/// Input rows/runs must be ordered and nonoverlapping; zero coverage is allowed.
/// Adjacent equal records are deliberately preserved until Raster canonicalizes.
pub struct A8InputBuilder<'a> {
    domain: Rect,
    rows: Vec<Rows>,
    runs: Vec<A8Run>,
    expected: (usize, usize),
    remaining: u64,
    check: &'a dyn Fn() -> Result<()>,
    charge: BudgetAccount,
}
impl std::fmt::Debug for A8InputBuilder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A8InputBuilder")
            .field("domain", &self.domain)
            .field("expected", &self.expected)
            .field("remaining", &self.remaining)
            .finish_non_exhaustive()
    }
}
impl<'a> A8InputBuilder<'a> {
    pub fn new(
        domain: Rect,
        bands: usize,
        runs: usize,
        budget: &SharedBudget,
        limits: MaskInputLimits,
        check: &'a dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if domain.x0 >= domain.x1
            || domain.y0 >= domain.y1
            || [domain.x0, domain.x1, domain.y0, domain.y1]
                .iter()
                .any(|v| v.unsigned_abs() > COORD_ABS_MAX as u64)
        {
            return Err(schema());
        }
        if bands > limits.bands || runs > limits.runs || bands == 0 && runs != 0 {
            return Err(limit());
        }
        let mut charge = budget.account(AllocationClass::CoverageMasks);
        charge.resize(quote(bands, runs)?)?;
        let mut rows = Vec::new();
        let mut values = Vec::new();
        rows.try_reserve_exact(bands).map_err(|_| limit())?;
        values.try_reserve_exact(runs).map_err(|_| limit())?;
        if let Err(error) = charge.resize(quote(rows.capacity(), values.capacity())?) {
            drop(rows);
            drop(values);
            return Err(error);
        }
        check()?;
        Ok(Self {
            domain,
            rows,
            runs: values,
            expected: (bands, runs),
            remaining: limits.work,
            check,
            charge,
        })
    }
    fn step(&mut self) -> Result<()> {
        (self.check)()?;
        self.remaining = self.remaining.checked_sub(1).ok_or_else(limit)?;
        Ok(())
    }
    pub fn push_band(&mut self, y0: i64, y1: i64) -> Result<()> {
        self.step()?;
        if self.rows.len() >= self.expected.0
            || y0 >= y1
            || y0 < self.domain.y0
            || y1 > self.domain.y1
            || self.rows.last().is_some_and(|r| r.y1 > y0)
        {
            return Err(schema());
        }
        self.rows.push(Rows {
            y0,
            y1,
            start: self.runs.len(),
            len: 0,
        });
        Ok(())
    }
    pub fn push_run(&mut self, run: A8Run) -> Result<()> {
        self.step()?;
        let band = self.rows.last_mut().ok_or_else(schema)?;
        if self.runs.len() >= self.expected.1
            || run.x0 >= run.x1
            || run.x0 < self.domain.x0
            || run.x1 > self.domain.x1
            || (band.len > 0 && self.runs.last().is_some_and(|r| r.x1 > run.x0))
        {
            return Err(schema());
        }
        band.len += 1;
        self.runs.push(run);
        Ok(())
    }
    pub fn finish(mut self) -> Result<A8Input> {
        self.step()?;
        if (self.rows.len(), self.runs.len()) != self.expected {
            return Err(schema());
        }
        let bytes = quote(self.rows.capacity(), self.runs.capacity())?;
        Ok(A8Input {
            domain: self.domain,
            storage: Arc::new(Storage {
                rows: self.rows,
                runs: self.runs,
                bytes,
                _charge: self.charge,
            }),
        })
    }
}
fn quote(bands: usize, runs: usize) -> Result<u64> {
    bands
        .checked_mul(size_of::<Rows>())
        .and_then(|n| {
            runs.checked_mul(size_of::<A8Run>())
                .and_then(|r| n.checked_add(r))
        })
        .and_then(|n| n.checked_add(size_of::<Storage>() + 2 * size_of::<usize>()))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(limit)
}
fn schema() -> Error {
    Error::new(ErrorCode::InputSchema, "Invalid numeric A8 mask records")
}
fn limit() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Numeric A8 mask input allowance")
}

#[cfg(test)]
mod tests;
