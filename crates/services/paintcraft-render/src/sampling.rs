//! Small, bounded color samples from immutable document pixels, never screen caches.
use super::*;
use paintcraft_core::types::picker::SampleSize;
use paintcraft_document::Layer;

/// `None` samples the ordered visible image. `Some(layer)` samples raw finite
/// layer pixels, independently of visibility/opacity. Selection does not clip a
/// picker. Neighborhoods clip to the canvas; pixels outside a layer are transparent.
/// Average encoded straight RGBA channels, with half-up integer rounding.
pub fn sample_color(
    doc: &Document,
    layer: Option<&Layer>,
    at: (i64, i64),
    size: SampleSize,
    check: impl Fn() -> Result<()>,
) -> Result<Rgba> {
    check()?;
    if !doc.canvas_rect().contains(at.0, at.1) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "color sample is outside the canvas",
        ));
    }
    let radius = i64::from(size.side() / 2);
    // Center has already been bounded to a u32 canvas: these additions cannot overflow.
    let region = Rect::new(
        at.0 - radius,
        at.1 - radius,
        i64::from(size.side()),
        i64::from(size.side()),
    )
    .unwrap()
    .intersect(&doc.canvas_rect())
    .unwrap();
    let mut sums = [0u64; 4];
    if let Some(layer) = layer {
        let extent = layer.extent();
        for y in region.y0..region.y1 {
            check()?;
            for x in region.x0..region.x1 {
                let color = if extent.contains(x, y) {
                    layer.raster.get(x - layer.origin.0, y - layer.origin.1)
                } else {
                    Rgba::TRANSPARENT
                };
                for (sum, c) in sums.iter_mut().zip(color.0) {
                    *sum += u64::from(c);
                }
            }
        }
    } else {
        let pixels = render_region_cancellable(doc, region, &check)?;
        for row in pixels.chunks(region.width() as usize * 4) {
            check()?;
            for pixel in row.as_chunks::<4>().0 {
                for (sum, c) in sums.iter_mut().zip(pixel) {
                    *sum += u64::from(*c);
                }
            }
        }
    }
    check()?;
    let count = region.area() as u64;
    Ok(Rgba(sums.map(|n| ((n + count / 2) / count) as u8)))
}
