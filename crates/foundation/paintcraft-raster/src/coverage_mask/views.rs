//! Single-depth immutable coordinate views and canonical visible row traversal.
use super::*;
#[derive(Clone)]
pub(super) struct VisibleRuns<'a> {
    source: &'a [Run],
    index: usize,
    shift: i64,
    x0: i64,
    x1: i64,
}
impl VisibleRuns<'_> {
    pub(super) fn next_checked(&mut self, probe: &mut Probe<'_>) -> Result<Option<Run>> {
        while let Some(run) = self.source.get(self.index) {
            probe.step()?;
            self.index += 1;
            let x0 = (run.x0 + self.shift).max(self.x0);
            let x1 = (run.x1 + self.shift).min(self.x1);
            if x0 < x1 {
                return Ok(Some(Run {
                    x0,
                    x1,
                    coverage: run.coverage,
                }));
            }
        }
        Ok(None)
    }
    fn equal_checked(mut self, mut other: Self, probe: &mut Probe<'_>) -> Result<bool> {
        loop {
            match (self.next_checked(probe)?, other.next_checked(probe)?) {
                (None, None) => return Ok(true),
                (Some(a), Some(b)) if a == b => {}
                _ => return Ok(false),
            }
        }
    }
}
impl Iterator for VisibleRuns<'_> {
    type Item = Run;
    fn next(&mut self) -> Option<Run> {
        while let Some(run) = self.source.get(self.index) {
            self.index += 1;
            let x0 = (run.x0 + self.shift).max(self.x0);
            let x1 = (run.x1 + self.shift).min(self.x1);
            if x0 < x1 {
                return Some(Run {
                    x0,
                    x1,
                    coverage: run.coverage,
                });
            }
        }
        None
    }
}
#[derive(Clone)]
pub(super) struct VisibleBand<'a> {
    pub(super) y0: i64,
    pub(super) y1: i64,
    runs: VisibleRuns<'a>,
}
impl VisibleBand<'_> {
    pub(super) fn runs(&self) -> VisibleRuns<'_> {
        self.runs.clone()
    }
}
impl PartialEq for VisibleBand<'_> {
    fn eq(&self, b: &Self) -> bool {
        self.y0 == b.y0 && self.y1 == b.y1 && self.runs().eq(b.runs())
    }
}
impl Eq for VisibleBand<'_> {}
pub(super) struct Visible<'a> {
    mask: &'a A8Mask,
    index: usize,
    pending: Option<VisibleBand<'a>>,
}
impl<'a> Visible<'a> {
    fn candidate(&mut self) -> Option<VisibleBand<'a>> {
        let storage = self.mask.storage.as_ref()?;
        let support = self.mask.support?;
        while let Some(band) = storage.bands.get(self.index) {
            self.index += 1;
            let y0 = (band.y0 + self.mask.offset.1).max(support.y0);
            let y1 = (band.y1 + self.mask.offset.1).min(support.y1);
            if y0 >= y1 {
                continue;
            }
            let runs = VisibleRuns {
                source: &storage.runs[band.start..band.start + band.len],
                index: 0,
                shift: self.mask.offset.0,
                x0: support.x0,
                x1: support.x1,
            };
            if runs.clone().next().is_some() {
                return Some(VisibleBand { y0, y1, runs });
            }
        }
        None
    }
}
impl<'a> Visible<'a> {
    fn candidate_checked(&mut self, probe: &mut Probe<'_>) -> Result<Option<VisibleBand<'a>>> {
        let Some(storage) = &self.mask.storage else {
            return Ok(None);
        };
        let Some(support) = self.mask.support else {
            return Ok(None);
        };
        while let Some(band) = storage.bands.get(self.index) {
            probe.step()?;
            self.index += 1;
            let y0 = (band.y0 + self.mask.offset.1).max(support.y0);
            let y1 = (band.y1 + self.mask.offset.1).min(support.y1);
            if y0 >= y1 {
                continue;
            }
            let runs = VisibleRuns {
                source: &storage.runs[band.start..band.start + band.len],
                index: 0,
                shift: self.mask.offset.0,
                x0: support.x0,
                x1: support.x1,
            };
            if runs.clone().next_checked(probe)?.is_some() {
                return Ok(Some(VisibleBand { y0, y1, runs }));
            }
        }
        Ok(None)
    }
    pub(super) fn next_checked(
        &mut self,
        probe: &mut Probe<'_>,
    ) -> Result<Option<VisibleBand<'a>>> {
        let band = self.pending.take();
        let Some(mut band) = (match band {
            Some(b) => Some(b),
            None => self.candidate_checked(probe)?,
        }) else {
            return Ok(None);
        };
        while let Some(next) = self.candidate_checked(probe)? {
            if band.y1 == next.y0 && band.runs().equal_checked(next.runs(), probe)? {
                band.y1 = next.y1;
            } else {
                self.pending = Some(next);
                break;
            }
        }
        Ok(Some(band))
    }
}
impl<'a> Iterator for Visible<'a> {
    type Item = VisibleBand<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut band = self.pending.take().or_else(|| self.candidate())?;
        while let Some(next) = self.candidate() {
            if band.y1 == next.y0 && band.runs().eq(next.runs()) {
                band.y1 = next.y1;
            } else {
                self.pending = Some(next);
                break;
            }
        }
        Some(band)
    }
}
impl A8Mask {
    pub(super) fn visible(&self) -> Visible<'_> {
        Visible {
            mask: self,
            index: 0,
            pending: None,
        }
    }
    fn view(
        &self,
        domain: Rect,
        support: Option<Rect>,
        offset: (i64, i64),
        work: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let mut probe = Probe {
            remaining: work,
            check,
        };
        probe.step()?;
        valid(domain)?;
        let mut result = Self {
            domain,
            support,
            offset,
            storage: self.storage.clone(),
            digest: [0; 32],
        };
        if result.visible().next_checked(&mut probe)?.is_none() {
            result.storage = None;
            result.support = None;
            result.offset = (0, 0);
        }
        result.digest = result.hash(&mut probe)?;
        probe.step()?;
        Ok(result)
    }
    pub fn translated_checked(
        &self,
        dx: i64,
        dy: i64,
        work: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        let domain = self.domain.translated(dx, dy).ok_or_else(limit)?;
        let support = self
            .support
            .map(|r| r.translated(dx, dy).ok_or_else(limit))
            .transpose()?;
        let offset = (
            self.offset.0.checked_add(dx).ok_or_else(limit)?,
            self.offset.1.checked_add(dy).ok_or_else(limit)?,
        );
        self.view(domain, support, offset, work, check)
    }
    /// Return None only for disjoint finite domains. Coverage-zero overlap is
    /// still an active zero field. Clipping never exposes hidden source storage.
    pub fn clipped_checked(
        &self,
        area: Rect,
        work: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Option<Self>> {
        check()?;
        valid(area)?;
        let Some(domain) = self.domain.intersect(&area) else {
            return Ok(None);
        };
        self.view(
            domain,
            self.support.and_then(|r| r.intersect(&domain)),
            self.offset,
            work,
            check,
        )
        .map(Some)
    }
    /// Newly exposed samples are zero, including after clip-then-expand.
    pub fn reframed_checked(
        &self,
        domain: Rect,
        work: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        valid(domain)?;
        self.view(
            domain,
            self.support
                .and_then(|r| r.intersect(&self.domain))
                .and_then(|r| r.intersect(&domain)),
            self.offset,
            work,
            check,
        )
    }
    /// Visit constant nonzero rectangles in canonical order, without walking
    /// their implicit pixels or allocating a dense canvas.
    pub fn visit_rectangles_checked(
        &self,
        work: u64,
        check: &dyn Fn() -> Result<()>,
        mut emit: impl FnMut(i64, i64, Run) -> Result<()>,
    ) -> Result<()> {
        let mut probe = Probe {
            remaining: work,
            check,
        };
        probe.step()?;
        let mut cursor = self.visible();
        while let Some(band) = cursor.next_checked(&mut probe)? {
            probe.step()?;
            let mut runs = band.runs();
            while let Some(run) = runs.next_checked(&mut probe)? {
                emit(band.y0, band.y1, run)?;
            }
        }
        probe.step()
    }
}
