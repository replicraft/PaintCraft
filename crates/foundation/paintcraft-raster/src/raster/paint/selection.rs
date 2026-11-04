//! Compile binary selection coverage into ordered writes exactly once.
use super::*;
use crate::selection::BinaryMask;

impl PaintPlan {
    /// Intersect layer-local writes with a finite document-space binary mask.
    /// Preserve blending and coverage exactly; replay must not mask this plan again.
    /// Output payload uses the original metadata realm and per-plan policy. Errors
    /// leave the input untouched; traversal consumes the caller's shared allowance.
    pub fn masked_checked(
        &self,
        mask: &BinaryMask,
        origin: (i64, i64),
        work: &mut u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        let mut output = Self {
            writes: Arc::new(writes::Writes {
                values: Vec::new(),
                charge: self.writes.charge.sibling(),
            }),
            metadata_limit: self.metadata_limit,
        };
        if self.is_empty() || mask.uniform_coverage() == Some(0) {
            return Ok(output);
        }
        if mask.uniform_coverage() == Some(255) {
            let mut all_inside = true;
            for write in self.writes.iter() {
                check()?;
                spend(work, 1)?;
                let area = document(write.area, origin)?;
                if area.intersect(&mask.domain()) != Some(area) {
                    all_inside = false;
                    break;
                }
            }
            if all_inside {
                return Ok(self.clone());
            }
        }
        for write in self.writes.iter() {
            check()?;
            spend(work, 1)?;
            let Some(area) = document(write.area, origin)?.intersect(&mask.domain()) else {
                continue;
            };
            if mask.uniform_coverage() == Some(255) {
                output.push(Write {
                    area: local(area, origin)?,
                    ..*write
                })?;
                continue;
            }
            for y in area.y0..area.y1 {
                check()?;
                let spent = mask.visit_coverage_row(
                    y,
                    area.x0,
                    area.x1,
                    *work,
                    check,
                    |x0, x1, coverage| {
                        let mut selected = *write;
                        if coverage == 255 {
                            // Preserve the original coverage denominator exactly.
                        } else if selected.destination_out {
                            selected.samples = ((u16::from(selected.samples) * u16::from(coverage)
                                + 127)
                                / 255) as u8;
                        } else if selected.mode == BlendMode::SourceOver {
                            selected.color.0[3] =
                                ((u16::from(selected.color.a()) * u16::from(coverage) + 127) / 255)
                                    as u8;
                        } else {
                            selected.samples = ((u32::from(selected.samples) * u32::from(coverage)
                                + u32::from(selected.sample_scale) / 2)
                                / u32::from(selected.sample_scale))
                                as u8;
                            selected.sample_scale = 255;
                        }
                        output.push(Write {
                            area: local(
                                Rect {
                                    x0,
                                    x1,
                                    y0: y,
                                    y1: y + 1,
                                },
                                origin,
                            )?,
                            ..selected
                        })
                    },
                )?;
                spend(work, spent)?;
            }
        }
        check()?;
        Ok(output)
    }
}
fn document(area: Rect, origin: (i64, i64)) -> Result<Rect> {
    area.translated(origin.0, origin.1).ok_or_else(exhausted)
}
fn local(area: Rect, origin: (i64, i64)) -> Result<Rect> {
    Ok(Rect {
        x0: area.x0.checked_sub(origin.0).ok_or_else(exhausted)?,
        x1: area.x1.checked_sub(origin.0).ok_or_else(exhausted)?,
        y0: area.y0.checked_sub(origin.1).ok_or_else(exhausted)?,
        y1: area.y1.checked_sub(origin.1).ok_or_else(exhausted)?,
    })
}
fn spend(work: &mut u64, amount: u64) -> Result<()> {
    *work = work.checked_sub(amount).ok_or_else(exhausted)?;
    Ok(())
}
fn exhausted() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "paint selection traversal or coordinate limit exceeded",
    )
}
