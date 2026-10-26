use super::*;
fn header(bytes: &[u8]) -> Result<Header> {
    if bytes.len() > BYTES_MAX {
        return Err(limit("MSP encoded byte allowance exceeded"));
    }
    let raw = bytes.get(..32).ok_or_else(|| bad("truncated MSP header"))?;
    let version = match &raw[..4] {
        b"DanM" => Version::UncompressedV1,
        b"LinS" => Version::RowRleV2,
        _ => return Err(bad("invalid MSP signature")),
    };
    let mut words = [0; 16];
    for (word, raw) in words.iter_mut().zip(raw.as_chunks::<2>().0) {
        *word = u16::from_le_bytes(*raw);
    }
    if words.iter().fold(0, |sum, word| sum ^ word) != 0 {
        return Err(bad("invalid MSP header checksum"));
    }
    geometry(u32::from(words[2]), u32::from(words[3]))?;
    Ok(Header {
        version,
        width: words[2],
        height: words[3],
        words,
    })
}
/// Complete structural validation precedes every decoded-pixel allocation.
/// MSP2 row-map entries are encoded byte lengths, not arbitrary file offsets.
pub fn inspect(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<Header> {
    check()?;
    let header = header(bytes)?;
    let row_bytes = header.row_bytes();
    let height = usize::from(header.height);
    match header.version {
        Version::UncompressedV1 => {
            if bytes.len() != 32 + row_bytes * height {
                return Err(bad(
                    "MSP1 payload must contain exactly its byte-aligned rows",
                ));
            }
        }
        Version::RowRleV2 => {
            let start = 32 + height * 2;
            let map = bytes
                .get(32..start)
                .ok_or_else(|| bad("truncated MSP2 row map"))?;
            let mut offset = start;
            for row in map.as_chunks::<2>().0 {
                check()?;
                let length = usize::from(u16::from_le_bytes(*row));
                // Existing independent reader calls white-fill semantics an
                // assumption. Do not activate guessed background behavior.
                if length == 0 {
                    return Err(unsupported("zero-length MSP2 rows are not qualified"));
                }
                let end = offset
                    .checked_add(length)
                    .filter(|end| *end <= bytes.len())
                    .ok_or_else(|| bad("MSP2 row extends beyond source"))?;
                row_packets(&bytes[offset..end], row_bytes, None, check)?;
                offset = end;
            }
            if offset != bytes.len() {
                return Err(bad("MSP2 trailing or unclaimed source bytes"));
            }
        }
    }
    check()?;
    Ok(header)
}
fn row_packets(
    row: &[u8],
    expected: usize,
    mut output: Option<&mut Vec<u8>>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let (mut offset, mut pixels, mut packets) = (0, 0usize, 0usize);
    while offset < row.len() {
        if packets.is_multiple_of(256) {
            check()?;
        }
        packets += 1;
        let kind = row[offset];
        offset += 1;
        let (count, value, literal) = if kind == 0 {
            let pair = row
                .get(offset..offset + 2)
                .ok_or_else(|| bad("truncated MSP2 repeat packet"))?;
            offset += 2;
            if pair[0] == 0 {
                return Err(bad("zero-count MSP2 repeat packet"));
            }
            (usize::from(pair[0]), pair[1], None)
        } else {
            let count = usize::from(kind);
            let data = row
                .get(offset..offset + count)
                .ok_or_else(|| bad("truncated MSP2 literal packet"))?;
            offset += count;
            (count, 0, Some(data))
        };
        pixels = pixels
            .checked_add(count)
            .filter(|n| *n <= expected)
            .ok_or_else(|| bad("MSP2 packet exceeds its decoded row"))?;
        if let Some(output) = output.as_deref_mut() {
            if let Some(literal) = literal {
                output.extend_from_slice(literal);
            } else {
                output.resize(output.len() + count, value);
            }
        }
    }
    if pixels != expected {
        return Err(bad("MSP2 decoded row length mismatch"));
    }
    Ok(())
}
pub fn decode(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<Bitmap> {
    let header = inspect(bytes, check)?;
    let mut packed = buffer(header.row_bytes() * usize::from(header.height))?;
    match header.version {
        Version::UncompressedV1 => {
            for chunk in bytes[32..].chunks(65536) {
                check()?;
                packed.extend_from_slice(chunk);
            }
        }
        Version::RowRleV2 => {
            let map = &bytes[32..32 + usize::from(header.height) * 2];
            let mut offset = 32 + map.len();
            for length in map.as_chunks::<2>().0 {
                check()?;
                let end = offset + usize::from(u16::from_le_bytes(*length));
                row_packets(
                    &bytes[offset..end],
                    header.row_bytes(),
                    Some(&mut packed),
                    check,
                )?;
                offset = end;
            }
        }
    }
    check()?;
    Ok(Bitmap { header, packed })
}
