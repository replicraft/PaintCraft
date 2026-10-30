use super::*;
use crate::compose_row;
use std::mem::size_of;

struct TileView<'a> {
    region: Rect,
    rows: Vec<&'a mut [u8]>,
}
fn refused() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "tile partition view budget exceeded",
    )
}
pub(super) fn admit(region: Rect, edge: u32, budget: usize) -> Result<()> {
    let columns = (region.width() as usize).div_ceil(edge as usize);
    let bands = (region.height() as usize).div_ceil(edge as usize);
    let charge = columns
        .checked_mul(bands)
        .and_then(|count| count.checked_mul(size_of::<TileView<'_>>()))
        .and_then(|table| {
            columns
                .checked_mul(region.height() as usize)
                .and_then(|rows| rows.checked_mul(size_of::<&mut [u8]>()))
                .and_then(|rows| table.checked_add(rows))
        })
        .ok_or_else(refused)?;
    if charge > budget {
        return Err(refused());
    }
    Ok(())
}

pub(super) fn compose(
    doc: &Document,
    region: Rect,
    edge: u32,
    budget: usize,
    pixels: &mut [u8],
    check: &(impl Fn() -> Result<()> + Sync),
) -> Result<()> {
    let edge = edge as usize;
    let columns = (region.width() as usize).div_ceil(edge);
    let bands = (region.height() as usize).div_ceil(edge);
    let mut views = Vec::new();
    views
        .try_reserve_exact(columns * bands)
        .map_err(|_| refused())?;
    let mut charge = views.capacity() * size_of::<TileView<'_>>();
    for band in 0..bands {
        check()?;
        for col in 0..columns {
            let rect = Rect {
                x0: region.x0 + (col * edge) as i64,
                y0: region.y0 + (band * edge) as i64,
                x1: (region.x0 + ((col + 1) * edge) as i64).min(region.x1),
                y1: (region.y0 + ((band + 1) * edge) as i64).min(region.y1),
            };
            let mut rows = Vec::new();
            rows.try_reserve_exact(rect.height() as usize)
                .map_err(|_| refused())?;
            charge = charge
                .checked_add(rows.capacity() * size_of::<&mut [u8]>())
                .ok_or_else(refused)?;
            if charge > budget {
                return Err(refused());
            }
            views.push(TileView { region: rect, rows });
        }
    }
    // Each disjoint mutable row fragment is moved into exactly one tile job.
    // Jobs cannot obtain adjacent pixels through indexing; no unsafe splitting.
    let stride = region.width() as usize * 4;
    for (y, row) in pixels.chunks_exact_mut(stride).enumerate() {
        check()?;
        for (col, part) in row.chunks_mut(edge * 4).enumerate() {
            views[y / edge * columns + col].rows.push(part);
        }
    }
    views.into_par_iter().try_for_each(|mut tile| {
        check()?;
        for layer in doc
            .layers_in_order()
            .filter(|layer| layer.visible && layer.opacity != 0)
        {
            check()?;
            let Some(area) = tile.region.intersect(&layer.extent()) else {
                continue;
            };
            for y in area.y0..area.y1 {
                check()?;
                let row = &mut tile.rows[(y - tile.region.y0) as usize];
                let start = (area.x0 - tile.region.x0) as usize * 4;
                compose_row(
                    layer,
                    y,
                    area.x0,
                    &mut row[start..start + area.width() as usize * 4],
                    &check,
                )?;
            }
        }
        Ok(())
    })
}
