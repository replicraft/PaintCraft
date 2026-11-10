//! Own fresh zeros here: no cache/prefix/caller-populated buffer can enter.
use super::{Seed, limit, missing};
use paintcraft_core::types::{Rect, Result, Rgba};
use paintcraft_document::Layer;
use paintcraft_raster::RowSpan;
pub(super) fn pixel(source: Rgba, opacity: u8) -> [u8; 4] {
    let source = source.with_opacity(opacity);
    if source.a() == 0 { [0; 4] } else { source.0 }
}
fn dense(out: &mut [u8], source: &[u8], opacity: u8) {
    for (d, s) in out
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(source.as_chunks::<4>().0)
    {
        *d = pixel(Rgba(*s), opacity);
    }
}
fn row(
    layer: &Layer,
    y: i64,
    x: i64,
    mut out: &mut [u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let local_y = y - layer.origin.1;
    let mut local_x = x - layer.origin.0;
    for span in layer
        .raster
        .row_spans(local_y as u32, local_x as u32, (out.len() / 4) as u32)?
    {
        let n = span.pixel_count();
        let (dst, rest) = out.split_at_mut(n * 4);
        out = rest;
        if !layer.filters.iter().any(|f| f.enabled()) && !crate::layer_mask::active(layer) {
            match span {
                RowSpan::Pixels(source) => {
                    for (out, input) in dst.chunks_mut(256).zip(source.chunks(256)) {
                        check()?;
                        dense(out, input, layer.opacity);
                    }
                }
                RowSpan::Uniform { color, .. } => {
                    let color = pixel(color, layer.opacity);
                    for out in dst.chunks_mut(256) {
                        check()?;
                        out.as_chunks_mut::<4>().0.fill(color);
                    }
                }
            }
            local_x += n as i64;
            continue;
        }
        // Same source/filter/own-mask preparation as original nonNormal rows.
        // Seed does not introduce Normal's additional uniform-proof attempt.
        let mut scratch = [[0u8; 4]; 64];
        for (index, out) in dst.chunks_mut(256).enumerate() {
            check()?;
            let input = scratch[..out.len() / 4].as_flattened_mut();
            match &span {
                RowSpan::Uniform { color, .. } => input.as_chunks_mut::<4>().0.fill(color.0),
                RowSpan::Pixels(bytes) => {
                    input.copy_from_slice(&bytes[index * 256..index * 256 + out.len()])
                }
            }
            crate::appearance::pixels(
                layer,
                (local_x + (index * 64) as i64, local_y),
                input,
                check,
            )?;
            dense(out, input, layer.opacity);
        }
        local_x += n as i64;
    }
    Ok(())
}
fn spatial(
    layer: &Layer,
    area: Rect,
    region: Rect,
    out: &mut [u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let local = area
        .translated(-layer.origin.0, -layer.origin.1)
        .ok_or_else(limit)?;
    let width = region.width() as usize;
    crate::spatial::visit(layer, local, check, |tile, pixels| {
        for (i, source) in pixels.iter().enumerate() {
            if i.is_multiple_of(64) {
                check()?;
            }
            let x = tile.x0 + layer.origin.0 + (i % tile.width() as usize) as i64;
            let y = tile.y0 + layer.origin.1 + (i / tile.width() as usize) as i64;
            let index = ((y - region.y0) as usize * width + (x - region.x0) as usize) * 4;
            out[index..index + 4].copy_from_slice(&pixel(*source, layer.opacity));
        }
        Ok(())
    })
}
pub(crate) fn fresh_output(
    seed: &Seed<'_>,
    bytes: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let doc = seed.document();
    let region = seed.region();
    if crate::region_bytes(doc, region)? != bytes {
        return Err(limit());
    }
    let mut out = crate::output_buffer(bytes)?;
    if seed.index == usize::MAX {
        crate::compose_roots_into(doc, region, 0..doc.roots.len(), &mut out, &check)?;
    } else {
        let layer = doc.layers.get(&doc.roots[seed.index]).ok_or_else(missing)?;
        let area = region.intersect(&layer.extent()).ok_or_else(missing)?;
        if crate::spatial::active(layer) {
            spatial(layer, area, region, &mut out, check)?;
        } else {
            let width = region.width() as usize;
            for y in area.y0..area.y1 {
                check()?;
                let start = ((y - region.y0) as usize * width + (area.x0 - region.x0) as usize) * 4;
                row(
                    layer,
                    y,
                    area.x0,
                    &mut out[start..start + area.width() as usize * 4],
                    check,
                )?;
            }
        }
        // Earlier roots provably write nothing in this ROI. Above-root order
        // and every intermediate RGBA8 boundary remain unchanged.
        crate::compose_roots_into(
            doc,
            region,
            seed.index + 1..doc.roots.len(),
            &mut out,
            &check,
        )?;
    }
    Ok(out)
}
