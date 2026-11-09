//! Strict canonical streams for portable read. Generic normalizers are unchanged.
use super::*;
use std::mem::{align_of, size_of};
mod hash;
#[cfg(test)]
mod tests;
pub const CANONICAL_A8_CONTROL_BYTES: u64 = 4096;
#[derive(Clone, Copy, Debug)]
pub struct CanonicalA8Config {
    pub domain: Rect,
    pub bands: usize,
    pub runs: usize,
    pub limits: Limits,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Pending {
    y0: i64,
    y1: i64,
    start: u64,
    expected: u64,
    last_x: i64,
    last_coverage: u8,
    has_run: bool,
}
struct Meter<'a> {
    check: &'a dyn Fn() -> Result<()>,
    spend: &'a dyn Fn(u64) -> Result<()>,
    used: u64,
    limit: u64,
}
impl Meter<'_> {
    fn debit(&mut self, n: u64) -> Result<()> {
        let next = self.used.checked_add(n).ok_or_else(limit)?;
        if next > self.limit {
            return Err(limit());
        }
        (self.spend)(n)?;
        self.used = next;
        Ok(())
    }
    fn check(&mut self) -> Result<()> {
        self.debit(512)?;
        (self.check)()
    }
    fn entry(&mut self) -> Result<()> {
        self.check()?;
        self.debit(32)
    }
}
/// Full defining Storage Arc allocation, separate from legacy retained policy.
/// Caller retains its original complete-union guard through every mask alias.
pub fn canonical_a8_storage_arc_quote() -> Result<u64> {
    let alignment = align_of::<Storage>().max(align_of::<usize>());
    let offset = (2 * size_of::<usize>())
        .checked_next_multiple_of(align_of::<Storage>())
        .ok_or_else(limit)?;
    offset
        .checked_add(size_of::<Storage>())
        .and_then(|n| n.checked_next_multiple_of(alignment))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(limit)
}
/// No raw storage/account escape or reusable invalid state. Each operation consumes
/// its handle on error. M's complete-source union/lifetime remains caller-owned.
/// ```compile_fail
/// use paintcraft_raster::coverage_mask::CanonicalA8Builder;
/// fn detach(builder: CanonicalA8Builder<'_>) { let _ = builder.into_vec(); }
/// ```
pub struct CanonicalA8Builder<'a> {
    rows: Vec<Rows>,
    runs: Vec<Run>,
    config: CanonicalA8Config,
    pending: Option<Pending>,
    meter: Meter<'a>,
    _scope: &'a BudgetScope,
    _storage_guard: &'a BudgetAccount,
    retained_bytes: u64,
    // Payloads and all local borrowed controls precede their declarations.
    retained: BudgetAccount,
    _control: BudgetAccount,
}
impl std::fmt::Debug for CanonicalA8Builder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanonicalA8Builder")
            .field("bands", &self.rows.len())
            .field("runs", &self.runs.len())
            .field("in_band", &self.pending.is_some())
            .finish_non_exhaustive()
    }
}
impl<'a> CanonicalA8Builder<'a> {
    pub fn new(
        config: CanonicalA8Config,
        scope: &'a BudgetScope,
        storage_guard: &'a BudgetAccount,
        check: &'a dyn Fn() -> Result<()>,
        spend: &'a dyn Fn(u64) -> Result<()>,
    ) -> Result<Self> {
        let mut meter = Meter {
            check,
            spend,
            used: 0,
            limit: config.limits.work,
        };
        meter.debit(64)?;
        let controls = [
            size_of::<Self>(),
            size_of::<Sha256>(),
            size_of::<A8Mask>(),
            2 * size_of::<Rows>(),
            2 * size_of::<Run>(),
            size_of::<Pending>(),
            size_of::<Rect>(),
            size_of::<Stats>(),
            2 * size_of::<[u8; 32]>(),
            4 * size_of::<[u8; 8]>(),
            size_of::<[u64; 8]>(),
            4 * size_of::<usize>(),
        ]
        .into_iter()
        .try_fold(0usize, |n, v| n.checked_add(v).ok_or_else(limit))?;
        if controls as u64 > CANONICAL_A8_CONTROL_BYTES {
            return Err(limit());
        }
        let mut control = scope.account();
        control.resize(CANONICAL_A8_CONTROL_BYTES)?;
        meter.check()?;
        valid(config.domain)?;
        if config.bands > config.limits.bands
            || config.runs > config.limits.runs
            || (config.bands == 0) != (config.runs == 0)
            || config.runs < config.bands
        {
            return Err(schema());
        }
        let retained_bytes = retained_quote(config.bands, config.runs)?;
        let guard = if config.bands == 0 {
            0
        } else {
            canonical_a8_storage_arc_quote()?
        };
        if storage_guard.bytes() < guard {
            return Err(limit());
        }
        let mut retained = scope.account();
        retained.resize(retained_bytes)?;
        meter.debit(64)?; // Mutually exclusive partial/success local owner cleanup.
        let slots = config
            .bands
            .checked_mul(size_of::<Rows>())
            .and_then(|n| {
                config
                    .runs
                    .checked_mul(size_of::<Run>())
                    .and_then(|r| n.checked_add(r))
            })
            .ok_or_else(limit)?;
        meter.debit(slots as u64)?;
        // Accounts are already admitted; local payloads drop first on failure.
        let mut rows = Vec::new();
        let mut runs = Vec::new();
        if config.bands != 0 {
            rows.try_reserve_exact(config.bands).map_err(|_| limit())?;
            runs.try_reserve_exact(config.runs).map_err(|_| limit())?;
        }
        if rows.capacity() != config.bands || runs.capacity() != config.runs {
            return Err(limit());
        }
        Ok(Self {
            rows,
            runs,
            config,
            pending: None,
            meter,
            _scope: scope,
            _storage_guard: storage_guard,
            retained_bytes,
            retained,
            _control: control,
        })
    }
    pub fn band(mut self, y0: i64, y1: i64, run_count: usize) -> Result<Self> {
        self.meter.entry()?;
        let domain = self.config.domain;
        if self.pending.is_some()
            || self.rows.len() >= self.config.bands
            || run_count == 0
            || run_count > self.config.runs - self.runs.len()
            || y0 >= y1
            || y0 < domain.y0
            || y1 > domain.y1
            || self.rows.last().is_some_and(|r| y0 < r.y1)
        {
            return Err(schema());
        }
        self.meter.debit(size_of::<Pending>() as u64)?;
        self.pending = Some(Pending {
            y0,
            y1,
            start: self.runs.len() as u64,
            expected: run_count as u64,
            last_x: domain.x0,
            ..Default::default()
        });
        Ok(self)
    }
    pub fn run(mut self, run: Run) -> Result<Self> {
        self.meter.entry()?;
        let pending = self.pending.as_mut().ok_or_else(schema)?;
        if self.runs.len() >= self.config.runs
            || self.runs.len() >= self.runs.capacity()
            || run.x0 < self.config.domain.x0
            || run.x0 < pending.last_x
            || run.x0 >= run.x1
            || run.x1 > self.config.domain.x1
            || run.coverage == 0
            || (pending.has_run
                && run.x0 == pending.last_x
                && run.coverage == pending.last_coverage)
        {
            return Err(schema());
        }
        self.meter.debit(size_of::<Run>() as u64)?;
        self.runs.push(run);
        pending.last_x = run.x1;
        pending.last_coverage = run.coverage;
        pending.has_run = true;
        if self.runs.len() as u64 - pending.start == pending.expected {
            let current = *pending;
            if let Some(old) = self.rows.last()
                && old.y1 == current.y0
                && old.len as u64 == current.expected
            {
                let mut equal = true;
                for i in 0..old.len {
                    if i % 128 == 0 {
                        self.meter.check()?;
                    }
                    self.meter.debit(34)?;
                    if self.runs[old.start + i] != self.runs[current.start as usize + i] {
                        equal = false;
                        break;
                    }
                }
                if equal {
                    return Err(schema());
                }
            }
            if self.rows.len() >= self.rows.capacity() {
                return Err(limit());
            }
            self.meter.debit(size_of::<Rows>() as u64)?;
            self.rows.push(Rows {
                y0: current.y0,
                y1: current.y1,
                start: current.start as usize,
                len: current.expected as usize,
            });
            self.pending = None;
        }
        Ok(self)
    }
    pub fn finish(mut self) -> Result<A8Mask> {
        self.meter.entry()?;
        if self.pending.is_some()
            || self.rows.len() != self.config.bands
            || self.runs.len() != self.config.runs
        {
            return Err(schema());
        }
        let digest = hash::canonical(self.config.domain, &self.rows, &self.runs, &mut self.meter)?;
        self.meter.debit(64)?;
        let stats = Stats {
            bands: self.rows.len(),
            runs: self.runs.len(),
        };
        let storage = if self.rows.is_empty() {
            None
        } else {
            self.meter.debit(64)?;
            Some(Arc::new(Storage {
                bands: self.rows,
                runs: self.runs,
                bytes: self.retained_bytes,
                _charge: self.retained,
            }))
        };
        let mask = A8Mask {
            domain: self.config.domain,
            support: Some(self.config.domain),
            offset: (0, 0),
            storage,
            digest,
            stats,
        };
        self.meter.check()?;
        Ok(mask)
    }
}
