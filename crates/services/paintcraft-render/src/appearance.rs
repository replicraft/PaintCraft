//! Source-local point appearances. No source tile or document is mutated.
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba};
use paintcraft_document::{FilterInstance, Layer};

fn mix(filter: &FilterInstance, local: (i64, i64), incoming: Rgba) -> Rgba {
    let coverage = filter
        .mask()
        .filter(|mask| mask.enabled())
        .map_or(255, |mask| mask.coverage().coverage(local.0, local.1));
    if coverage == 0 {
        return incoming;
    }
    let candidate = paintcraft_effects::evaluate_pixel(&[filter.descriptor()], incoming);
    if coverage == 255 {
        candidate
    } else {
        paintcraft_effects::mix_point_coverage(incoming, candidate, coverage)
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
    for filter in layer.filters.iter().filter(|filter| filter.enabled()) {
        check()?;
        let candidate = paintcraft_effects::evaluate_pixel(&[filter.descriptor()], pixel);
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
    for (index, chunk) in bytes.chunks_mut(64 * 4).enumerate() {
        check()?;
        let x0 = local.0 + (index * 64) as i64;
        for filter in layer.filters.iter().filter(|filter| filter.enabled()) {
            check()?;
            for (x, pixel) in chunk.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                *pixel = mix(filter, (x0 + x as i64, local.1), Rgba(*pixel)).0;
            }
        }
    }
    check()
}
