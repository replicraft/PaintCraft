use super::*;
use flate2::{Decompress, FlushDecompress, Status};
use graph::Item;

pub(super) struct Plan<'a> {
    pub id: u32,
    pub name: &'a str,
    pub width: u32,
    pub height: u32,
    pub origin: (i32, i32),
    pub compressed: &'a [u8],
    pub order: [usize; 4],
}
fn ispe(graph: &Graph<'_>, item: &Item<'_>) -> Result<(u32, u32)> {
    let mut r = Reader::new(graph.property(item, *b"ispe")?.payload);
    r.full(0)?;
    let result = (r.u32()?, r.u32()?);
    r.end()?;
    Ok(result)
}
fn pixi(graph: &Graph<'_>, item: &Item<'_>) -> Result<()> {
    let mut r = Reader::new(graph.property(item, *b"pixi")?.payload);
    r.full(0)?;
    if r.take(5)? != [4, 8, 8, 8, 8] {
        return Err(unsupported("PAINT requires four unsigned 8-bit channels"));
    }
    r.end()
}
pub(super) fn validate_overlay(
    graph: &Graph<'_>,
    item: &Item<'_>,
    dimensions: (u32, u32),
) -> Result<()> {
    if item.associations.len() != 2 {
        return Err(unsupported("Unsupported PAINT overlay properties"));
    }
    if ispe(graph, item)? != dimensions {
        return Err(malformed("PAINT overlay and ispe dimensions disagree"));
    }
    pixi(graph, item)
}
pub(super) fn plan<'a>(
    graph: &Graph<'a>,
    item: &Item<'a>,
    origin: (i32, i32),
    limits: &DecodeLimits,
) -> Result<Plan<'a>> {
    if item.kind != *b"unci" || item.associations.len() != 6 {
        return Err(unsupported("Unsupported PAINT layer item or property set"));
    }
    let (width, height) = ispe(graph, item)?;
    dimensions(width, height, limits)?;
    pixi(graph, item)?;
    let mut r = Reader::new(graph.property(item, *b"cmpC")?.payload);
    r.full(0)?;
    if r.take(5)? != b"defl\0" {
        return Err(unsupported(
            "Unsupported PAINT layer compression configuration",
        ));
    }
    r.end()?;
    let mut r = Reader::new(graph.property(item, *b"colr")?.payload);
    if r.take(11)? != b"nclx\0\x01\0\x0d\0\0\x80" {
        return Err(unsupported(
            "PAINT layer color space must be full-range sRGB",
        ));
    }
    r.end()?;
    let mut r = Reader::new(graph.property(item, *b"cmpd")?.payload);
    if r.u32()? != 4 {
        return Err(unsupported("Unsupported PAINT component count"));
    }
    let components = [r.u16()?, r.u16()?, r.u16()?, r.u16()?];
    r.end()?;
    let mut r = Reader::new(graph.property(item, *b"uncC")?.payload);
    r.full(0)?;
    if r.take(4)? != b"gene" || r.u32()? != 4 {
        return Err(unsupported("Unsupported PAINT uncompressed pixel profile"));
    }
    let mut order = [usize::MAX; 4];
    for position in 0..4 {
        let index = usize::from(r.u16()?);
        if r.take(3)? != [7, 0, 0] {
            return Err(unsupported(
                "PAINT component depth, type or alignment is unsupported",
            ));
        }
        let channel = *components
            .get(index)
            .ok_or_else(|| malformed("PAINT component index is outside cmpd"))?;
        let slot = channel
            .checked_sub(4)
            .filter(|i| *i < 4)
            .ok_or_else(|| unsupported("PAINT components must be red, green, blue and alpha"))?
            as usize;
        if order[slot] != usize::MAX {
            return Err(malformed("Duplicate PAINT color component"));
        }
        order[slot] = position;
    }
    if order.contains(&usize::MAX) {
        return Err(malformed("Missing PAINT color component"));
    }
    if r.take(4)? != [0, 1, 0, 0] || r.take(20)? != [0; 20] {
        return Err(unsupported(
            "PAINT component packing, padding or tiles are unsupported",
        ));
    }
    r.end()?;
    Ok(Plan {
        id: item.id,
        name: item.name,
        width,
        height,
        origin,
        compressed: graph.data(item)?,
        order,
    })
}
pub(super) fn inflate(
    compressed: &[u8],
    width: u32,
    height: u32,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let expected = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| resource("PAINT decoded buffer exceeds address space"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected)
        .map_err(|_| resource("Cannot allocate decoded PAINT pixels"))?;
    bytes.resize(expected, 0);
    let mut decoder = Decompress::new(false);
    let mut consumed = 0usize;
    let mut written = 0usize;
    let mut excess = [0u8; 1];
    loop {
        check()?;
        let before_in = decoder.total_in();
        let before_out = decoder.total_out();
        let input = &compressed[consumed..compressed.len().min(consumed.saturating_add(65536))];
        let output = if written < expected {
            let end = expected.min(written.saturating_add(65536));
            &mut bytes[written..end]
        } else {
            &mut excess
        };
        let status = decoder
            .decompress(input, output, FlushDecompress::None)
            .map_err(|_| malformed("Invalid PAINT raw DEFLATE stream"))?;
        let read = (decoder.total_in() - before_in) as usize;
        let produced = (decoder.total_out() - before_out) as usize;
        consumed += read;
        written = written
            .checked_add(produced)
            .ok_or_else(|| resource("PAINT output count overflow"))?;
        if written > expected {
            return Err(malformed("PAINT DEFLATE output exceeds declared pixels"));
        }
        if status == Status::StreamEnd {
            if written != expected || consumed != compressed.len() {
                return Err(malformed(
                    "PAINT DEFLATE length or trailing bytes disagree with the item",
                ));
            }
            break;
        }
        if read == 0 && produced == 0 {
            return Err(malformed("Truncated PAINT DEFLATE stream"));
        }
    }
    check()?;
    Ok(bytes)
}
pub(super) fn swizzle(
    mut bytes: Vec<u8>,
    order: [usize; 4],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    for chunk in bytes.chunks_mut(65536) {
        check()?;
        for pixel in chunk.as_chunks_mut::<4>().0 {
            let original = *pixel;
            *pixel = [
                original[order[0]],
                original[order[1]],
                original[order[2]],
                original[order[3]],
            ];
        }
    }
    check()?;
    Ok(bytes)
}
