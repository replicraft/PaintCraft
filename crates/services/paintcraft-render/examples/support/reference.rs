//! Frozen row-copy scalar compositor from 95db5d0, for measured comparison only.
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba};
use paintcraft_document::Document;
use paintcraft_render::RENDER_PIXELS_MAX;

pub fn reference(doc: &Document, check: impl Fn() -> Result<()>) -> Result<Vec<u8>> {
    check()?;
    let (w, h) = (u64::from(doc.canvas.0), u64::from(doc.canvas.1));
    if w * h > RENDER_PIXELS_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "canvas exceeds the render admission limit",
        ));
    }
    let width = w as usize;
    let mut out = vec![0u8; width * h as usize * 4];
    let canvas = doc.canvas_rect();
    let mut span = Vec::new();
    for layer in doc.layers_in_order() {
        if !layer.visible || layer.opacity == 0 {
            continue;
        }
        let Some(area) = canvas.intersect(&layer.extent()) else {
            continue;
        };
        span.resize(area.width() as usize * 4, 0);
        for y in area.y0..area.y1 {
            check()?;
            layer
                .raster
                .read_span(y - layer.origin.1, area.x0 - layer.origin.0, &mut span);
            let row = &mut out[(y as usize * width + area.x0 as usize) * 4..][..span.len()];
            for (dst, src) in row
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(span.as_chunks::<4>().0)
            {
                *dst = Rgba(*src)
                    .with_opacity(layer.opacity)
                    .source_over(Rgba(*dst))
                    .0;
            }
        }
    }
    Ok(out)
}
