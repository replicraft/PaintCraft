//! Bounds checks on uncompressed container boxes; unknown payloads are never opened.
use super::*;
pub(super) fn inspect(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<bool> {
    check()?;
    if bytes.len() > INPUT_BYTES_MAX {
        return Err(resource("JPEG XL exceeds the input byte limit"));
    }
    if !sniff(bytes) {
        return Err(codec("Invalid JPEG XL signature"));
    }
    if bytes.starts_with(&[0xff, 0x0a]) {
        return Ok(false);
    }
    let mut offset = 12usize;
    let mut count = 0usize;
    let mut metadata = 0usize;
    let mut has_metadata = false;
    while offset < bytes.len() {
        check()?;
        count += 1;
        if count > 4096 {
            return Err(resource("JPEG XL has too many container boxes"));
        }
        let header = bytes
            .get(offset..offset.saturating_add(8))
            .ok_or_else(|| codec("Truncated JPEG XL box header"))?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap());
        let mut header_len = 8usize;
        let length = match length {
            0 => bytes.len() - offset,
            1 => {
                header_len = 16;
                let extended = bytes
                    .get(offset + 8..offset.saturating_add(16))
                    .ok_or_else(|| codec("Truncated JPEG XL extended box"))?;
                usize::try_from(u64::from_be_bytes(extended.try_into().unwrap()))
                    .map_err(|_| resource("JPEG XL box length overflow"))?
            }
            value => value as usize,
        };
        if length < header_len {
            return Err(codec("Invalid JPEG XL box length"));
        }
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| codec("JPEG XL box exceeds input"))?;
        let kind = &header[4..8];
        if !matches!(kind, b"ftyp" | b"jxlc" | b"jxlp" | b"jxll") {
            has_metadata = true;
            metadata = metadata
                .checked_add(length - header_len)
                .ok_or_else(|| resource("JPEG XL metadata length overflow"))?;
            if metadata > METADATA_BYTES_MAX {
                return Err(resource("JPEG XL exceeds the metadata byte limit"));
            }
        }
        offset = end;
    }
    Ok(has_metadata)
}
