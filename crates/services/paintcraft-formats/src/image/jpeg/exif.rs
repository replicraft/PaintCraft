use super::*;

pub(super) fn orientation(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<u8> {
    if bytes.len() < 8 {
        return Err(malformed("JPEG EXIF header is truncated"));
    }
    let little = match &bytes[..4] {
        b"II\x2a\0" => true,
        b"MM\0\x2a" => false,
        _ => return Err(malformed("JPEG EXIF byte order or TIFF version is invalid")),
    };
    let word = |at: usize| -> Result<u16> {
        let data = bytes
            .get(
                at..at
                    .checked_add(2)
                    .ok_or_else(|| malformed("JPEG EXIF offset overflows"))?,
            )
            .ok_or_else(|| malformed("JPEG EXIF field is outside its segment"))?;
        let data = data.try_into().unwrap();
        Ok(if little {
            u16::from_le_bytes(data)
        } else {
            u16::from_be_bytes(data)
        })
    };
    let long = |at: usize| -> Result<u32> {
        let data = bytes
            .get(
                at..at
                    .checked_add(4)
                    .ok_or_else(|| malformed("JPEG EXIF offset overflows"))?,
            )
            .ok_or_else(|| malformed("JPEG EXIF field is outside its segment"))?;
        let data = data.try_into().unwrap();
        Ok(if little {
            u32::from_le_bytes(data)
        } else {
            u32::from_be_bytes(data)
        })
    };
    let at = long(4)? as usize;
    if at < 8 {
        return Err(malformed("JPEG EXIF directory overlaps its header"));
    }
    let count = usize::from(word(at)?);
    let end = at
        .checked_add(2 + count * 12 + 4)
        .ok_or_else(|| malformed("JPEG EXIF directory overflows"))?;
    if end > bytes.len() {
        return Err(malformed("JPEG EXIF directory is outside its segment"));
    }
    let mut result = None;
    for index in 0..count {
        check()?;
        let entry = at + 2 + index * 12;
        if word(entry)? == 0x112 {
            if result.is_some() || word(entry + 2)? != 3 || long(entry + 4)? != 1 {
                return Err(malformed(
                    "JPEG EXIF orientation is duplicate or has an invalid type/count",
                ));
            }
            let value = word(entry + 8)?;
            if !(1..=8).contains(&value) {
                return Err(malformed("JPEG EXIF orientation is outside 1 through 8"));
            }
            result = Some(value as u8);
        }
    }
    // IFD1 describes a thumbnail, not the full-resolution image's transform.
    Ok(result.unwrap_or(1))
}
