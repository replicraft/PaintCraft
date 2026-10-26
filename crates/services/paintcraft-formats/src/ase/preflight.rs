//! Upstream allows lossy names, crossed block boundaries and nested-group loss.
//! Prove exact extents, strict UTF-16 and the supported group topology first.
use super::*;
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| invalid("ASE extent overflow"))?;
        let value = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| invalid("truncated ASE block"))?;
        self.pos = end;
        Ok(value)
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn name(&mut self, total: &mut usize) -> Result<()> {
        let n = usize::from(self.u16()?);
        if n == 0 {
            return Err(invalid("ASE name lacks its null terminator"));
        }
        if n > NAME_UNITS_MAX + 1 {
            return Err(resource("ASE name unit allowance exceeded"));
        }
        *total += n * 2;
        if *total > NAME_BYTES_MAX {
            return Err(resource("ASE aggregate name allowance exceeded"));
        }
        let bytes = self.take(n * 2)?;
        if !bytes.ends_with(&[0, 0]) {
            return Err(invalid("ASE name lacks its null terminator"));
        }
        let units = bytes[..bytes.len() - 2]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_be_bytes([b[0], b[1]]));
        for character in char::decode_utf16(units) {
            if character.map_or(true, |c| c == '\0') {
                return Err(invalid("ASE name is not strict null-free UTF-16"));
            }
        }
        Ok(())
    }
}
pub(super) fn inspect(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<()> {
    check()?;
    if bytes.len() > BYTES_MAX {
        return Err(resource("ASE source exceeds byte allowance"));
    }
    let mut source = Cursor { bytes, pos: 0 };
    if source.take(8)? != b"ASEF\0\x01\0\0" {
        return Err(invalid("unsupported ASE signature/version"));
    }
    let blocks = source.u32()? as usize;
    if blocks > BLOCKS_MAX {
        return Err(resource("ASE block allowance exceeded"));
    }
    let (mut group, mut groups, mut swatches, mut names) = (false, 0, 0, 0);
    for _ in 0..blocks {
        check()?;
        let kind = source.u16()?;
        let n = source.u32()? as usize;
        let mut block = Cursor {
            bytes: source.take(n)?,
            pos: 0,
        };
        match kind {
            1 => {
                swatches += 1;
                if swatches > SWATCHES_MAX {
                    return Err(resource("ASE swatch allowance exceeded"));
                }
                block.name(&mut names)?;
                let channels = match block.take(4)? {
                    b"RGB " | b"LAB " => 3,
                    b"CMYK" => 4,
                    b"Gray" => 1,
                    _ => return Err(invalid("unknown ASE color model")),
                };
                for _ in 0..channels {
                    if !f32::from_bits(block.u32()?).is_finite() {
                        return Err(invalid("ASE channels must be finite"));
                    }
                }
                if block.u16()? > 2 {
                    return Err(invalid("unknown ASE swatch type"));
                }
            }
            0xc001 => {
                if group {
                    return Err(invalid(
                        "nested ASE groups cannot be retained by the selected codec",
                    ));
                }
                group = true;
                groups += 1;
                if groups > GROUPS_MAX {
                    return Err(resource("ASE group allowance exceeded"));
                }
                block.name(&mut names)?;
            }
            0xc002 => {
                if !group {
                    return Err(invalid("unmatched ASE group end"));
                }
                group = false;
            }
            _ => return Err(invalid("unknown ASE block type")),
        }
        if block.pos != block.bytes.len() {
            return Err(invalid("ASE block has trailing or inconsistent payload"));
        }
    }
    if group || source.pos != bytes.len() {
        return Err(invalid("unterminated ASE group or trailing data"));
    }
    Ok(())
}
