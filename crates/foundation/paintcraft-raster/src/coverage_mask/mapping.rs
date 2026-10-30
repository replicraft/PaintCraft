//! Exact integer mappings from disjoint constant rectangles; no dense canvas.
use super::*;
use paintcraft_core::types::orthogonal::OrthogonalTransform;
use std::mem::size_of;
#[derive(Clone, Copy, Debug)]
pub(super) struct Sample {
    pub rect: Rect,
    pub value: u8,
}
pub(super) struct Buffer<T> {
    pub values: Vec<T>,
    charge: BudgetAccount,
    max: usize,
}
impl<T> Buffer<T> {
    pub fn new(scope: &BudgetScope, max: usize) -> Self {
        Self {
            values: Vec::new(),
            charge: scope.account(),
            max,
        }
    }
    pub fn reserve(&mut self, wanted: usize) -> Result<()> {
        if wanted > self.max {
            return Err(limit());
        }
        if wanted <= self.values.capacity() {
            return Ok(());
        }
        let capacity = wanted
            .max(self.values.capacity().saturating_mul(2))
            .min(self.max);
        let bytes = capacity
            .checked_mul(size_of::<T>())
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(limit)?;
        self.charge.resize(bytes)?;
        if self
            .values
            .try_reserve_exact(capacity - self.values.len())
            .is_err()
        {
            self.values = Vec::new();
            self.charge.resize(0)?;
            return Err(limit());
        }
        let actual = self
            .values
            .capacity()
            .checked_mul(size_of::<T>())
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(limit)?;
        if let Err(error) = self.charge.resize(actual) {
            self.values = Vec::new();
            return Err(error);
        }
        Ok(())
    }
    pub fn push(&mut self, value: T, probe: &Probe<'_>) -> Result<()> {
        probe.step()?;
        self.reserve(self.values.len().checked_add(1).ok_or_else(limit)?)?;
        self.values.push(value);
        Ok(())
    }
}
pub(super) fn sort<T: Copy + Ord>(
    values: &mut [T],
    scope: &BudgetScope,
    probe: &Probe<'_>,
) -> Result<()> {
    for chunk in values.chunks_mut(128) {
        let mut failure = None;
        chunk.sort_unstable_by(|a, b| {
            if failure.is_none()
                && let Err(e) = probe.step()
            {
                failure = Some(e);
            }
            // Finish at most this128-record chunk with a consistent ordering;
            // latch cancellation without asking the caller again after failure.
            a.cmp(b)
        });
        if let Some(error) = failure {
            return Err(error);
        }
    }
    if values.len() <= 128 {
        return Ok(());
    }
    let mut scratch = Buffer::new(scope, values.len());
    scratch.reserve(values.len())?;
    let mut width = 128usize;
    while width < values.len() {
        scratch.values.clear();
        for start in (0..values.len()).step_by(width.saturating_mul(2)) {
            let mid = (start + width).min(values.len());
            let end = (mid + width).min(values.len());
            let (mut a, mut b) = (start, mid);
            while a < mid || b < end {
                probe.step()?;
                let item = if b == end || (a < mid && values[a] <= values[b]) {
                    let v = values[a];
                    a += 1;
                    v
                } else {
                    let v = values[b];
                    b += 1;
                    v
                };
                scratch.values.push(item);
            }
        }
        for (dst, src) in values.chunks_mut(128).zip(scratch.values.chunks(128)) {
            probe.step()?;
            dst.copy_from_slice(src);
        }
        width = width.checked_mul(2).ok_or_else(limit)?;
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct OrderedRun(Run);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Event {
    y: i64,
    start: bool,
    index: usize,
}
pub(super) fn assemble(
    domain: Rect,
    samples: &[Sample],
    scope: &BudgetScope,
    limits: Limits,
    probe: &Probe<'_>,
) -> Result<A8Mask> {
    let mut events = Buffer::new(scope, samples.len().checked_mul(2).ok_or_else(limit)?);
    for (index, sample) in samples.iter().enumerate() {
        probe.step()?;
        if sample.value == 0 {
            continue;
        }
        if sample.rect.intersect(&domain) != Some(sample.rect) {
            return Err(schema());
        }
        events.push(
            Event {
                y: sample.rect.y0,
                start: true,
                index,
            },
            probe,
        )?;
        events.push(
            Event {
                y: sample.rect.y1,
                start: false,
                index,
            },
            probe,
        )?;
    }
    sort(&mut events.values, scope, probe)?;
    let mut positions = Buffer::new(scope, samples.len());
    positions.reserve(samples.len())?;
    for _ in samples {
        positions.push(usize::MAX, probe)?;
    }
    let mut active = Buffer::new(scope, samples.len());
    let mut rows = Buffer::<Rows>::new(scope, limits.bands);
    let mut flat = Buffer::<Run>::new(scope, limits.runs);
    let mut at = 0;
    while at < events.values.len() {
        let y = events.values[at].y;
        while at < events.values.len() && events.values[at].y == y {
            probe.step()?;
            let event = events.values[at];
            at += 1;
            if event.start {
                positions.values[event.index] = active.values.len();
                active.push(event.index, probe)?;
            } else {
                let position = positions.values[event.index];
                if position >= active.values.len() {
                    return Err(schema());
                }
                active.values.swap_remove(position);
                positions.values[event.index] = usize::MAX;
                if let Some(index) = active.values.get(position) {
                    positions.values[*index] = position;
                }
            }
        }
        let Some(next) = events.values.get(at).map(|e| e.y) else {
            break;
        };
        if active.values.is_empty() {
            continue;
        }
        let mut runlist = Buffer::new(
            scope,
            usize::try_from(probe.remaining.get()).unwrap_or(usize::MAX),
        );
        for &index in &active.values {
            let sample = samples[index];
            runlist.push(
                OrderedRun(Run {
                    x0: sample.rect.x0,
                    x1: sample.rect.x1,
                    coverage: sample.value,
                }),
                probe,
            )?;
        }
        sort(&mut runlist.values, scope, probe)?;
        let mut canonical = Buffer::<Run>::new(scope, limits.runs);
        for OrderedRun(run) in runlist.values.iter().copied() {
            probe.step()?;
            if let Some(last) = canonical.values.last_mut() {
                if last.x1 > run.x0 {
                    return Err(schema());
                }
                if last.x1 == run.x0 && last.coverage == run.coverage {
                    last.x1 = run.x1;
                    continue;
                }
            }
            canonical.push(run, probe)?;
        }
        let same = if let Some(last) = rows
            .values
            .last()
            .filter(|r| r.y1 == y && r.len == canonical.values.len())
        {
            let mut equal = true;
            for (a, b) in flat.values[last.start..last.start + last.len]
                .iter()
                .zip(&canonical.values)
            {
                probe.step()?;
                if a != b {
                    equal = false;
                    break;
                }
            }
            equal
        } else {
            false
        };
        if same {
            rows.values.last_mut().unwrap().y1 = next;
            continue;
        }
        let start = flat.values.len();
        for &run in &canonical.values {
            flat.push(run, probe)?;
        }
        rows.push(
            Rows {
                y0: y,
                y1: next,
                start,
                len: flat.values.len() - start,
            },
            probe,
        )?;
    }
    let mut bands = Buffer::new(scope, rows.values.len());
    for row in &rows.values {
        bands.push(
            Band {
                y0: row.y0,
                y1: row.y1,
                runs: &flat.values[row.start..row.start + row.len],
            },
            probe,
        )?;
    }
    A8Mask::from_bands_in_scope(
        domain,
        &bands.values,
        scope,
        Limits {
            work: probe.remaining.get(),
            ..limits
        },
        probe.check,
    )
}
impl A8Mask {
    pub fn orthogonal_checked(
        &self,
        source_size: (u32, u32),
        transform: OrthogonalTransform,
        budget: &SharedBudget,
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let probe = Probe {
            remaining: Cell::new(limits.work),
            check,
        };
        probe.step()?;
        let source =
            Rect::from_origin_size(0, 0, source_size.0, source_size.1).ok_or_else(schema)?;
        valid(source)?;
        let (w, h) = transform.dimensions(source_size.0, source_size.1);
        let domain = Rect::from_origin_size(0, 0, w, h).ok_or_else(schema)?;
        let scope = budget.scope(AllocationClass::CoverageMasks);
        let mut samples = Buffer::new(&scope, usize::try_from(limits.work).unwrap_or(usize::MAX));
        self.visit_rectangles_checked(probe.remaining.get(), &|| probe.step(), |y0, y1, run| {
            let Some(r) = (Rect {
                x0: run.x0,
                x1: run.x1,
                y0,
                y1,
            })
            .intersect(&source) else {
                return Ok(());
            };
            let a = transform
                .pixel_checked(r.x0, r.y0, source_size.0, source_size.1)
                .ok_or_else(limit)?;
            let b = transform
                .pixel_checked(r.x1 - 1, r.y1 - 1, source_size.0, source_size.1)
                .ok_or_else(limit)?;
            let rect = Rect {
                x0: a.0.min(b.0),
                y0: a.1.min(b.1),
                x1: a.0.max(b.0) + 1,
                y1: a.1.max(b.1) + 1,
            };
            samples.push(
                Sample {
                    rect,
                    value: run.coverage,
                },
                &probe,
            )
        })?;
        assemble(domain, &samples.values, &scope, limits, &probe)
    }
}
