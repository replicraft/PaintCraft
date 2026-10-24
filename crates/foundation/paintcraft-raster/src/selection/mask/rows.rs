//! Allocation-free selected row traversal, including sparse full/inverted fields.
use super::*;
pub(super) fn visit(
    mask: &BinaryMask,
    y: i64,
    x0: i64,
    x1: i64,
    allowance: u64,
    check: &dyn Fn() -> Result<()>,
    mut emit: impl FnMut(i64, i64) -> Result<()>,
) -> Result<u64> {
    check()?;
    if y < mask.domain.y0 || y >= mask.domain.y1 {
        return Ok(0);
    }
    let (mut x, end) = (x0.max(mask.domain.x0), x1.min(mask.domain.x1));
    if x >= end {
        return Ok(0);
    }
    let mut work = allowance;
    if mask.storage.is_none() {
        spend(&mut work, 1)?;
        if mask.default_coverage() == 255 {
            emit(x, end)?;
        }
        return Ok(allowance - work);
    }
    let mut start = None;
    let (ty, iy) = tile_split(y - mask.offset.1);
    while x < end {
        check()?;
        spend(&mut work, 1)?;
        let (tx, ix) = tile_split(x - mask.offset.0);
        let next = end.min((tx + 1) * i64::from(crate::TILE_EDGE) + mask.offset.0);
        let tile = mask
            .storage
            .as_ref()
            .and_then(|s| s.tiles.get(&TileCoord { tx, ty }));
        let solid = match tile {
            None => Some(mask.default),
            Some(Tile::Solid(v)) => Some(*v),
            _ => None,
        };
        if let Some(value) = solid {
            segment(&mut start, x, mask.logical(value) == 255, &mut emit)?;
        } else if let Some(Tile::Dense(tile)) = tile {
            let mut index = iy as usize * crate::TILE_EDGE as usize + ix as usize;
            let mut at = x;
            while at < next {
                if (at - x) % 64 == 0 {
                    check()?;
                }
                spend(&mut work, 1)?;
                segment(
                    &mut start,
                    at,
                    mask.logical(tile.values[index]) == 255,
                    &mut emit,
                )?;
                index += 1;
                at += 1;
            }
        }
        x = next;
    }
    check()?;
    if let Some(start) = start {
        emit(start, end)?;
    }
    Ok(allowance - work)
}
fn segment(
    start: &mut Option<i64>,
    x0: i64,
    selected: bool,
    emit: &mut impl FnMut(i64, i64) -> Result<()>,
) -> Result<()> {
    if selected {
        start.get_or_insert(x0);
    } else if let Some(start) = start.take() {
        emit(start, x0)?;
    }
    Ok(())
}
