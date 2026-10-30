//! Immutable finite A8 fields. This is independent of binary live selections.
use paintcraft_core::types::resources::{BudgetAccount, SharedBudget};
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, policy::COORD_ABS_MAX};
use sha2::{Digest, Sha256};
use std::sync::Arc;
mod builder;
mod views;

/// A half-open nonzero run after canonicalization. Input may contain zero runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub x0: i64,
    pub x1: i64,
    pub coverage: u8,
}
/// Repeated identical rows, with ordered nonoverlapping half-open runs.
#[derive(Clone, Copy, Debug)]
pub struct Band<'a> {
    pub y0: i64,
    pub y1: i64,
    pub runs: &'a [Run],
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub work: u64,
    pub bands: usize,
    pub runs: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            work: 67_108_864,
            bands: 262_144,
            runs: 262_144,
        }
    }
}
#[derive(Debug)]
struct Rows {
    y0: i64,
    y1: i64,
    start: usize,
    len: usize,
}
#[derive(Debug)]
struct Storage {
    bands: Vec<Rows>,
    runs: Vec<Run>,
    bytes: u64,
    // Allocations precede the reservation in drop order.
    _charge: BudgetAccount,
}
/// Outside `domain`, or outside retained support after a clip, coverage is zero.
/// Views have one offset and one support rectangle; repeated views cannot build
/// a wrapper chain. Clones/views share their unique immutable storage charge.
#[derive(Clone, Debug)]
pub struct A8Mask {
    domain: Rect,
    support: Option<Rect>,
    offset: (i64, i64),
    storage: Option<Arc<Storage>>,
    digest: [u8; 32],
}
// Equality is synchronous and bounded by retained records. Callers that need
// cooperative cancellation must use same_coverage_checked instead.
impl PartialEq for A8Mask {
    fn eq(&self, other: &Self) -> bool {
        self.domain == other.domain
            && self.digest == other.digest
            && self.visible().eq(other.visible())
    }
}
impl Eq for A8Mask {}
struct Probe<'a> {
    remaining: u64,
    check: &'a dyn Fn() -> Result<()>,
}
impl Probe<'_> {
    fn step(&mut self) -> Result<()> {
        (self.check)()?;
        self.remaining = self.remaining.checked_sub(1).ok_or_else(limit)?;
        Ok(())
    }
}
impl A8Mask {
    /// Canonicalize without walking implicit pixels. Adjacent equal runs and
    /// identical row bands merge; zero runs disappear. Every allocation is charged.
    pub fn from_bands(
        domain: Rect,
        bands: &[Band<'_>],
        budget: &SharedBudget,
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let mut probe = Probe {
            remaining: limits.work,
            check,
        };
        probe.step()?;
        valid(domain)?;
        let storage = builder::build(domain, bands, budget, limits, &mut probe)?;
        let mut mask = Self {
            domain,
            support: Some(domain),
            offset: (0, 0),
            storage,
            digest: [0; 32],
        };
        mask.digest = mask.hash(&mut probe)?;
        probe.step()?;
        Ok(mask)
    }
    /// A nonzero rectangle uses one band/run, regardless of its logical area.
    pub fn uniform(
        domain: Rect,
        coverage: u8,
        budget: &SharedBudget,
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let runs = [Run {
            x0: domain.x0,
            x1: domain.x1,
            coverage,
        }];
        Self::from_bands(
            domain,
            &[Band {
                y0: domain.y0,
                y1: domain.y1,
                runs: &runs,
            }],
            budget,
            limits,
            check,
        )
    }
    pub fn domain(&self) -> Rect {
        self.domain
    }
    /// Implicit samples are always zero; nonzero uniform fields are one rectangle.
    pub fn default_coverage(&self) -> u8 {
        0
    }
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn storage_usage(&self) -> Option<(usize, u64)> {
        self.storage
            .as_ref()
            .map(|s| (Arc::as_ptr(s) as usize, s.bytes))
    }
    pub fn coverage(&self, x: i64, y: i64) -> u8 {
        if !self.domain.contains(x, y) || self.support.is_none_or(|s| !s.contains(x, y)) {
            return 0;
        }
        let Some(storage) = &self.storage else {
            return 0;
        };
        let (x, y) = (x - self.offset.0, y - self.offset.1);
        let b = storage.bands.partition_point(|b| b.y1 <= y);
        let Some(band) = storage.bands.get(b).filter(|b| b.y0 <= y) else {
            return 0;
        };
        let runs = &storage.runs[band.start..band.start + band.len];
        let r = runs.partition_point(|r| r.x1 <= x);
        runs.get(r).filter(|r| r.x0 <= x).map_or(0, |r| r.coverage)
    }
    /// Exact semantic comparison, including the finite domain, with cooperative
    /// work/cancellation. Cached digest is only a rejection shortcut, not authority.
    pub fn same_coverage_checked(
        &self,
        other: &Self,
        work: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<bool> {
        let mut probe = Probe {
            remaining: work,
            check,
        };
        probe.step()?;
        if self.domain != other.domain || self.digest != other.digest {
            return Ok(false);
        }
        let mut a = self.visible();
        let mut b = other.visible();
        loop {
            probe.step()?;
            match (a.next_checked(&mut probe)?, b.next_checked(&mut probe)?) {
                (None, None) => return Ok(true),
                (Some(a), Some(b)) if (a.y0, a.y1) == (b.y0, b.y1) => {
                    let mut ar = a.runs();
                    let mut br = b.runs();
                    loop {
                        probe.step()?;
                        match (ar.next_checked(&mut probe)?, br.next_checked(&mut probe)?) {
                            (None, None) => break,
                            (Some(a), Some(b)) if a == b => {}
                            _ => return Ok(false),
                        }
                    }
                }
                _ => return Ok(false),
            }
        }
    }
    fn hash(&self, probe: &mut Probe<'_>) -> Result<[u8; 32]> {
        let mut hash = Sha256::new();
        hash.update(b"paintcraft.a8-mask/v1\0");
        for v in [
            self.domain.x0,
            self.domain.y0,
            self.domain.x1,
            self.domain.y1,
        ] {
            hash.update(v.to_be_bytes());
        }
        let mut count = 0u64;
        let mut cursor = self.visible();
        while cursor.next_checked(probe)?.is_some() {
            count += 1;
        }
        hash.update(count.to_be_bytes());
        let mut cursor = self.visible();
        while let Some(band) = cursor.next_checked(probe)? {
            hash.update(band.y0.to_be_bytes());
            hash.update(band.y1.to_be_bytes());
            let mut count = 0u64;
            let mut runs = band.runs();
            while runs.next_checked(probe)?.is_some() {
                count += 1;
            }
            hash.update(count.to_be_bytes());
            let mut runs = band.runs();
            while let Some(run) = runs.next_checked(probe)? {
                hash.update(run.x0.to_be_bytes());
                hash.update(run.x1.to_be_bytes());
                hash.update([run.coverage]);
            }
        }
        probe.step()?;
        Ok(hash.finalize().into())
    }
}
fn valid(r: Rect) -> Result<()> {
    if r.x0 >= r.x1
        || r.y0 >= r.y1
        || [r.x0, r.x1, r.y0, r.y1]
            .iter()
            .any(|v| v.unsigned_abs() > COORD_ABS_MAX as u64)
    {
        Err(Error::new(ErrorCode::InputSchema, "Invalid A8 mask domain"))
    } else {
        Ok(())
    }
}
fn schema() -> Error {
    Error::new(
        ErrorCode::InputSchema,
        "A8 bands/runs must be ordered, disjoint and inside the finite domain",
    )
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "A8 mask work or storage admission exceeded",
    )
}
#[cfg(test)]
mod tests;
