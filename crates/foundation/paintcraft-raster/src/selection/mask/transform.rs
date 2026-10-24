//! Exact orthogonal coverage mapping, independent of selected pixel storage.
use super::*;
use paintcraft_core::types::orthogonal::OrthogonalTransform;

impl BinaryMask {
    /// Map samples from `area` into a same-origin finite field. Quarter turns
    /// swap dimensions. Holes and samples outside the original domain stay zero.
    /// Uniform fully covered areas require no pixel storage, even at huge sizes.
    pub fn transformed_checked(
        &self,
        area: Rect,
        transform: OrthogonalTransform,
        budget: &SharedBudget,
        limits: MaskLimits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        self.transformed_with_remaining_checked(
            area,
            transform,
            budget,
            limits,
            &mut { limits.work },
            check,
        )
    }

    /// Share source traversal, run writes and final tile-accounting work across
    /// operations. Cancellation/admission failure releases every new private tile.
    pub fn transformed_with_remaining_checked(
        &self,
        area: Rect,
        transform: OrthogonalTransform,
        budget: &SharedBudget,
        mut limits: MaskLimits,
        work: &mut u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        valid_domain(area)?;
        limits.work = limits.work.checked_sub(1).ok_or_else(limit)?;
        spend(work, 1)?;
        let (width, height) = (area.width(), area.height());
        let (dw, dh) = match transform {
            OrthogonalTransform::RotateRight | OrthogonalTransform::RotateLeft => (height, width),
            _ => (width, height),
        };
        let destination = Rect {
            x0: area.x0,
            y0: area.y0,
            x1: area.x0.checked_add(dw).ok_or_else(limit)?,
            y1: area.y0.checked_add(dh).ok_or_else(limit)?,
        };
        valid_domain(destination).map_err(|_| limit())?;
        let overlap = self.domain.intersect(&area);
        if overlap.is_none() || self.uniform_coverage() == Some(0) {
            return Self::uniform(destination, false);
        }
        if self.uniform_coverage() == Some(255) && overlap == Some(area) {
            return Self::uniform(destination, true);
        }
        limits.work = limits.work.min(*work);
        let allowance = limits.work;
        let mut build = builder::Builder::new(destination, false, budget, limits)?;
        let prepared = (|| {
            for dy in 0..dh {
                check()?;
                let mut start = None;
                for dx in 0..dw {
                    if dx % 64 == 0 {
                        check()?;
                    }
                    build.spend(1)?;
                    let (sx, sy) = match transform {
                        OrthogonalTransform::RotateRight => (dy, height - 1 - dx),
                        OrthogonalTransform::RotateLeft => (width - 1 - dy, dx),
                        OrthogonalTransform::Rotate180 => (width - 1 - dx, height - 1 - dy),
                        OrthogonalTransform::FlipHorizontal => (width - 1 - dx, dy),
                        OrthogonalTransform::FlipVertical => (dx, height - 1 - dy),
                    };
                    let selected = self.coverage(area.x0 + sx, area.y0 + sy) == 255;
                    if selected {
                        start.get_or_insert(dx);
                    } else if let Some(first) = start.take() {
                        build.span(
                            MaskSpan {
                                y: area.y0 + dy,
                                x0: area.x0 + first,
                                x1: area.x0 + dx - 1,
                                coverage: 255,
                            },
                            check,
                        )?;
                    }
                }
                if let Some(first) = start {
                    build.span(
                        MaskSpan {
                            y: area.y0 + dy,
                            x0: area.x0 + first,
                            x1: area.x0 + dw - 1,
                            coverage: 255,
                        },
                        check,
                    )?;
                }
            }
            Ok(())
        })();
        spend(work, allowance - build.remaining_work())?;
        prepared?;
        // Finalization charges one visit per retained tile. Charge the caller
        // before handing ownership over; a later cancellation may conservatively
        // consume that finalization allowance without returning a partial mask.
        spend(work, build.retained_tiles() as u64)?;
        build.finish(check)
    }
}
