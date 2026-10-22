//! Bounded constant-colour stroke union; rows are independent, not ordered dabs.
use super::stroke_budget::Budget;
use super::*;
use std::{cmp::Reverse, collections::BinaryHeap, mem::size_of};
/// Provisional capacity allowance, excluding allocator/hash-control overhead.
pub const DEFAULT_STROKE_SCRATCH_BYTES: usize = 64 << 20;
const CHUNK: usize = 1024;
type Run = (i64, i64);
type Head = Reverse<(i64, i64, usize, usize)>;
#[derive(Debug)]
pub struct CheckedSpanSet {
    rows: HashMap<i64, Vec<Run>>,
    budget: Budget,
}
impl CheckedSpanSet {
    pub fn with_scratch_limit(bytes: usize) -> Self {
        Self {
            rows: HashMap::new(),
            budget: Budget::new(bytes),
        }
    }
    pub fn add_stamp(
        &mut self,
        cx: i64,
        cy: i64,
        n: i64,
        round: bool,
        clip: &Rect,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        check()?;
        if n < 1 {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "stroke diameter must be positive",
            ));
        }
        if clip.x0 >= clip.x1 || clip.y0 >= clip.y1 {
            return Ok(());
        }
        let (left, top) = (
            i128::from(cx) - (i128::from(n) - 1) / 2,
            i128::from(cy) - (i128::from(n) - 1) / 2,
        );
        let first = (i128::from(clip.y0) - top).max(0);
        let last = (i128::from(clip.y1) - top).min(i128::from(n));
        for j in first..last {
            check()?;
            let range = if round {
                ellipse_row(n, n, j as i64)
            } else {
                Some((0, n - 1))
            };
            if let Some((a, b)) = range {
                let x0 = (left + i128::from(a)).max(i128::from(clip.x0));
                let x1 = (left + i128::from(b)).min(i128::from(clip.x1) - 1);
                if x0 > x1 {
                    continue;
                }
                let y = (top + j) as i64;
                if self.rows.len() == self.rows.capacity() && !self.rows.contains_key(&y) {
                    self.budget.table(&mut self.rows, check)?;
                }
                let runs = self.rows.entry(y).or_default();
                self.budget.vector(runs, check)?;
                runs.push((x0 as i64, x1 as i64));
            }
        }
        Ok(())
    }
    /// Stream nonoverlapping runs in x order within each row. Row order is not
    /// specified: uniform per-stroke coverage is independent across rows.
    pub fn visit_spans(
        self,
        check: &dyn Fn() -> Result<()>,
        mut emit: impl FnMut(Span) -> Result<()>,
    ) -> Result<()> {
        check()?;
        let Self { rows, mut budget } = self;
        let mut heap: BinaryHeap<Head> = BinaryHeap::new();
        for (y, mut runs) in rows {
            check()?;
            for chunk in runs.chunks_mut(CHUNK) {
                check()?;
                chunk.sort_unstable();
            }
            let mut ordered = true;
            for start in (CHUNK..runs.len()).step_by(CHUNK) {
                check()?;
                if runs[start - 1] > runs[start] {
                    ordered = false;
                    break;
                }
            }
            if ordered {
                visit_runs(y, runs.iter().copied(), check, &mut emit)?;
            } else {
                budget.heap(&mut heap, runs.len().div_ceil(CHUNK), check)?;
                for start in (0..runs.len()).step_by(CHUNK) {
                    check()?;
                    let (a, b) = runs[start];
                    heap.push(Reverse((a, b, start + 1, (start + CHUNK).min(runs.len()))));
                }
                let merged = std::iter::from_fn(|| {
                    let Reverse((a, b, next, end)) = heap.pop()?;
                    if next < end {
                        let (a, b) = runs[next];
                        heap.push(Reverse((a, b, next + 1, end)));
                    }
                    Some((a, b))
                });
                visit_runs(y, merged, check, &mut emit)?;
            }
            budget.used -= runs.capacity() * size_of::<Run>();
        }
        check()
    }
}

fn visit_runs(
    y: i64,
    runs: impl Iterator<Item = Run>,
    check: &dyn Fn() -> Result<()>,
    emit: &mut impl FnMut(Span) -> Result<()>,
) -> Result<()> {
    let mut current: Option<Run> = None;
    for (work, (a, b)) in runs.enumerate() {
        if work.is_multiple_of(CHUNK) {
            check()?;
        }
        match current {
            Some((x0, x1)) if a <= x1.saturating_add(1) => current = Some((x0, x1.max(b))),
            Some((x0, x1)) => {
                check()?;
                emit(Span { y, x0, x1 })?;
                current = Some((a, b));
            }
            None => current = Some((a, b)),
        }
    }
    if let Some((x0, x1)) = current {
        check()?;
        emit(Span { y, x0, x1 })?;
    }
    Ok(())
}
