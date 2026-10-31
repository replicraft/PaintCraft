use super::*;
use paintcraft_core::types::filters::Descriptor;
use paintcraft_effects::spatial::BoxBlurPlan;
use std::mem::size_of;

/// Logical simultaneous workspace and deterministic sample/coverage work.
/// Bytes exclude the caller-owned final output, source raster and other jobs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkspaceQuote {
    pub bytes: u64,
    pub work: u64,
    pub stages: usize,
    pub pixels_per_buffer: usize,
    pub scratch_elements: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Stage {
    pub index: usize,
    pub output: Rect,
    pub input: Rect,
    pub blur: Option<BoxBlurPlan>,
}

/// No allocation is needed to plan. The borrowed layer freezes descriptors,
/// masks, implicit defaults and source versions throughout evaluation.
#[derive(Debug)]
pub struct LayerAppearancePlan<'a> {
    pub(super) layer: &'a Layer,
    pub(super) source: Rect,
    pub(super) input: Rect,
    pub(super) output: Rect,
    pub(super) quote: WorkspaceQuote,
}

fn stage(index: usize, descriptor: Descriptor, source: Rect, output: Rect) -> Result<Stage> {
    let blur = match descriptor {
        Descriptor::BoxBlurV1(policy) => Some(BoxBlurPlan::new(source, output, policy)?),
        Descriptor::InvertRgbV1
        | Descriptor::BrightnessContrastV1(_)
        | Descriptor::BlackWhiteRgbV1(_)
        | Descriptor::ThresholdLumaV1(_)
        | Descriptor::PosterizeRgbV1(_)
        | Descriptor::SolarizeRgbV1
        | Descriptor::LevelsRgbV1(_)
        | Descriptor::ExposureSrgbV1(_)
        | Descriptor::CurvesRgbV1(_)
        | Descriptor::VibranceRgbV1(_) => None,
    };
    Ok(Stage {
        index,
        output,
        input: blur.map_or(output, BoxBlurPlan::required_input),
        blur,
    })
}

impl<'a> LayerAppearancePlan<'a> {
    pub fn new(layer: &'a Layer, output: Rect, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        let source = Rect::from_origin_size(0, 0, layer.raster.width(), layer.raster.height())
            .ok_or_else(|| invalid("Appearance source is empty"))?;
        pixels(output)?;
        if source.intersect(&output) != Some(output) {
            return Err(invalid("Appearance output is outside its finite source"));
        }
        let mut input = output;
        let mut count = 0usize;
        let mut scratch = 0usize;
        let mut work = 0u64;
        for (index, filter) in layer
            .filters
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, filter)| filter.enabled())
        {
            check()?;
            let stage = stage(index, filter.descriptor(), source, input)?;
            count += 1;
            if count > paintcraft_document::filters::FILTERS_MAX {
                return Err(limit("Appearance stage count exceeds its bound"));
            }
            let visits = stage
                .blur
                .map_or(pixels(stage.output)?, BoxBlurPlan::sample_visits);
            let visits = u64::try_from(visits).map_err(|_| limit("Appearance work overflows"))?;
            work = work
                .checked_add(visits)
                .and_then(|value| value.checked_add(stage.output.area() as u64))
                .ok_or_else(|| limit("Appearance work overflows"))?;
            if matches!(filter.descriptor(), Descriptor::VibranceRgbV1(_)) {
                work = work
                    .checked_add(crate::direct::work(stage.output.area() as u64)?)
                    .ok_or_else(|| limit("Direct stage work overflows"))?;
            }
            scratch = scratch.max(stage.blur.map_or(0, BoxBlurPlan::scratch_elements));
            input = stage.input;
        }
        let capacity = pixels(input)?;
        work = work
            .checked_add(capacity as u64)
            .ok_or_else(|| limit("Appearance work overflows"))?;
        let bytes = capacity
            .checked_mul(size_of::<Rgba>() * 2)
            .and_then(|value| value.checked_add(scratch.checked_mul(size_of::<u64>())?))
            .and_then(|value| value.checked_add(count.checked_mul(size_of::<Stage>())?))
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| limit("Appearance workspace overflows"))?;
        check()?;
        Ok(Self {
            layer,
            source,
            input,
            output,
            quote: WorkspaceQuote {
                bytes,
                work,
                stages: count,
                pixels_per_buffer: capacity,
                scratch_elements: scratch,
            },
        })
    }
    /// Extra direct arithmetic bound, with the same reverse prefix halos. No
    /// new priced fields. Old-only layers return zero without callbacks.
    pub fn direct_work(&self, check: &dyn Fn() -> Result<()>) -> Result<u64> {
        if !crate::direct::active(self.layer) {
            return Ok(0);
        }
        let mut output = self.output;
        let mut work = 0u64;
        for (index, filter) in self
            .layer
            .filters
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, f)| f.enabled())
        {
            check()?;
            let value = stage(index, filter.descriptor(), self.source, output)?;
            if matches!(filter.descriptor(), Descriptor::VibranceRgbV1(_)) {
                work = work
                    .checked_add(crate::direct::work(value.output.area() as u64)?)
                    .ok_or_else(|| limit("Direct stage work overflows"))?;
            }
            output = value.input;
        }
        Ok(work)
    }
    pub fn required_input(&self) -> Rect {
        self.input
    }
    pub fn output(&self) -> Rect {
        self.output
    }
    pub fn quote(&self) -> WorkspaceQuote {
        self.quote
    }

    pub(super) fn fill_stages(
        &self,
        stages: &mut Vec<Stage>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        let mut output = self.output;
        for (index, filter) in self
            .layer
            .filters
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, filter)| filter.enabled())
        {
            check()?;
            let value = stage(index, filter.descriptor(), self.source, output)?;
            output = value.input;
            stages.push(value);
        }
        stages.reverse();
        Ok(())
    }
}
