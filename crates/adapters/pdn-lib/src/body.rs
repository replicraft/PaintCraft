use super::*;

pub(super) fn inspect(
    bytes: &[u8],
    limits: &Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<BodyHeader> {
    check()?;
    let marker = bytes
        .get(..2)
        .ok_or_else(|| malformed("Truncated PDN body marker"))?;
    match marker {
        [0, 1] => {
            // PDN's deferred marker is separate from the NRBF header record.
            let record = bytes
                .get(2..19)
                .ok_or_else(|| malformed("Truncated PDN NRBF header"))?;
            if record[0] != 0 {
                return Err(malformed(
                    "PDN body does not start with an NRBF header record",
                ));
            }
            let int = |offset| i32::from_le_bytes(record[offset..offset + 4].try_into().unwrap());
            let header = NrbfHeader {
                root_id: int(1),
                header_id: int(5),
                major_version: int(9),
                minor_version: int(13),
            };
            if header.major_version != 1 || header.minor_version != 0 {
                return Err(unsupported("Unsupported NRBF protocol version"));
            }
            Ok(BodyHeader::DeferredNrbf(header))
        }
        [0x1f, 0x8b] => gzip(bytes, limits, check),
        _ => Err(unsupported("Unsupported PDN body encoding marker")),
    }
}
fn gzip(bytes: &[u8], limits: &Limits, check: &dyn Fn() -> Result<()>) -> Result<BodyHeader> {
    let fixed = bytes
        .get(..10)
        .ok_or_else(|| malformed("Truncated PDN gzip header"))?;
    if fixed[2] != 8 {
        return Err(unsupported("Unsupported PDN gzip compression method"));
    }
    let flags = fixed[3];
    if flags & 0xe0 != 0 {
        return Err(malformed("Reserved PDN gzip flags are set"));
    }
    let mut end = 10usize;
    if flags & 4 != 0 {
        let length = bytes
            .get(end..end + 2)
            .ok_or_else(|| malformed("Truncated PDN gzip extra length"))?;
        let length = usize::from(u16::from_le_bytes(length.try_into().unwrap()));
        end = end
            .checked_add(2 + length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| malformed("Truncated PDN gzip extra field"))?;
    }
    for flag in [8, 16] {
        if flags & flag != 0 {
            loop {
                if end > limits.header_bytes {
                    return Err(resource("PDN gzip header exceeds the byte limit"));
                }
                if end.is_multiple_of(4096) {
                    check()?;
                }
                let byte = *bytes
                    .get(end)
                    .ok_or_else(|| malformed("Unterminated PDN gzip header string"))?;
                end += 1;
                if byte == 0 {
                    break;
                }
            }
        }
    }
    if flags & 2 != 0 {
        end = end
            .checked_add(2)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| malformed("Truncated PDN gzip header CRC"))?;
    }
    if end > limits.header_bytes {
        return Err(resource("PDN gzip header exceeds the byte limit"));
    }
    // Require room for compressed bytes and a trailer without claiming the
    // compressed stream or either CRC has been validated.
    if bytes.len().saturating_sub(end) <= 8 {
        return Err(malformed("Truncated PDN gzip member"));
    }
    Ok(BodyHeader::Gzip {
        flags,
        header_bytes: end,
    })
}
