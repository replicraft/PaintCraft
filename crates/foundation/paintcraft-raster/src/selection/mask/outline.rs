//! Exact finite-field boundaries, clipped for display without closing the viewport.
use super::*;
use paintcraft_core::types::resources::AllocationClass;
use std::mem::size_of;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundarySegment {
    pub from: (i64, i64),
    pub to: (i64, i64),
}
#[derive(Clone, Copy, Debug)]
pub struct OutlineLimits {
    pub work: u64,
    pub segments: usize,
}
impl Default for OutlineLimits {
    fn default() -> Self {
        Self {
            work: 4_194_304,
            segments: 65_536,
        }
    }
}
/// Owns admitted display geometry, independent of the source mask's lifetime.
#[derive(Debug)]
pub struct MaskOutline {
    segments: Buffer<BoundarySegment>,
}
impl MaskOutline {
    pub fn segments(&self) -> &[BoundarySegment] {
        &self.segments.values
    }
}
impl BinaryMask {
    /// Pixel-edge boundaries, including holes and the true finite domain edge.
    /// Clip coordinates include their perimeter. A viewport through a selected
    /// area does not introduce an edge at its own boundary. All work and retained
    /// output/scratch capacities are bounded; failures return no partial outline.
    pub fn outline_checked(
        &self,
        clip: Rect,
        budget: &SharedBudget,
        limits: OutlineLimits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<MaskOutline> {
        check()?;
        valid_domain(clip)?;
        let mut output = MaskOutline {
            segments: Buffer::new(budget, limits.segments),
        };
        if self.uniform_coverage() == Some(0) {
            return Ok(output);
        }
        // Inclusive contact matters when a true edge lies on the viewport border.
        if clip.x1 < self.domain.x0
            || clip.x0 > self.domain.x1
            || clip.y1 < self.domain.y0
            || clip.y0 > self.domain.y1
        {
            return Ok(output);
        }
        let mut work = limits.work;
        if self.uniform_coverage() == Some(255) {
            let d = self.domain;
            for segment in [
                BoundarySegment {
                    from: (d.x0, d.y0),
                    to: (d.x1, d.y0),
                },
                BoundarySegment {
                    from: (d.x0, d.y1),
                    to: (d.x1, d.y1),
                },
                BoundarySegment {
                    from: (d.x0, d.y0),
                    to: (d.x0, d.y1),
                },
                BoundarySegment {
                    from: (d.x1, d.y0),
                    to: (d.x1, d.y1),
                },
            ] {
                check()?;
                spend(&mut work, 1)?;
                output.emit(segment, clip)?;
            }
            check()?;
            return Ok(output);
        }
        let first = clip.y0.max(self.domain.y0);
        let last = clip.y1.min(self.domain.y1);
        let mut previous = Buffer::new(budget, limits.segments);
        let mut current = Buffer::new(budget, limits.segments);
        self.outline_row(first - 1, clip, &mut previous, &mut work, check)?;
        for y in first..=last {
            check()?;
            spend(&mut work, 1)?;
            current.values.clear();
            self.outline_row(y, clip, &mut current, &mut work, check)?;
            horizontal(
                &previous.values,
                &current.values,
                y,
                clip,
                &mut output,
                &mut work,
                check,
            )?;
            if y < last {
                for &(x0, x1) in &current.values {
                    check()?;
                    spend(&mut work, 1)?;
                    for x in [x0, x1] {
                        if (clip.x0..=clip.x1).contains(&x) {
                            output.emit(
                                BoundarySegment {
                                    from: (x, y),
                                    to: (x, y + 1),
                                },
                                clip,
                            )?;
                        }
                    }
                }
            }
            std::mem::swap(&mut previous, &mut current);
        }
        check()?;
        Ok(output)
    }
    fn outline_row(
        &self,
        y: i64,
        clip: Rect,
        row: &mut Buffer<(i64, i64)>,
        work: &mut u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        // The one-pixel halo distinguishes a real run endpoint from a viewport cut.
        let spent = self.visit_row(y, clip.x0 - 1, clip.x1 + 1, *work, check, |x0, x1| {
            row.push((x0, x1))
        })?;
        spend(work, spent)
    }
}
fn events(runs: &[(i64, i64)], clip: Rect) -> impl Iterator<Item = i64> + '_ {
    runs.iter()
        .filter_map(move |&(a, b)| {
            let (a, b) = (a.max(clip.x0), b.min(clip.x1));
            (a < b).then_some([a, b])
        })
        .flatten()
}
#[allow(clippy::too_many_arguments)]
fn horizontal(
    a: &[(i64, i64)],
    b: &[(i64, i64)],
    y: i64,
    clip: Rect,
    output: &mut MaskOutline,
    work: &mut u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let (mut a, mut b) = (events(a, clip).peekable(), events(b, clip).peekable());
    let (mut active, mut start) = (false, clip.x0);
    while let Some(x) = a.peek().copied().into_iter().chain(b.peek().copied()).min() {
        check()?;
        spend(work, 1)?;
        if active && start < x {
            output.emit(
                BoundarySegment {
                    from: (start, y),
                    to: (x, y),
                },
                clip,
            )?;
        }
        while a.peek() == Some(&x) {
            a.next();
            active = !active;
        }
        while b.peek() == Some(&x) {
            b.next();
            active = !active;
        }
        start = x;
    }
    Ok(())
}
impl MaskOutline {
    fn emit(&mut self, mut edge: BoundarySegment, clip: Rect) -> Result<()> {
        if edge.from.1 == edge.to.1 {
            if !(clip.y0..=clip.y1).contains(&edge.from.1) {
                return Ok(());
            }
            edge.from.0 = edge.from.0.max(clip.x0);
            edge.to.0 = edge.to.0.min(clip.x1);
            if edge.from.0 >= edge.to.0 {
                return Ok(());
            }
        } else {
            if !(clip.x0..=clip.x1).contains(&edge.from.0) {
                return Ok(());
            }
            edge.from.1 = edge.from.1.max(clip.y0);
            edge.to.1 = edge.to.1.min(clip.y1);
            if edge.from.1 >= edge.to.1 {
                return Ok(());
            }
        }
        if let Some(previous) = self.segments.values.last_mut()
            && previous.to == edge.from
            && ((previous.from.0 == previous.to.0 && edge.from.0 == edge.to.0)
                || (previous.from.1 == previous.to.1 && edge.from.1 == edge.to.1))
        {
            previous.to = edge.to;
            return Ok(());
        }
        self.segments.push(edge)
    }
}
#[derive(Debug)]
struct Buffer<T> {
    values: Vec<T>,
    charge: BudgetAccount,
    limit: usize,
}
impl<T> Buffer<T> {
    fn new(budget: &SharedBudget, limit: usize) -> Self {
        Self {
            values: Vec::new(),
            charge: budget.account(AllocationClass::SelectionMasks),
            limit,
        }
    }
    fn push(&mut self, value: T) -> Result<()> {
        if self.values.len() == self.limit {
            return Err(limit());
        }
        if self.values.len() == self.values.capacity() {
            let old = self.values.capacity();
            let extra = old.max(4).min(self.limit - old);
            let bytes = (old + extra)
                .checked_mul(size_of::<T>())
                .ok_or_else(limit)?;
            self.charge.resize(bytes as u64)?;
            self.values.try_reserve_exact(extra).map_err(|_| limit())?;
            let actual = self
                .values
                .capacity()
                .checked_mul(size_of::<T>())
                .ok_or_else(limit)?;
            if self.values.capacity() > self.limit {
                drop(std::mem::take(&mut self.values));
                self.charge.resize(0)?;
                return Err(limit());
            }
            if let Err(error) = self.charge.resize(actual as u64) {
                drop(std::mem::take(&mut self.values));
                self.charge.resize(0)?;
                return Err(error);
            }
        }
        self.values.push(value);
        Ok(())
    }
}
