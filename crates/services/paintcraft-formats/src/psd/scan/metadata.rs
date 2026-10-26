use super::*;

pub(super) fn resources(
    data: &[u8],
    losses: &mut Vec<String>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    if data.len() > METADATA_MAX {
        return Err(resource("PSD image resources exceed metadata allowance"));
    }
    let mut c = Cursor::new(data);
    while c.remaining() > 0 {
        check()?;
        if c.take(4)? != b"8BIM" {
            return Err(malformed("invalid PSD image resource signature"));
        }
        let id = c.u16()?;
        let name_len = usize::from(c.u8()?);
        c.take(name_len)?;
        c.zeros((name_len + 1) % 2)?;
        let payload = c.section(false)?;
        c.zeros(payload.len() % 2)?;
        if id == 1039 {
            loss(losses, "icc-profile-ignored");
        } else {
            loss(losses, "psd-metadata-ignored");
        }
    }
    Ok(())
}

pub(super) fn layer(
    data: &[u8],
    losses: &mut Vec<String>,
    check: &dyn Fn() -> Result<()>,
) -> Result<(String, Option<u32>)> {
    let mut c = Cursor::new(data);
    if !c.section(false)?.is_empty() {
        return Err(unsupported("PSD layer masks are not qualified"));
    }
    let ranges = c.section(false)?;
    if !ranges.len().is_multiple_of(8)
        || ranges
            .as_chunks::<4>()
            .0
            .iter()
            .any(|part| *part != [0, 0, 255, 255])
    {
        return Err(unsupported(
            "PSD conditional blending ranges are not qualified",
        ));
    }
    let len = usize::from(c.u8()?);
    let pascal = c.take(len)?;
    let legacy_ascii = pascal.is_ascii();
    let mut name = String::from_utf8_lossy(pascal).into_owned();
    c.zeros((4 - (len + 1) % 4) % 4)?;
    let mut id = None;
    let mut seen = Vec::new();
    let mut unicode_name = false;
    while c.remaining() > 0 {
        check()?;
        if c.take(4)? != b"8BIM" {
            return Err(unsupported("PSD layer tagged signature is not qualified"));
        }
        let key: [u8; 4] = c.take(4)?.try_into().unwrap();
        if seen.contains(&key) {
            return Err(malformed("duplicate PSD layer metadata key"));
        }
        seen.push(key);
        let payload = c.section(false)?;
        c.zeros(payload.len() % 2)?;
        let mut p = Cursor::new(payload);
        match &key {
            b"luni" => {
                let count = p.u32()? as usize;
                if count > LAYER_NAME_UTF16_MAX {
                    return Err(resource("PSD layer name exceeds import allowance"));
                }
                let utf16 = p
                    .take(count * 2)?
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|unit| u16::from_be_bytes([unit[0], unit[1]]))
                    .collect::<Vec<_>>();
                name = String::from_utf16(&utf16)
                    .map_err(|_| malformed("invalid UTF-16 PSD layer name"))?;
                if name.ends_with('\0') {
                    name.pop();
                }
                if p.remaining() > 3 {
                    return Err(malformed("PSD Unicode layer name has an undeclared suffix"));
                }
                let remaining = p.remaining();
                p.zeros(remaining)?;
                unicode_name = true;
            }
            b"lyid" => {
                id = Some(p.u32()?);
            }
            b"iOpa" => {
                if p.u8()? != 255 {
                    return Err(unsupported("PSD fill opacity is not qualified"));
                }
                p.zeros(3)?;
            }
            b"knko" | b"tsly" | b"clbl" | b"infx" => {
                let value = p.u8()?;
                if value > 1 || (&key == b"knko" && value != 0) || (&key == b"tsly" && value != 1) {
                    return Err(unsupported("PSD layer blending options are not qualified"));
                }
                p.zeros(3)?;
            }
            b"lspf" | b"lyvr" => {
                p.take(4)?;
                loss(losses, "psd-metadata-ignored");
            }
            b"lnsr" => {
                p.take(4)?;
                loss(losses, "psd-metadata-ignored");
            }
            b"lclr" => {
                p.take(8)?;
                loss(losses, "psd-metadata-ignored");
            }
            b"fxrp" => {
                p.take(16)?;
                loss(losses, "psd-metadata-ignored");
            }
            _ => {
                return Err(unsupported(
                    "PSD layer extensions require a separately qualified conversion path",
                ));
            }
        }
        if p.remaining() != 0 {
            return Err(malformed("PSD layer metadata has an undeclared suffix"));
        }
    }
    if !legacy_ascii && !unicode_name {
        return Err(unsupported(
            "PSD legacy non-ASCII layer names require a Unicode override",
        ));
    }
    Ok((name, id))
}
