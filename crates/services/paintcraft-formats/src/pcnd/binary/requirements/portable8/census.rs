use super::*;
use wire::Cursor;
pub(super) struct ScanOutput {
    pub facts: PortableSource8Facts,
    pub catalog: Catalog,
    pub header_start: usize,
}
struct Scratch {
    ids: [[u8; 16]; 256],
}
pub(super) fn control_bytes() -> u64 {
    // Explicit simultaneous fixed manifest, UUID array, cursors/results/locals.
    (size_of::<Catalog>()
        + size_of::<ScanOutput>()
        + size_of::<Scratch>()
        + 3 * size_of::<Cursor<'static, 'static>>()
        + 2 * size_of::<PortableLayerHeader<'static>>()
        + 2 * size_of::<PortableMaskView<'static>>()
        + size_of::<PortableHeaderView<'static>>()
        + size_of::<PortableFilterView<'static>>()
        + size_of::<PortableSource8Facts>()) as u64
}
pub(super) fn prefix<'w>(c: &mut Cursor<'w, '_>) -> Result<PortableHeaderView<'w>> {
    let document_id = uuid::Uuid::from_bytes(c.id()?);
    let state_id = uuid::Uuid::from_bytes(c.id()?);
    let canvas = (c.dimension()?, c.dimension()?);
    let title = if c.flag()? {
        Some(c.text(STRING_BYTES_MAX, 4096)?)
    } else {
        None
    };
    let source_metadata = if c.flag()? {
        let start = c.input.rest();
        let header = c.take(paintcraft_document::source_metadata::WIRE_HEADER_BYTES)?;
        let n = u64::from_be_bytes(header[112..120].try_into().unwrap());
        if n.checked_add(192)
            .is_none_or(|n| n > paintcraft_document::source_metadata::BYTES_MAX)
        {
            return Err(bound());
        }
        c.take(n as usize)?;
        Some(&start[..start.len() - c.input.remaining()])
    } else {
        None
    };
    let layers = c.number(LAYER_COUNT_MAX as u64)? as usize;
    Ok(PortableHeaderView {
        document_id,
        state_id,
        canvas,
        title,
        source_metadata,
        layers,
    })
}
pub(super) fn layer_header<'w>(c: &mut Cursor<'w, '_>) -> Result<PortableLayerHeader<'w>> {
    let id = uuid::Uuid::from_bytes(c.id()?);
    let name = c.text(STRING_BYTES_MAX, 1024)?;
    let visible = c.flag()?;
    let opacity = c.byte()?;
    let blend_mode = match c.byte()? {
        0 => LayerBlendMode::Normal,
        1 => LayerBlendMode::Multiply,
        2 => LayerBlendMode::Screen,
        _ => return Err(invalid()),
    };
    let origin = (c.signed()?, c.signed()?);
    let width = c.dimension()?;
    let height = c.dimension()?;
    let default = Rgba(c.take(4)?.try_into().unwrap());
    let filters = c.number(256)? as usize;
    Ok(PortableLayerHeader {
        id,
        name,
        visible,
        opacity,
        blend_mode,
        origin,
        width,
        height,
        default,
        filters,
    })
}
fn mask_totals(f: &mut PortableSource8Facts, m: Option<PortableMaskView<'_>>) -> Result<()> {
    if let Some(m) = m {
        f.bands = add(f.bands, m.bands)?;
        f.runs = add(f.runs, m.runs)?;
    }
    Ok(())
}
pub(super) fn block<'w>(
    c: &mut Cursor<'w, '_>,
    code: u8,
    width: u32,
    height: u32,
    body_len: usize,
) -> Result<BlockRecord> {
    if code == 3 {
        return Err(unsupported("portable Source8 excludes native blocks"));
    }
    if !matches!(code, 1 | 2 | 4 | 5) {
        return Err(invalid());
    }
    let x = c.number(u64::from(width.div_ceil(TILE_EDGE) - 1))? as u32;
    let y = c.number(u64::from(height.div_ceil(TILE_EDGE) - 1))? as u32;
    let d = (width - x * TILE_EDGE).min(TILE_EDGE) * (height - y * TILE_EDGE).min(TILE_EDGE) * 4;
    let len = c.number(BLOCK_BYTES_MAX)? as usize;
    if len == 0
        || (code == 1 && len != d as usize)
        || (code == 2 && (!len.is_multiple_of(6) || len > 6 * (d as usize / 4)))
    {
        return Err(invalid());
    }
    let offset = body_len
        .checked_sub(c.input.remaining())
        .ok_or_else(invalid)?;
    let stored = c.take(len)?;
    if code == 5 {
        let r = stored
            .get(..4)
            .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
            .ok_or_else(invalid)?;
        if len <= 4 || r == 0 || !r.is_multiple_of(6) || r > 6 * (d / 4) {
            return Err(invalid());
        }
    }
    Ok(BlockRecord {
        x,
        y,
        encoding: code,
        reserved: [0; 3],
        decoded_bytes: d,
        encoded_start: u32::try_from(offset).map_err(|_| bound())?,
        encoded_len: len as u32,
    })
}
pub(super) fn scan<'w>(
    checked: &CheckedContainer<'w>,
    limits: &PortableSource8Limits,
    meter: &Meter<'_>,
    mut records: Option<(&mut Vec<LayerRecord>, &mut Vec<BlockRecord>)>,
) -> Result<ScanOutput> {
    let body = checked.body();
    let mut c = Cursor::new(body, meter);
    let mut catalog = c.catalog()?;
    let header_start = body.len() - c.input.remaining();
    let header = prefix(&mut c)?;
    if header.layers > limits.layers
        || u64::from(header.canvas.0) * u64::from(header.canvas.1) > limits.canvas_pixels
    {
        return Err(bound());
    }
    let mut facts = PortableSource8Facts {
        layers: header.layers as u64,
        string_bytes: header.title.map_or(0, |s| s.len() as u64),
        source_metadata_bytes: header.source_metadata.map_or(0, |s| s.len() as u64),
        ..Default::default()
    };
    let mut masks = limits.coverage;
    let mut payload = 0u64;
    let mut nonnormal = false;
    meter.step(size_of::<Scratch>() as u64)?;
    let mut scratch = Scratch {
        ids: [[0; 16]; 256],
    };
    for _ in 0..header.layers {
        let start = body.len() - c.input.remaining();
        let h = layer_header(&mut c)?;
        if let Some((layers, _)) = &mut records {
            for old in layers.iter() {
                meter.step(18)?;
                if old.id == *h.id.as_bytes() {
                    return Err(invalid());
                }
            }
        }
        facts.string_bytes = add(facts.string_bytes, h.name.len() as u64)?;
        facts.filters = add(facts.filters, h.filters as u64)?;
        nonnormal |= h.blend_mode != LayerBlendMode::Normal;
        for i in 0..h.filters {
            let id = c.id()?;
            for old in &scratch.ids[..i] {
                meter.step(18)?;
                if *old == id {
                    return Err(invalid());
                }
            }
            meter.step(16)?;
            scratch.ids[i] = id;
            c.flag()?;
            c.descriptor(&mut catalog)?;
            mask_totals(&mut facts, c.mask(&mut masks)?)?;
        }
        mask_totals(&mut facts, c.mask(&mut masks)?)?;
        let end = body.len() - c.input.remaining();
        let first_block = facts.tiles;
        loop {
            let code = c.byte()?;
            if code == 0 {
                break;
            }
            let b = block(&mut c, code, h.width, h.height, body.len())?;
            facts.tiles = add(facts.tiles, 1)?;
            if facts.tiles > limits.tiles {
                return Err(bound());
            }
            payload = add(payload, u64::from(b.encoded_len))?;
            let s = match b.encoding {
                1 => 0,
                2 | 4 => u64::from(b.decoded_bytes),
                5 => {
                    let o = b.encoded_start as usize;
                    u64::from(b.decoded_bytes)
                        + u64::from(u32::from_le_bytes(body[o..o + 4].try_into().unwrap()))
                }
                _ => unreachable!(),
            };
            facts.maximum_decoder_bytes = facts.maximum_decoder_bytes.max(s);
            if let Some((_, blocks)) = &mut records {
                for old in &blocks[first_block as usize..] {
                    meter.step(10)?;
                    if old.x == b.x && old.y == b.y {
                        return Err(invalid());
                    }
                }
                if blocks.len() == blocks.capacity() {
                    return Err(bound());
                }
                meter.step(size_of::<BlockRecord>() as u64)?;
                blocks.push(b);
            }
        }
        if let Some((layers, _)) = &mut records {
            if layers.len() == layers.capacity() {
                return Err(bound());
            }
            meter.step(size_of::<LayerRecord>() as u64)?;
            layers.push(LayerRecord {
                id: *h.id.as_bytes(),
                header_start: start as u64,
                header_end: end as u64,
                blocks_start: u32::try_from(first_block).map_err(|_| bound())?,
                blocks_len: u32::try_from(facts.tiles - first_block).map_err(|_| bound())?,
                width: h.width,
                height: h.height,
                name_bytes: h.name.len() as u32,
                filter_count: h.filters as u16,
                reserved: 0,
            });
        }
        let metadata = (body.len() + MAGIC.len() - c.input.remaining()) as u64;
        if metadata
            .checked_sub(payload)
            .is_none_or(|n| n > limits.metadata_bytes.min(DOCUMENT_MAX as usize) as u64)
        {
            return Err(bound());
        }
    }
    if c.input.remaining() != 0 || !nonnormal {
        return Err(invalid());
    }
    for used in &catalog.used[..catalog.len] {
        meter.step(1)?;
        if !used {
            return Err(invalid());
        }
    }
    facts.metadata_bytes = (body.len() + MAGIC.len()) as u64 - payload;
    if facts.metadata_bytes > limits.metadata_bytes.min(DOCUMENT_MAX as usize) as u64 {
        return Err(bound());
    }
    Ok(ScanOutput {
        facts,
        catalog,
        header_start,
    })
}
