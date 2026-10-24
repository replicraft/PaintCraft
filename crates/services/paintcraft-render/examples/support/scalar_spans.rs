//! Frozen optimized scalar span compositor from 75ff9e3 for SIMD comparisons.
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba};

use paintcraft_document::Document;
use paintcraft_raster::RowSpan;

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
            compose_row(layer, y, area.x0, row)?;
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
                let color = color.with_opacity(layer.opacity);
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
                for (dst, src) in dst
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(src.as_chunks::<4>().0)
                {
                    let color = Rgba(*src).with_opacity(layer.opacity);
                    match color.a() {
                        0 => {}
                        255 => *dst = color.0,
                        _ => *dst = color.source_over(Rgba(*dst)).0,
                    }
                }
            }
        }
    }
    Ok(())
}
