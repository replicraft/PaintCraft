//! Fresh deterministic boxes; fixups refer only to fields serialized here.
use super::*;
fn full(out: &mut Output, version: u8) -> Result<()> {
    out.put(&[version, 0, 0, 0])
}
fn ispe(out: &mut Output, width: u32, height: u32) -> Result<()> {
    let start = out.start(b"ispe")?;
    full(out, 0)?;
    out.u32(width)?;
    out.u32(height)?;
    out.end(start)
}
fn item(out: &mut Output, id: u16, kind: &[u8; 4], name: &str) -> Result<()> {
    let start = out.start(b"infe")?;
    full(out, 2)?;
    out.u16(id)?;
    out.u16(0)?;
    out.put(kind)?;
    out.put(name.as_bytes())?;
    out.put(&[0])?;
    out.end(start)
}
fn properties(
    out: &mut Output,
    doc: &WriteDocument<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let iprp = out.start(b"iprp")?;
    let ipco = out.start(b"ipco")?;
    let cmpc = out.start(b"cmpC")?;
    full(out, 0)?;
    out.put(b"defl\0")?;
    out.end(cmpc)?;
    ispe(out, doc.width, doc.height)?;
    let colr = out.start(b"colr")?;
    out.put(b"nclx")?;
    out.u16(1)?;
    out.u16(13)?;
    out.u16(0)?;
    out.put(&[0x80])?;
    out.end(colr)?;
    let cmpd = out.start(b"cmpd")?;
    out.u32(4)?;
    for channel in [4, 5, 6, 7] {
        out.u16(channel)?;
    }
    out.end(cmpd)?;
    let uncc = out.start(b"uncC")?;
    full(out, 0)?;
    out.put(b"gene")?;
    out.u32(4)?;
    for component in [2, 1, 0, 3] {
        out.u16(component)?;
        out.put(&[7, 0, 0])?;
    }
    out.put(&[0, 1, 0, 0])?;
    out.put(&[0; 20])?;
    out.end(uncc)?;
    let pixi = out.start(b"pixi")?;
    full(out, 0)?;
    out.put(&[4, 8, 8, 8, 8])?;
    out.end(pixi)?;
    for layer in doc.layers {
        check()?;
        ispe(out, layer.width, layer.height)?;
    }
    out.end(ipco)?;
    let ipma = out.start(b"ipma")?;
    full(out, 0)?;
    out.u32(doc.layers.len() as u32 + 1)?;
    for index in 0..doc.layers.len() {
        check()?;
        out.u16(index as u16 + 1)?;
        out.put(&[6, 0x81, 0x80 | (index as u8 + 7), 0x83, 0x84, 0x85, 0x86])?;
    }
    out.u16(doc.layers.len() as u16 + 1)?;
    out.put(&[2, 0x82, 0x86])?;
    out.end(ipma)?;
    out.end(iprp)
}
pub(super) fn metadata(
    out: &mut Output,
    doc: &WriteDocument<'_>,
    plan: &WritePlan,
    check: &dyn Fn() -> Result<()>,
) -> Result<[usize; 64]> {
    let n = doc.layers.len();
    let primary = n as u16 + 1;
    let ftyp = out.start(b"ftyp")?;
    out.put(b"mif1")?;
    out.u32(0)?;
    out.put(b"mif1gcmiisoamiaf")?;
    out.end(ftyp)?;
    let meta = out.start(b"meta")?;
    full(out, 0)?;
    let hdlr = out.start(b"hdlr")?;
    full(out, 0)?;
    out.u32(0)?;
    out.put(b"pict")?;
    out.put(&[0; 13])?;
    out.end(hdlr)?;
    let pitm = out.start(b"pitm")?;
    full(out, 0)?;
    out.u16(primary)?;
    out.end(pitm)?;
    let iinf = out.start(b"iinf")?;
    full(out, 0)?;
    out.u16(primary)?;
    for (index, layer) in doc.layers.iter().enumerate() {
        check()?;
        item(out, index as u16 + 1, b"unci", layer.item_name)?;
    }
    item(out, primary, b"iovl", "")?;
    out.end(iinf)?;
    let mut locations = [0usize; 64];
    let iloc = out.start(b"iloc")?;
    full(out, 1)?;
    out.put(&[0x44, 0])?;
    out.u16(primary)?;
    for (index, location) in locations[..n].iter_mut().enumerate() {
        check()?;
        out.u16(index as u16 + 1)?;
        out.u16(0)?;
        out.u16(0)?;
        out.u16(1)?;
        *location = out.bytes.len();
        out.put(&[0; 8])?;
    }
    out.u16(primary)?;
    out.u16(1)?;
    out.u16(0)?;
    out.u16(1)?;
    out.u32(0)?;
    let overlay_length = if plan.wide_overlay {
        18 + n * 8
    } else {
        14 + n * 4
    };
    out.u32(overlay_length as u32)?;
    out.end(iloc)?;
    properties(out, doc, check)?;
    let iref = out.start(b"iref")?;
    full(out, 0)?;
    let dimg = out.start(b"dimg")?;
    out.u16(primary)?;
    out.u16(n as u16)?;
    for index in 0..n {
        out.u16(index as u16 + 1)?;
    }
    out.end(dimg)?;
    out.end(iref)?;
    let idat = out.start(b"idat")?;
    out.put(&[0, u8::from(plan.wide_overlay)])?;
    for fill in doc.canvas_fill {
        out.u16(fill)?;
    }
    if plan.wide_overlay {
        out.u32(doc.width)?;
        out.u32(doc.height)?;
    } else {
        out.u16(doc.width as u16)?;
        out.u16(doc.height as u16)?;
    }
    for layer in doc.layers {
        check()?;
        if plan.wide_overlay {
            out.put(&layer.origin.0.to_be_bytes())?;
            out.put(&layer.origin.1.to_be_bytes())?;
        } else {
            out.put(&(layer.origin.0 as i16).to_be_bytes())?;
            out.put(&(layer.origin.1 as i16).to_be_bytes())?;
        }
    }
    out.end(idat)?;
    out.end(meta)?;
    if out.bytes.len() > plan.metadata_bytes {
        return Err(resource("PAINT constructed metadata exceeds declaration"));
    }
    Ok(locations)
}
