use super::*;
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
pub(crate) fn resolve<'a, P>(
    bytes: &'a [u8],
    tree: &BoxTree<'a>,
    meta: usize,
    iloc: usize,
    value: &mut Inspection<'a, P>,
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
    Ok(())
}
