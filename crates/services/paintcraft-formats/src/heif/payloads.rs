//! Resolve finite iloc extents only inside admitted mdat/idat storage. Never
//! follow data references, URI items, or construction-method-2 item offsets.
use super::{metadata::Reader, *};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    pub rows: u16,
    pub columns: u16,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payload<'a> {
    pub parts: Vec<&'a [u8]>,
    pub byte_len: usize,
    pub grid: Option<Grid>,
}
fn uint(r: &mut Reader<'_>, n: usize) -> Result<u64> {
    if !matches!(n, 0 | 4 | 8) {
        return Err(unsupported("HEIF iloc integer width"));
    }
    Ok(r.take(n)?
        .iter()
        .fold(0, |value, b| (value << 8) | u64::from(*b)))
}
fn extent<'a>(
    bytes: &'a [u8],
    tree: &BoxTree<'a>,
    idat: Option<usize>,
    method: u16,
    offset: u64,
    length: u64,
) -> Result<(paintcraft_paint::Extent, &'a [u8])> {
    let scope = if method == 1 {
        let id = idat.ok_or_else(|| invalid("HEIF idat-relative extent has no idat"))?;
        let b = &tree.boxes[id];
        (b.range.start + b.header_bytes) as u64..b.range.end as u64
    } else {
        tree.boxes
            .iter()
            .filter(|b| b.parent.is_none() && b.kind == *b"mdat")
            .map(|b| (b.range.start + b.header_bytes) as u64..b.range.end as u64)
            .find(|scope| {
                offset >= scope.start
                    && offset
                        .checked_add(length)
                        .is_some_and(|end| end <= scope.end)
            })
            .ok_or_else(|| invalid("HEIF file extent is outside encoded mdat payload"))?
    };
    let absolute = if method == 1 {
        scope
            .start
            .checked_add(offset)
            .ok_or_else(|| invalid("HEIF idat offset overflow"))?
    } else {
        offset
    };
    let extent = paintcraft_paint::Extent {
        offset: absolute,
        length,
    };
    let slice = paintcraft_paint::resolve_extent(bytes, scope, extent)?;
    Ok((extent, slice))
}
pub(super) fn resolve<'a>(
    bytes: &'a [u8],
    tree: &BoxTree<'a>,
    meta: usize,
    iloc: usize,
    value: &mut Inspection<'a>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let (v, flags, body) = full(tree.boxes[iloc].payload)?;
    if v > 2 || flags != 0 {
        return Err(unsupported("HEIF iloc version/flags"));
    }
    let idat = unique_child(tree, meta, *b"idat")?;
    let mut r = Reader::new(body);
    let sizes = r.u8()?;
    let offset_size = usize::from(sizes >> 4);
    let length_size = usize::from(sizes & 15);
    let sizes = r.u8()?;
    let base_size = usize::from(sizes >> 4);
    let index_size = usize::from(sizes & 15);
    if length_size == 0
        || !matches!(length_size, 4 | 8)
        || !matches!(offset_size, 0 | 4 | 8)
        || !matches!(base_size, 0 | 4 | 8)
        || !matches!(index_size, 0 | 4 | 8)
        || (v == 0 && index_size != 0)
    {
        return Err(unsupported("HEIF iloc widths or implicit lengths"));
    }
    let count = r.id(v == 2)? as usize;
    if count > ITEMS_MAX {
        return Err(resource("HEIF iloc items exceed allowance"));
    }
    let mut total = 0usize;
    let mut extents = Vec::new();
    for _ in 0..count {
        check()?;
        let id = r.id(v == 2)?;
        let method = if v > 0 { r.u16()? } else { 0 };
        if method > 1 {
            return Err(unsupported("HEIF iloc construction method"));
        }
        if r.u16()? != 0 {
            return Err(unsupported("external HEIF item data is prohibited"));
        }
        let base = uint(&mut r, base_size)?;
        let n = usize::from(r.u16()?);
        if n == 0 {
            return Err(unsupported("HEIF zero-extent item payload"));
        }
        if extents
            .len()
            .checked_add(n)
            .is_none_or(|total| total > LINKS_MAX)
        {
            return Err(resource("HEIF extent count exceeds allowance"));
        }
        let item = value
            .items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| invalid("HEIF iloc names an absent item"))?;
        if item.payload.is_some() {
            return Err(invalid("duplicate HEIF iloc item"));
        }
        let mut parts = Vec::new();
        let mut byte_len = 0usize;
        for _ in 0..n {
            check()?;
            if v > 0 && uint(&mut r, index_size)? != 0 {
                return Err(unsupported("HEIF indexed item extents are not admitted"));
            }
            let offset = base
                .checked_add(uint(&mut r, offset_size)?)
                .ok_or_else(|| invalid("HEIF extent offset overflow"))?;
            let length = uint(&mut r, length_size)?;
            if length == 0 {
                return Err(unsupported("HEIF implicit/empty item extent"));
            }
            let length = usize::try_from(length)
                .map_err(|_| resource("HEIF extent exceeds target address space"))?;
            total = total
                .checked_add(length)
                .ok_or_else(|| resource("HEIF item payload aggregate overflow"))?;
            if total > INPUT_BYTES_MAX {
                return Err(resource(
                    "HEIF resolved payload aggregate exceeds allowance",
                ));
            }
            let (extent, slice) = extent(bytes, tree, idat, method, offset, length as u64)?;
            push(&mut extents, extent, LINKS_MAX)?;
            push(&mut parts, slice, LINKS_MAX)?;
            byte_len = byte_len
                .checked_add(length)
                .ok_or_else(|| resource("HEIF item extent total overflow"))?;
        }
        item.payload = Some(Payload {
            parts,
            byte_len,
            grid: None,
        });
    }
    r.end()?;
    paintcraft_paint::validate_extents(
        &extents,
        0..bytes.len() as u64,
        paintcraft_paint::AliasPolicy::AllowExact,
        &paintcraft_paint::Limits {
            extents: LINKS_MAX,
            metadata_bytes: METADATA_BYTES_MAX,
            ..Default::default()
        },
        check,
    )?;
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
