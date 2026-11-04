//! Freeze finite live selection coverage without walking implicit canvas pixels.
use super::mapping::{Buffer, Sample, assemble, sort};
use super::*;
use crate::Selection;
impl A8Mask {
    pub fn from_selection(
        selection: &Selection,
        source_extent: Rect,
        budget: &SharedBudget,
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        Self::from_selection_measured(selection, source_extent, budget, limits, check)
            .map(|(mask, _)| mask)
    }
    /// Same field, per-call limits and callback cadence as from_selection.
    /// Successful work includes all callback steps (also terminal canonicalizing
    /// builder/hash callbacks) plus weighted BinaryMask row traversal units.
    /// Callers carrying one transaction allowance debit this total ONCE. Pass a
    /// cancellation-only callback, not another callback-count work debit.
    pub fn from_selection_measured(
        selection: &Selection,
        source_extent: Rect,
        budget: &SharedBudget,
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<(Self, u64)> {
        Self::capture_selection_measured(
            selection,
            source_extent,
            || budget.scope(AllocationClass::CoverageMasks),
            limits,
            check,
        )
    }
    /// Capture into the exact supplied scope, retaining its reservation and
    /// shared footprint for scratch, canonical construction and final storage.
    pub fn from_selection_in_scope(
        selection: &Selection,
        source_extent: Rect,
        scope: &BudgetScope,
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        Self::from_selection_measured_in_scope(selection, source_extent, scope, limits, check)
            .map(|(mask, _)| mask)
    }
    /// Same canonical bytes, callbacks and complete weighted work as the budget
    /// API. No underlying-budget escape or fresh capacity reservation occurs.
    pub fn from_selection_measured_in_scope(
        selection: &Selection,
        source_extent: Rect,
        scope: &BudgetScope,
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<(Self, u64)> {
        Self::capture_selection_measured(selection, source_extent, || scope.clone(), limits, check)
    }
    fn capture_selection_measured(
        selection: &Selection,
        source_extent: Rect,
        scope: impl FnOnce() -> BudgetScope,
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<(Self, u64)> {
        let callbacks = Cell::new(0u64);
        let weighted = Cell::new(0u64);
        let checked = || {
            check()?;
            callbacks.set(callbacks.get().checked_add(1).ok_or_else(limit)?);
            Ok(())
        };
        let probe = Probe {
            remaining: Cell::new(limits.work),
            check: &checked,
        };
        probe.step()?;
        valid(source_extent)?;
        let domain = Rect {
            x0: 0,
            y0: 0,
            x1: source_extent.width(),
            y1: source_extent.height(),
        };
        valid(domain)?;
        let scope = scope();
        let mut samples = Buffer::new(&scope, usize::try_from(limits.work).unwrap_or(usize::MAX));
        let mut emit = |rect: Rect, value: u8| -> Result<()> {
            if value == 0 {
                return Ok(());
            }
            if let Some(rect) = rect.intersect(&source_extent) {
                let rect = rect
                    .translated(-source_extent.x0, -source_extent.y0)
                    .ok_or_else(limit)?;
                samples.push(Sample { rect, value }, &probe)?;
            }
            Ok(())
        };
        match selection {
            Selection::Inactive => {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "An active selection is required for a filter mask",
                ));
            }
            Selection::Empty => {}
            Selection::Rect(rect) => emit(*rect, 255)?,
            Selection::Mask(mask) => {
                if let Some(area) = mask.domain().intersect(&source_extent) {
                    if let Some(value) = mask.uniform_coverage() {
                        emit(area, value)?;
                    } else {
                        let mut tiles =
                            Buffer::new(&scope, usize::try_from(limits.work).unwrap_or(usize::MAX));
                        mask.stored_tile_rectangles_checked(
                            area,
                            probe.remaining.get(),
                            &|| probe.step(),
                            |r| tiles.push((r.y0, r.x0, r.y1, r.x1), &probe),
                        )?;
                        sort(&mut tiles.values, &scope, &probe)?;
                        let default = mask.default_coverage();
                        let mut at = 0;
                        let mut y = area.y0;
                        while at < tiles.values.len() {
                            probe.step()?;
                            let top = tiles.values[at].0;
                            let bottom = tiles.values[at].2;
                            if y < top {
                                emit(
                                    Rect {
                                        x0: area.x0,
                                        x1: area.x1,
                                        y0: y,
                                        y1: top,
                                    },
                                    default,
                                )?;
                            }
                            let start = at;
                            while at < tiles.values.len()
                                && tiles.values[at].0 == top
                                && tiles.values[at].2 == bottom
                            {
                                probe.step()?;
                                at += 1;
                            }
                            for row in top..bottom {
                                probe.step()?;
                                let mut x = area.x0;
                                for &(_, left, _, right) in &tiles.values[start..at] {
                                    probe.step()?;
                                    if x < left {
                                        emit(
                                            Rect {
                                                x0: x,
                                                x1: left,
                                                y0: row,
                                                y1: row + 1,
                                            },
                                            default,
                                        )?;
                                    }
                                    let spent = mask.visit_coverage_row(
                                        row,
                                        left,
                                        right,
                                        probe.remaining.get(),
                                        &|| probe.step(),
                                        |x0, x1, coverage| {
                                            emit(
                                                Rect {
                                                    x0,
                                                    x1,
                                                    y0: row,
                                                    y1: row + 1,
                                                },
                                                coverage,
                                            )
                                        },
                                    )?;
                                    probe.spend(spent)?;
                                    weighted
                                        .set(weighted.get().checked_add(spent).ok_or_else(limit)?);
                                    x = right;
                                }
                                if x < area.x1 {
                                    emit(
                                        Rect {
                                            x0: x,
                                            x1: area.x1,
                                            y0: row,
                                            y1: row + 1,
                                        },
                                        default,
                                    )?;
                                }
                            }
                            y = bottom;
                        }
                        if y < area.y1 {
                            emit(
                                Rect {
                                    x0: area.x0,
                                    x1: area.x1,
                                    y0: y,
                                    y1: area.y1,
                                },
                                default,
                            )?;
                        }
                    }
                }
            }
        }
        drop(emit);
        let mask = assemble(domain, &samples.values, &scope, limits, &probe)?;
        let work = callbacks
            .get()
            .checked_add(weighted.get())
            .ok_or_else(limit)?;
        Ok((mask, work))
    }
}

#[cfg(test)]
mod tests;
