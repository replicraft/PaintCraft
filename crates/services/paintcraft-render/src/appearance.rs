//! Source-local point appearances. No source tile or document is mutated.
use paintcraft_core::types::{Result, Rgba};
use paintcraft_document::Layer;

pub(crate) fn pixel(
    layer: &Layer,
    mut pixel: Rgba,
    check: &dyn Fn() -> Result<()>,
) -> Result<Rgba> {
    for filter in layer.filters.iter().filter(|filter| filter.enabled()) {
        check()?;
        pixel = paintcraft_effects::evaluate_pixel(&[filter.descriptor()], pixel);
    }
    Ok(pixel)
}

/// Transform private, tightly packed RGBA storage in bounded chunks. A failed
/// evaluation leaves partial private output which the enclosing render discards.
pub(crate) fn pixels(
    layer: &Layer,
    bytes: &mut [u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    if !layer.filters.iter().any(|filter| filter.enabled()) {
        return Ok(());
    }
    for chunk in bytes.chunks_mut(64 * 4) {
        check()?;
        for filter in layer.filters.iter().filter(|filter| filter.enabled()) {
            check()?;
            for pixel in chunk.as_chunks_mut::<4>().0 {
                *pixel = paintcraft_effects::evaluate_pixel(&[filter.descriptor()], Rgba(*pixel)).0;
            }
        }
    }
    check()
}
