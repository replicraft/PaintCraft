//! Reframing explicitly zeros newly exposed areas, even after an earlier clip.
use super::*;
impl BinaryMask {
    pub fn reframed(
        &self,
        domain: Rect,
        budget: &SharedBudget,
        limits: MaskLimits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        valid_domain(domain)?;
        if domain == self.domain {
            return Ok(self.clone());
        }
        let Some(overlap) = self.domain.intersect(&domain) else {
            return Self::uniform(domain, false);
        };
        if self.uniform_coverage() == Some(0) {
            return Self::uniform(domain, false);
        }
        if overlap == domain {
            return Ok(self.clipped(domain)?.expect("overlap checked"));
        }
        let mut build = builder::Builder::new(domain, false, budget, limits)?;
        let mut work = limits.work;
        for y in overlap.y0..overlap.y1 {
            let spent = self.visit_row(y, overlap.x0, overlap.x1, work, check, |x0, x1| {
                build.span(
                    MaskSpan {
                        y,
                        x0,
                        x1: x1 - 1,
                        coverage: 255,
                    },
                    check,
                )
            })?;
            spend(&mut work, spent)?;
        }
        build.finish(check)
    }
}
