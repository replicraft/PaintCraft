//! HEVC payload presence and derived-grid policy after shared finite storage.
use super::{metadata::Reader, *};
pub(super) fn resolve<'a>(
    bytes: &'a [u8],
    tree: &BoxTree<'a>,
    meta: usize,
    iloc: usize,
    value: &mut Inspection<'a>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    crate::item_graph::payloads::resolve(bytes, tree, meta, iloc, value, check)?;
    for item in &value.items {
        check()?;
        if matches!(&item.kind, b"hvc1" | b"grid") && item.payload.is_none() {
            return Err(invalid("HEIF image item has no admitted encoded payload"));
        }
    }
    // Decode only the tiny derived-grid descriptor, not any HEVC pixels.
    let mut grids = Vec::new();
    for item in &value.items {
        if item.kind != *b"grid" {
            continue;
        }
        check()?;
        let data = item
            .payload
            .as_ref()
            .ok_or_else(|| invalid("HEIF grid lacks payload"))?;
        if !matches!(data.byte_len, 8 | 12) {
            return Err(invalid("HEIF grid descriptor length"));
        }
        let mut descriptor = [0u8; 12];
        let mut offset = 0usize;
        for part in &data.parts {
            descriptor[offset..offset + part.len()].copy_from_slice(part);
            offset += part.len();
        }
        let mut r = Reader::new(&descriptor[..data.byte_len]);
        if r.u8()? != 0 {
            return Err(unsupported("HEIF grid descriptor version"));
        }
        let flags = r.u8()?;
        if flags > 1 {
            return Err(unsupported("HEIF grid descriptor flags"));
        }
        let rows = u16::from(r.u8()?) + 1;
        let columns = u16::from(r.u8()?) + 1;
        let width = r.id(flags == 1)?;
        let height = r.id(flags == 1)?;
        r.end()?;
        let grid = Grid {
            rows,
            columns,
            width,
            height,
        };
        validate_grid(value, item, &grid)?;
        push(&mut grids, (item.id, grid), ITEMS_MAX)?;
    }
    for (id, grid) in grids {
        value
            .items
            .iter_mut()
            .find(|item| item.id == id)
            .and_then(|item| item.payload.as_mut())
            .ok_or_else(|| invalid("HEIF grid payload disappeared"))?
            .grid = Some(grid);
    }
    Ok(())
}
fn size(item: &Item<'_>) -> Result<(u32, u32)> {
    item.properties
        .iter()
        .find_map(|p| {
            if let Property::SpatialExtent { width, height } = p.property {
                Some((width, height))
            } else {
                None
            }
        })
        .ok_or_else(|| invalid("HEIF grid image lacks extent"))
}
fn validate_grid(value: &Inspection<'_>, item: &Item<'_>, grid: &Grid) -> Result<()> {
    if size(item)? != (grid.width, grid.height) {
        return Err(invalid(
            "HEIF grid descriptor disagrees with spatial extent",
        ));
    }
    let tiles = value
        .references(*b"dimg", item.id)
        .ok_or_else(|| invalid("HEIF grid lacks tile order"))?;
    if tiles.len() != usize::from(grid.rows) * usize::from(grid.columns) {
        return Err(invalid("HEIF grid tile count disagrees with rows/columns"));
    }
    let first = value
        .item(tiles[0])
        .ok_or_else(|| invalid("HEIF grid tile is absent"))?;
    let tile_size = size(first)?;
    for id in tiles {
        if size(
            value
                .item(*id)
                .ok_or_else(|| invalid("HEIF grid tile is absent"))?,
        )? != tile_size
        {
            return Err(unsupported(
                "HEIF grid tiles have inconsistent declared geometry",
            ));
        }
    }
    let columns = u64::from(grid.columns);
    let rows = u64::from(grid.rows);
    let w = u64::from(tile_size.0);
    let h = u64::from(tile_size.1);
    if u64::from(grid.width) > columns * w
        || u64::from(grid.height) > rows * h
        || u64::from(grid.width) <= (columns - 1) * w
        || u64::from(grid.height) <= (rows - 1) * h
    {
        return Err(invalid(
            "HEIF grid dimensions leave a gap or unused whole tile",
        ));
    }
    Ok(())
}
#[cfg(test)]
pub(super) mod tests;
