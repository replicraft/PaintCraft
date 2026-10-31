use super::*;
use paintcraft_core::types::resources::BudgetAccount;
use paintcraft_effects::spatial::{box_blur, mix_spatial_coverage};
use paintcraft_raster::RowSpan;
use std::mem::size_of;

pub(super) struct Workspace {
    stages: Vec<plan::Stage>,
    front: Vec<Rgba>,
    back: Vec<Rgba>,
    scratch: Vec<u64>,
    // Fields drop in declaration order: all allocations precede charge release.
    charge: BudgetAccount,
}

fn reserve<T>(length: usize) -> Result<Vec<T>> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| limit("Cannot allocate spatial workspace"))?;
    Ok(result)
}

impl Workspace {
    pub fn new(
        plan: &LayerAppearancePlan<'_>,
        admission: &WorkspaceAdmission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let quote = plan.quote;
        let mut charge = admission.scope.account();
        charge.resize(quote.bytes)?;
        check()?;
        let mut value = Self {
            stages: reserve(quote.stages)?,
            front: reserve(quote.pixels_per_buffer)?,
            back: reserve(quote.pixels_per_buffer)?,
            scratch: reserve(quote.scratch_elements)?,
            charge,
        };
        let actual = value
            .stages
            .capacity()
            .checked_mul(size_of::<plan::Stage>())
            .and_then(|bytes| {
                bytes.checked_add(value.front.capacity().checked_mul(size_of::<Rgba>())?)
            })
            .and_then(|bytes| {
                bytes.checked_add(value.back.capacity().checked_mul(size_of::<Rgba>())?)
            })
            .and_then(|bytes| {
                bytes.checked_add(value.scratch.capacity().checked_mul(size_of::<u64>())?)
            })
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or_else(|| limit("Spatial allocation capacities overflow"))?;
        if actual > admission.bytes_limit {
            return Err(limit("Spatial allocation capacity exceeds its limit"));
        }
        value.charge.resize(actual)?;
        value
            .front
            .resize(quote.pixels_per_buffer, Rgba::TRANSPARENT);
        value
            .back
            .resize(quote.pixels_per_buffer, Rgba::TRANSPARENT);
        value.scratch.resize(quote.scratch_elements, 0);
        plan.fill_stages(&mut value.stages, check)?;
        Ok(value)
    }

    pub fn extract(
        &mut self,
        plan: &LayerAppearancePlan<'_>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        let width = plan.input.width() as usize;
        for (row, y) in self
            .front
            .chunks_mut(width)
            .zip(plan.input.y0..plan.input.y1)
        {
            check()?;
            let mut offset = 0;
            for span in plan
                .layer
                .raster
                .row_spans(y as u32, plan.input.x0 as u32, width as u32)?
            {
                check()?;
                let count = span.pixel_count();
                let destination = &mut row[offset..offset + count];
                match span {
                    RowSpan::Uniform { color, .. } => destination.fill(color),
                    RowSpan::Pixels(bytes) => {
                        for (destination, source) in
                            destination.iter_mut().zip(bytes.as_chunks::<4>().0)
                        {
                            *destination = Rgba(*source);
                        }
                    }
                }
                offset += count;
            }
        }
        check()
    }

    pub fn evaluate(
        &mut self,
        plan: &LayerAppearancePlan<'_>,
        admission: &WorkspaceAdmission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        for stage in &self.stages {
            check()?;
            let filter = &plan.layer.filters.as_slice()[stage.index];
            let mask = filter.mask().filter(|mask| mask.enabled());
            let coverage = |x, y| mask.map_or(255, |mask| mask.coverage().coverage(x, y));
            let output_count = pixels(stage.output)?;
            if let Some(blur) = stage.blur {
                box_blur(
                    blur,
                    &self.front[..pixels(stage.input)?],
                    &mut self.back[..output_count],
                    &mut self.scratch,
                    check,
                )?;
                let input_width = stage.input.width() as usize;
                let output_width = stage.output.width() as usize;
                for (i, candidate) in self.back[..output_count].iter_mut().enumerate() {
                    if i.is_multiple_of(64) {
                        check()?;
                    }
                    let x = stage.output.x0 + (i % output_width) as i64;
                    let y = stage.output.y0 + (i / output_width) as i64;
                    let incoming = self.front[(y - stage.input.y0) as usize * input_width
                        + (x - stage.input.x0) as usize];
                    *candidate = mix_spatial_coverage(incoming, *candidate, coverage(x, y));
                }
                std::mem::swap(&mut self.front, &mut self.back);
            } else {
                let program =
                    crate::appearance::PointProgram::new(filter.descriptor(), Some(admission))?;
                let width = stage.output.width() as usize;
                for (i, pixel) in self.front[..output_count].iter_mut().enumerate() {
                    if i.is_multiple_of(64) {
                        check()?;
                    }
                    let x = stage.output.x0 + (i % width) as i64;
                    let y = stage.output.y0 + (i / width) as i64;
                    let m = coverage(x, y);
                    if m != 0 {
                        let candidate = program.evaluate(*pixel)?;
                        *pixel = paintcraft_effects::mix_point_coverage(*pixel, candidate, m);
                    }
                }
            }
        }
        check()
    }
    pub fn result(&self, length: usize) -> &[Rgba] {
        &self.front[..length]
    }
}
