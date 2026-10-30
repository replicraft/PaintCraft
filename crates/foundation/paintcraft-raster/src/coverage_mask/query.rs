//! Regional proofs inspect retained intervals, never every implicit pixel.
use super::*;
impl A8Mask {
    pub fn uniform_region_checked(
        &self,
        area: Rect,
        work: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Option<u8>> {
        let mut probe = Probe {
            remaining: Cell::new(work),
            check,
        };
        probe.step()?;
        valid(area)?;
        let expected = self.coverage(area.x0, area.y0);
        let mut y = area.y0;
        let mut bands = self.visible();
        while let Some(band) = bands.next_checked(&mut probe)? {
            let top = band.y0.max(area.y0);
            let bottom = band.y1.min(area.y1);
            if top >= bottom {
                continue;
            }
            if expected != 0 && top != y {
                return Ok(None);
            }
            let mut x = area.x0;
            let mut runs = band.runs();
            while let Some(run) = runs.next_checked(&mut probe)? {
                let left = run.x0.max(area.x0);
                let right = run.x1.min(area.x1);
                if left >= right {
                    continue;
                }
                if expected == 0 || run.coverage != expected || left != x {
                    return Ok(None);
                }
                x = right;
            }
            if expected != 0 && x != area.x1 {
                return Ok(None);
            }
            y = bottom;
        }
        probe.step()?;
        Ok(if expected == 0 || y == area.y1 {
            Some(expected)
        } else {
            None
        })
    }
    /// Canonical grouping for persistence and bounded consumers. Counting a
    /// clipped row is an explicit checked pass; no temporary row is allocated.
    pub fn visit_bands_checked(
        &self,
        work: u64,
        check: &dyn Fn() -> Result<()>,
        mut emit: impl FnMut(Visit) -> Result<()>,
    ) -> Result<()> {
        let mut probe = Probe {
            remaining: Cell::new(work),
            check,
        };
        probe.step()?;
        let mut bands = self.visible();
        while let Some(band) = bands.next_checked(&mut probe)? {
            let mut runs = band.runs();
            let mut count = 0;
            while runs.next_checked(&mut probe)?.is_some() {
                count += 1;
            }
            emit(Visit::Band {
                y0: band.y0,
                y1: band.y1,
                runs: count,
            })?;
            let mut runs = band.runs();
            while let Some(run) = runs.next_checked(&mut probe)? {
                emit(Visit::Run(run))?;
            }
        }
        probe.step()
    }
    /// Crop a finite source rectangle and rebase its origin. Disjoint coverage
    /// remains an active zero mask over the full new source, never None.
    pub fn cropped_checked(
        &self,
        area: Rect,
        work: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        valid(area)?;
        let domain = Rect {
            x0: 0,
            y0: 0,
            x1: area.width(),
            y1: area.height(),
        };
        let support = self
            .support
            .and_then(|r| r.intersect(&area))
            .map(|r| r.translated(-area.x0, -area.y0).ok_or_else(limit))
            .transpose()?;
        let offset = (
            self.offset.0.checked_sub(area.x0).ok_or_else(limit)?,
            self.offset.1.checked_sub(area.y0).ok_or_else(limit)?,
        );
        self.view(domain, support, offset, work, check)
    }
}
