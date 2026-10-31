//! Source-local point appearances. No source tile or document is mutated.
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba};
use paintcraft_document::{FilterInstance, Layer};

/// A candidate binding is resolved once per stage/chunk, never per pixel.
pub(crate) enum PointProgram<'a> {
    Scalar(paintcraft_core::types::filters::Descriptor),
    Vibrance(paintcraft_core::types::filters::VibranceRgbV1),
    Levels(&'a crate::prepared::PreparedLevels),
    Exposure(&'a crate::prepared::PreparedExposure),
    Curves(&'a crate::prepared::PreparedCurves),
}
impl<'a> PointProgram<'a> {
    pub(crate) fn new(
        descriptor: paintcraft_core::types::filters::Descriptor,
        admission: Option<&'a crate::spatial::WorkspaceAdmission>,
    ) -> Result<Self> {
        match descriptor {
            paintcraft_core::types::filters::Descriptor::VibranceRgbV1(p) => {
                admission.ok_or_else(crate::direct::missing)?;
                Ok(Self::Vibrance(p))
            }
            paintcraft_core::types::filters::Descriptor::CurvesRgbV1(p) => admission
                .and_then(|a| a.prepared_curves())
                .and_then(|j| j.get(p))
                .map(Self::Curves)
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "Curves appearance lacks its frozen prepared policy",
                    )
                }),
            paintcraft_core::types::filters::Descriptor::LevelsRgbV1(policy) => admission
                .and_then(|admission| admission.prepared_levels())
                .and_then(|job| job.get(policy))
                .map(Self::Levels)
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "Levels appearance lacks its frozen prepared policy",
                    )
                }),
            paintcraft_core::types::filters::Descriptor::ExposureSrgbV1(policy) => admission
                .and_then(|admission| admission.prepared_programs())
                .and_then(|job| job.exposure(policy))
                .map(Self::Exposure)
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "Exposure appearance lacks its frozen prepared policy",
                    )
                }),
            other => Ok(Self::Scalar(other)),
        }
    }
    pub(crate) fn evaluate(&self, incoming: Rgba) -> Result<Rgba> {
        match self {
            Self::Vibrance(policy) => {
                paintcraft_effects::vibrance::evaluate_pixel(*policy, incoming)
            }
            Self::Scalar(descriptor) => {
                paintcraft_effects::evaluate_pixel(&[*descriptor], incoming)
            }
            Self::Levels(program) => Ok(program.evaluate_pixel(incoming)),
            Self::Exposure(program) => Ok(program.evaluate_pixel(incoming)),
            Self::Curves(program) => Ok(program.evaluate_pixel(incoming)),
        }
    }
}

fn mix(
    filter: &FilterInstance,
    program: &PointProgram<'_>,
    local: (i64, i64),
    incoming: Rgba,
) -> Result<Rgba> {
    let coverage = filter
        .mask()
        .filter(|mask| mask.enabled())
        .map_or(255, |mask| mask.coverage().coverage(local.0, local.1));
    if coverage == 0 {
        return Ok(incoming);
    }
    let candidate = program.evaluate(incoming)?;
    if coverage == 255 {
        Ok(candidate)
    } else {
        Ok(paintcraft_effects::mix_point_coverage(
            incoming, candidate, coverage,
        ))
    }
}

/// A varying mask can make an implicit default or uniform tile nonuniform.
/// Prove the full appearance before reusing one color for a source rectangle.
pub(crate) fn uniform(
    layer: &Layer,
    local: Rect,
    mut pixel: Rgba,
    check: &dyn Fn() -> Result<()>,
) -> Result<Option<Rgba>> {
    if crate::spatial::active(layer) {
        return Ok(None);
    }
    let admission = crate::spatial::current();
    for filter in layer.filters.iter().filter(|filter| filter.enabled()) {
        check()?;
        let program = PointProgram::new(filter.descriptor(), admission.as_ref())?;
        crate::direct::reserve(filter.descriptor(), 1, admission.as_ref())?;
        let candidate = program.evaluate(pixel)?;
        if candidate == pixel {
            continue;
        }
        let coverage = match filter.mask().filter(|mask| mask.enabled()) {
            None => 255,
            Some(mask) => {
                let Some(coverage) = mask.coverage().uniform_region_checked(
                    local,
                    paintcraft_raster::coverage_mask::Limits::default().work,
                    check,
                )?
                else {
                    return Ok(None);
                };
                coverage
            }
        };
        pixel = if coverage == 255 {
            candidate
        } else {
            paintcraft_effects::mix_point_coverage(pixel, candidate, coverage)
        };
    }
    Ok(Some(pixel))
}

/// Transform private, tightly packed RGBA rows in bounded chunks, advancing
/// source-local coordinates through implicit pixels too. Failed renders discard
/// partial output; source pixels and history are untouched.
pub(crate) fn pixels(
    layer: &Layer,
    local: (i64, i64),
    bytes: &mut [u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    if crate::spatial::active(layer) {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Spatial appearances require admitted regional evaluation",
        ));
    }
    let count = i64::try_from(bytes.len() / 4).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "Filter row length exceeds coordinates",
        )
    })?;
    if !bytes.len().is_multiple_of(4)
        || local.0 < 0
        || local.1 < 0
        || local.1 >= i64::from(layer.raster.height())
        || local
            .0
            .checked_add(count)
            .is_none_or(|end| end > i64::from(layer.raster.width()))
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Filter row lies outside its source",
        ));
    }
    if !layer.filters.iter().any(|filter| filter.enabled()) {
        return Ok(());
    }
    let admission = crate::spatial::current();
    for (index, chunk) in bytes.chunks_mut(64 * 4).enumerate() {
        check()?;
        let x0 = local.0 + (index * 64) as i64;
        for filter in layer.filters.iter().filter(|filter| filter.enabled()) {
            check()?;
            let program = PointProgram::new(filter.descriptor(), admission.as_ref())?;
            crate::direct::reserve(
                filter.descriptor(),
                (chunk.len() / 4) as u64,
                admission.as_ref(),
            )?;
            for (x, pixel) in chunk.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                *pixel = mix(filter, &program, (x0 + x as i64, local.1), Rgba(*pixel))?.0;
            }
        }
    }
    check()
}
