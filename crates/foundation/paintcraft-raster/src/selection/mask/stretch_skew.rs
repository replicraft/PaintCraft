//! Binary selection resampling uses the same finite inverse map as RGBA/A8.
use super::*;
use paintcraft_core::types::stretch_skew::StretchSkew;
impl BinaryMask {
    pub fn stretch_skew_with_remaining_checked(
        &self,
        geometry: StretchSkew,
        budget: &SharedBudget,
        mut limits: MaskLimits,
        work: &mut u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        spend(work, 1)?;
        let destination = geometry.bounds(geometry.source())?;
        valid_domain(destination)?;
        let pixels = (destination.width() as u64)
            .checked_mul(destination.height() as u64)
            .ok_or_else(limit)?;
        if pixels > (*work).min(limits.work) {
            return Err(limit());
        }
        limits.work = limits.work.min(*work);
        let allowance = limits.work;
        let mut build = builder::Builder::new(destination, false, budget, limits)?;
        let result = (|| {
            for y in destination.y0..destination.y1 {
                check()?;
                let mut start = None;
                for x in destination.x0..destination.x1 {
                    if (x - destination.x0) % 64 == 0 {
                        check()?;
                    }
                    build.spend(1)?;
                    let (sx, sy) = geometry.source_pixel(x, y)?;
                    let covered =
                        geometry.source().contains(sx, sy) && self.coverage(sx, sy) == 255;
                    if covered {
                        start.get_or_insert(x);
                    } else if let Some(x0) = start.take() {
                        build.span(
                            MaskSpan {
                                y,
                                x0,
                                x1: x - 1,
                                coverage: 255,
                            },
                            check,
                        )?;
                    }
                }
                if let Some(x0) = start {
                    build.span(
                        MaskSpan {
                            y,
                            x0,
                            x1: destination.x1 - 1,
                            coverage: 255,
                        },
                        check,
                    )?;
                }
            }
            Ok(())
        })();
        spend(work, allowance - build.remaining_work())?;
        result?;
        spend(work, build.retained_tiles() as u64)?;
        build.finish(check)
    }
}
