//! New-mode rows/spatial output only; old Normal body remains in lib/kernels.
use super::*;
pub(crate) fn row(
    layer: &Layer,
    y: i64,
    x: i64,
    row: &mut [u8],
    kind: Kind,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let local_y = y - layer.origin.1;
    let mut local_x = x - layer.origin.0;
    let mut remaining = row;
    for span in
        layer
            .raster
            .row_spans(local_y as u32, local_x as u32, (remaining.len() / 4) as u32)?
    {
        let n = span.pixel_count();
        let (dst, rest) = remaining.split_at_mut(n * 4);
        remaining = rest;
        if !layer.filters.iter().any(|f| f.enabled()) && !crate::layer_mask::active(layer) {
            match span {
                RowSpan::Pixels(source) => {
                    for (out, input) in dst.chunks_mut(256).zip(source.chunks(256)) {
                        check()?;
                        dense(kind, out, input, layer.opacity)?;
                    }
                }
                RowSpan::Uniform { color, .. } => {
                    let source = color.with_opacity(layer.opacity);
                    for out in dst.chunks_mut(256) {
                        check()?;
                        reserve_pixels(kind, (out.len() / 4) as u64)?;
                        for pixel in out.as_chunks_mut::<4>().0 {
                            *pixel = pixel_precharged(kind, source, Rgba(*pixel))?.0;
                        }
                    }
                }
            }
            local_x += n as i64;
            continue;
        }
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
            dense(kind, out, input, layer.opacity)?;
        }
        local_x += n as i64;
    }
    Ok(())
}
pub(crate) fn spatial(
    layer: &Layer,
    area: Rect,
    region: Rect,
    out: &mut [u8],
    kind: Kind,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let local = area
        .translated(-layer.origin.0, -layer.origin.1)
        .ok_or_else(limit)?;
    reserve_pixels(kind, u64::try_from(area.area()).map_err(|_| limit())?)?;
    let width = region.width() as usize;
    crate::spatial::visit(layer, local, check, |tile, pixels| {
        for (i, source) in pixels.iter().enumerate() {
            if i.is_multiple_of(64) {
                check()?;
            }
            let x = tile.x0 + layer.origin.0 + (i % tile.width() as usize) as i64;
            let y = tile.y0 + layer.origin.1 + (i / tile.width() as usize) as i64;
            let index = ((y - region.y0) as usize * width + (x - region.x0) as usize) * 4;
            let destination = Rgba(out[index..index + 4].try_into().unwrap());
            out[index..index + 4].copy_from_slice(
                &pixel_precharged(kind, source.with_opacity(layer.opacity), destination)?.0,
            );
        }
        Ok(())
    })
}
