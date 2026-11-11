//! Finite nearest-neighbor field resampling, preserving fractional A8 samples.
use super::mapping::{Buffer, Sample, assemble};
use super::*;
use crate::resampling::Geometry;
use paintcraft_core::types::{rotation::Rotation, stretch_skew::StretchSkew};
impl A8Mask {
    /// Map a source-local layer field using the same global geometry as its pixels.
    /// Both enabled and disabled stored masks use this operation.
    pub fn stretch_skew_checked(
        &self,
        geometry: StretchSkew,
        extent: Rect,
        budget: &SharedBudget,
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        self.resample_affine_checked(
            Geometry::StretchSkew(geometry),
            extent,
            budget,
            limits,
            check,
        )
    }
    /// Rotate a finite A8 layer field, including disabled stored masks.
    /// # Errors
    /// Preserves the same finite geometry/work/admission/callback errors.
    pub fn rotated_checked(
        &self,
        geometry: Rotation,
        extent: Rect,
        budget: &SharedBudget,
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        self.resample_affine_checked(Geometry::Rotation(geometry), extent, budget, limits, check)
    }
    fn resample_affine_checked(
        &self,
        geometry: Geometry,
        extent: Rect,
        budget: &SharedBudget,
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let destination = geometry.bounds(extent)?;
        let domain = Rect::from_origin_size(
            0,
            0,
            destination.width() as u32,
            destination.height() as u32,
        )
        .ok_or_else(schema)?;
        let probe = Probe {
            remaining: Cell::new(limits.work),
            check,
        };
        probe.step()?;
        let pixels = (domain.width() as u64)
            .checked_mul(domain.height() as u64)
            .ok_or_else(limit)?;
        if pixels > limits.work {
            return Err(limit());
        }
        let scope = budget.scope(AllocationClass::CoverageMasks);
        let mut samples = Buffer::new(
            &scope,
            limits
                .runs
                .min(usize::try_from(limits.work).unwrap_or(usize::MAX)),
        );
        for y in 0..domain.y1 {
            probe.step()?;
            let mut run = None;
            for x in 0..domain.x1 {
                if x % 64 == 0 {
                    (probe.check)()?;
                }
                probe.step()?;
                let (sx, sy) = geometry.source_pixel(destination.x0 + x, destination.y0 + y)?;
                let value = if extent.contains(sx, sy) {
                    self.coverage(sx - extent.x0, sy - extent.y0)
                } else {
                    0
                };
                match run {
                    Some((start, previous)) if previous == value => {
                        run = Some((start, previous));
                    }
                    previous => {
                        if let Some((start, value)) = previous
                            && value != 0
                        {
                            samples.push(
                                Sample {
                                    rect: Rect {
                                        x0: start,
                                        x1: x,
                                        y0: y,
                                        y1: y + 1,
                                    },
                                    value,
                                },
                                &probe,
                            )?;
                        }
                        run = Some((x, value));
                    }
                }
            }
            if let Some((start, value)) = run
                && value != 0
            {
                samples.push(
                    Sample {
                        rect: Rect {
                            x0: start,
                            x1: domain.x1,
                            y0: y,
                            y1: y + 1,
                        },
                        value,
                    },
                    &probe,
                )?;
            }
        }
        assemble(domain, &samples.values, &scope, limits, &probe)
    }
}
