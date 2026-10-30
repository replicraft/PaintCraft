//! The flat render baseline of `paintcraft.core/default-v1`.
//!
//! Start from a canvas-sized buffer of transparent black. For each visible root
//! layer, bottom to top, apply layer opacity to every sample inside the layer's
//! bounds and composite it with straight RGBA8 source-over. Output is straight
//! RGBA8 encoded sRGB, row-major.

use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba};

use paintcraft_document::Document;
use paintcraft_raster::RowSpan;

mod appearance;
mod kernels;

pub mod parallel;

pub mod cache;
pub mod clipboard;
pub mod display;
pub mod sampling;
pub mod thumbnail;

/// Largest canvas this renderer will materialize (provisional admission limit).
pub const RENDER_PIXELS_MAX: u64 = 67_108_864;

pub fn render_flat(doc: &Document) -> Result<Vec<u8>> {
    render_cancellable(doc, || Ok(()))
}

pub fn render_cancellable(doc: &Document, check: impl Fn() -> Result<()>) -> Result<Vec<u8>> {
    render_region_cancellable(doc, doc.canvas_rect(), check)
}

/// Exact full-resolution RGBA8 for a nonempty document-space rectangle inside
/// the canvas. Only the requested output is allocated, even on enormous canvases.
pub fn render_region(doc: &Document, region: Rect) -> Result<Vec<u8>> {
    render_region_cancellable(doc, region, || Ok(()))
}

pub(crate) fn validate_region(doc: &Document, region: Rect) -> Result<()> {
    if region.x0 < 0
        || region.y0 < 0
        || region.x1 <= region.x0
        || region.y1 <= region.y0
        || region.x1 > i64::from(doc.canvas.0)
        || region.y1 > i64::from(doc.canvas.1)
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "render region must be nonempty and inside the canvas",
        ));
    }
    Ok(())
}

pub(crate) fn region_bytes(doc: &Document, region: Rect) -> Result<usize> {
    validate_region(doc, region)?;
    let pixels = region.area();
    if pixels > u128::from(RENDER_PIXELS_MAX) {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "region exceeds the render admission limit",
        ));
    }
    usize::try_from(pixels * 4)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "render buffer length overflows"))
}

pub(crate) fn output_buffer(bytes: usize) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.try_reserve_exact(bytes)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "cannot allocate render output"))?;
    out.resize(bytes, 0);
    Ok(out)
}

/// Cancellable regional composition; errors never return a partial image.
pub fn render_region_cancellable(
    doc: &Document,
    region: Rect,
    check: impl Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    let bytes = region_bytes(doc, region)?;
    let mut out = output_buffer(bytes)?;
    compose_roots_into(doc, region, 0..doc.roots.len(), &mut out, &check)?;
    Ok(out)
}

// Preserve root order and each intermediate RGBA8 rounding boundary when
// resuming above a cached prefix. Never regroup source-over operations.
pub(crate) fn compose_roots_into(
    doc: &Document,
    region: Rect,
    roots: std::ops::Range<usize>,
    out: &mut [u8],
    check: &impl Fn() -> Result<()>,
) -> Result<()> {
    let width = region.width() as usize;
    for layer in doc.layers_in_order().skip(roots.start).take(roots.len()) {
        if !layer.visible || layer.opacity == 0 {
            continue;
        }
        let Some(area) = region.intersect(&layer.extent()) else {
            continue;
        };
        for y in area.y0..area.y1 {
            check()?;
            let row = &mut out
                [((y - region.y0) as usize * width + (area.x0 - region.x0) as usize) * 4..]
                [..area.width() as usize * 4];
            compose_row(layer, y, area.x0, row, check)?;
        }
    }
    Ok(())
}

// The caller intersects the row with the layer extent before borrowing it.
pub(crate) fn compose_row(
    layer: &paintcraft_document::Layer,
    y: i64,
    x: i64,
    mut row: &mut [u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    for span in layer.raster.row_spans(
        (y - layer.origin.1) as u32,
        (x - layer.origin.0) as u32,
        (row.len() / 4) as u32,
    )? {
        let (dst, rest) = row.split_at_mut(span.pixel_count() * 4);
        row = rest;
        match span {
            RowSpan::Uniform { color, .. } => {
                let color = appearance::pixel(layer, color, check)?.with_opacity(layer.opacity);
                match color.a() {
                    0 => {}
                    255 => dst.as_chunks_mut::<4>().0.fill(color.0),
                    _ => {
                        for pixel in dst.as_chunks_mut::<4>().0 {
                            *pixel = color.source_over(Rgba(*pixel)).0;
                        }
                    }
                }
            }
            RowSpan::Pixels(src) => {
                if layer.filters.is_empty() {
                    kernels::dense(dst, src, layer.opacity);
                } else {
                    // Only private bounded scratch is transformed. Source tiles
                    // remain shared and their versions never change for a filter.
                    let mut scratch = [[0u8; 4]; 64];
                    for (dst, src) in dst.chunks_mut(256).zip(src.chunks(256)) {
                        check()?;
                        let temporary = scratch[..src.len() / 4].as_flattened_mut();
                        temporary.copy_from_slice(src);
                        appearance::pixels(layer, temporary, check)?;
                        kernels::dense(dst, temporary, layer.opacity);
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_document::{POLICY, new_document};
    use paintcraft_editor::session::Session;

    fn run(s: &mut Session, ops: &str) {
        let t = s.ticket();
        let text = format!(
            r#"{{"schema":"paintcraft.batch/v1","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":{ops}}}"#,
            t.state_id, t.publication_epoch
        );
        s.apply_json(text.as_bytes()).unwrap();
    }

    #[test]
    fn no_layers_renders_transparent_black() {
        let s = Session::new(new_document(3, 2, None, 0, POLICY).unwrap());
        assert_eq!(render_flat(s.document()).unwrap(), vec![0u8; 24]);
    }

    #[test]
    fn opacity_visibility_and_clipping() {
        let mut s =
            Session::new(new_document(3, 1, Some(Rgba([255, 255, 255, 255])), 0, POLICY).unwrap());
        run(
            &mut s,
            r##"[{"op":"layer.add","ref":"ink","name":"Ink","width":4,"height":1,"origin":{"x":-1,"y":0},"default_pixel":"#FF000080"},
                {"op":"layer.set_properties","layer_ref":{"ref":"ink"},"opacity_u8":128}]"##,
        );
        // Layer pixel (255,0,0,128) at opacity 128 has alpha 64 over opaque white.
        let red64 = Rgba([255, 0, 0, 64])
            .source_over(Rgba([255, 255, 255, 255]))
            .0;
        let img = render_flat(s.document()).unwrap();
        assert_eq!(img.as_chunks::<4>().0, [red64, red64, red64]);
        let ink = s.document().roots[1];
        run(
            &mut s,
            &format!(
                r#"[{{"op":"layer.set_properties","layer_ref":{{"id":"{ink}"}},"visible":false}}]"#
            ),
        );
        assert!(
            render_flat(s.document())
                .unwrap()
                .chunks(4)
                .all(|p| p == [255; 4])
        );
    }

    #[test]
    fn cancelled_render_never_returns_a_partial_composite() {
        let doc = new_document(100, 100, Some(Rgba([255; 4])), 0, POLICY).unwrap();
        let checks = std::cell::Cell::new(0);
        let result = render_cancellable(&doc, || {
            checks.set(checks.get() + 1);
            if checks.get() >= 3 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "test cancellation",
                ))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(checks.get(), 3);
        assert_eq!(render_flat(&doc).unwrap(), vec![255; 100 * 100 * 4]);
    }

    #[test]
    fn oversized_canvas_is_refused() {
        let doc = new_document(16_384, 16_384, None, 0, POLICY).unwrap();
        assert_eq!(
            render_flat(&doc).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
}

#[cfg(feature = "gpu")]
pub mod gpu;
