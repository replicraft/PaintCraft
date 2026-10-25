use super::*;

/// Check stream shape without materializing RGBA. In particular, the selected
/// backend's unbounded ZIP inflater is called only after exact bounded inflation
/// and end-of-stream checks have succeeded on these immutable bytes.
pub(super) fn admit(
    data: &[u8],
    width: u32,
    rows: u64,
    large: bool,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let mut c = Cursor::new(data);
    let expected = u64::from(width)
        .checked_mul(rows)
        .ok_or_else(|| resource("PSD channel extent overflow"))?;
    match c.u16()? {
        0 => {
            if c.remaining() as u64 != expected {
                return Err(malformed("PSD raw channel length contradicts dimensions"));
            }
        }
        1 => rle(&mut c, width, rows, large, check)?,
        2 | 3 => zip(&data[2..], expected, check)?,
        _ => return Err(unsupported("PSD channel compression is not supported")),
    }
    check()
}
fn rle(
    c: &mut Cursor<'_>,
    width: u32,
    rows: u64,
    large: bool,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let table_bytes = rows
        .checked_mul(if large { 4 } else { 2 })
        .ok_or_else(|| resource("PSD RLE table overflow"))?;
    let table_size = usize::try_from(table_bytes)
        .map_err(|_| resource("PSD RLE table exceeds address space"))?;
    let mut table = Cursor::new(c.take(table_size)?);
    for _ in 0..rows {
        check()?;
        let size = if large {
            table.u32()? as usize
        } else {
            usize::from(table.u16()?)
        };
        let mut row = Cursor::new(c.take(size)?);
        let mut produced = 0u64;
        while row.remaining() > 0 {
            let before = row.pos;
            let command = row.u8()?;
            let count = if command < 128 {
                let count = usize::from(command) + 1;
                row.take(count)?;
                count
            } else if command > 128 {
                row.take(1)?;
                257 - usize::from(command)
            } else {
                0
            };
            produced += count as u64;
            if produced > u64::from(width) {
                return Err(malformed("PSD RLE packet crosses a scanline"));
            }
            if row.pos / 65536 != before / 65536 {
                check()?;
            }
        }
        if produced != u64::from(width) {
            return Err(malformed("PSD RLE row does not fill its scanline"));
        }
    }
    if c.remaining() != 0 {
        return Err(malformed("PSD RLE channel has undeclared trailing data"));
    }
    Ok(())
}
fn zip(data: &[u8], expected: u64, check: &dyn Fn() -> Result<()>) -> Result<()> {
    use flate2::{Decompress, FlushDecompress, Status};
    let mut decoder = Decompress::new(true);
    let mut buffer = [0u8; 65536];
    loop {
        check()?;
        let input = usize::try_from(decoder.total_in())
            .map_err(|_| resource("PSD ZIP input offset overflow"))?;
        let before = (decoder.total_in(), decoder.total_out());
        // A fixed scratch buffer bounds even a declared-size-one ZIP bomb.
        let status = decoder
            .decompress(&data[input..], &mut buffer, FlushDecompress::None)
            .map_err(|_| malformed("invalid PSD ZIP channel"))?;
        if decoder.total_out() > expected {
            return Err(malformed(
                "PSD ZIP channel expands beyond its declared dimensions",
            ));
        }
        if status == Status::StreamEnd {
            if decoder.total_in() as usize != data.len() || decoder.total_out() != expected {
                return Err(malformed("PSD ZIP channel length or end marker mismatch"));
            }
            return Ok(());
        }
        if before == (decoder.total_in(), decoder.total_out()) {
            return Err(malformed("truncated PSD ZIP channel"));
        }
    }
}
