use super::*;
pub(super) fn validate(
    bytes: &mut Bytes<'_>,
    header: &Header<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<u32> {
    let lines = if header.compression == 3 { 16 } else { 1 };
    let count = header.height.div_ceil(lines);
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(count as usize)
        .map_err(|_| resource("EXR offset inventory allocation"))?;
    for _ in 0..count {
        check()?;
        let offset = u64::from_le_bytes(bytes.take(8)?.try_into().unwrap());
        offsets.push(
            usize::try_from(offset)
                .map_err(|_| resource("EXR chunk offset exceeds address space"))?,
        );
    }
    let body = bytes.at;
    let mut intervals = Vec::new();
    intervals
        .try_reserve_exact(count as usize)
        .map_err(|_| resource("EXR chunk interval allocation"))?;
    let pixel_bytes: u64 = header
        .channels
        .iter()
        .map(|c| u64::from(c.sample_type.bytes()))
        .sum();
    for (index, offset) in offsets.into_iter().enumerate() {
        check()?;
        if offset < body {
            return Err(invalid("EXR chunk points into header/table"));
        }
        let mut chunk = Bytes {
            bytes: bytes.bytes,
            at: offset,
        };
        let y = chunk.i32()?;
        let expected = i64::from(header.origin[1]) + index as i64 * i64::from(lines);
        if i64::from(y) != expected {
            return Err(invalid("EXR chunk row disagrees with offset table"));
        }
        let size = chunk.i32()?;
        if size <= 0 {
            return Err(invalid("EXR chunk byte count must be positive"));
        }
        let rows = lines.min(header.height - index as u32 * lines);
        let raw = u64::from(rows) * u64::from(header.width) * pixel_bytes;
        if size as u64 > raw || (header.compression == 0 && size as u64 != raw) {
            return Err(invalid(
                "EXR chunk size disagrees with admitted channel geometry",
            ));
        }
        chunk.take(size as usize)?;
        intervals.push((offset, chunk.at));
    }
    intervals.sort_unstable();
    let mut end = body;
    for (start, next) in intervals {
        check()?;
        if start != end {
            return Err(invalid("EXR chunks overlap or leave unqualified padding"));
        }
        end = next;
    }
    if end != bytes.bytes.len() {
        return Err(unsupported("EXR trailing data unqualified"));
    }
    Ok(count)
}
