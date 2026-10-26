//! Allocation-free container admission before the selected decoder's header/ICC
//! work. This validates framing, not metadata contents or codestream entropy.
//!
//! Reference: libjxl v0.12.0 lib/jxl/decode.cc (ftyp and partial codestream
//! stages), and jxl 0.8.0 api/box_parser.rs (level and brob wrapper rules).
//! https://github.com/libjxl/libjxl/blob/v0.12.0/lib/jxl/decode.cc
//! https://github.com/libjxl/jxl-rs/blob/9a08ca643fd2d01f803dd0be6a5bebc9cc11e597/jxl/src/api/box_parser.rs
use super::*;
const BOXES_MAX: usize = 4096;

struct Fragments {
    seen: [u64; BOXES_MAX / 64],
    count: usize,
    highest: usize,
    last: Option<usize>,
}
impl Default for Fragments {
    fn default() -> Self {
        Self {
            seen: [0; BOXES_MAX / 64],
            count: 0,
            highest: 0,
            last: None,
        }
    }
}
impl Fragments {
    fn add(&mut self, payload: &[u8], version: u32, unbounded: bool) -> Result<()> {
        let word = payload
            .get(..4)
            .ok_or_else(|| codec("Truncated JPEG XL fragment index"))?;
        let word = u32::from_be_bytes(word.try_into().unwrap());
        let index = (word & 0x7fff_ffff) as usize;
        let last = word & 0x8000_0000 != 0;
        if index >= BOXES_MAX {
            return Err(resource("JPEG XL fragment index exceeds box allowance"));
        }
        if self.seen[index / 64] & (1 << (index % 64)) != 0 {
            return Err(codec("Duplicate JPEG XL fragment index"));
        }
        // Version 0 requires physical order. Version 1 permits buffering a
        // later logical part; retain that format feature within the box limit.
        if version == 0 && index != self.count {
            return Err(codec(
                "Out-of-order JPEG XL fragments require file version 1",
            ));
        }
        if unbounded && (!last || index != self.count) {
            return Err(codec(
                "Unbounded JPEG XL fragment must be the final in-order part",
            ));
        }
        if self.last.is_some_and(|final_index| index > final_index)
            || (last && (self.last.is_some() || self.highest > index))
        {
            return Err(codec("Conflicting JPEG XL final fragment index"));
        }
        self.seen[index / 64] |= 1 << (index % 64);
        self.highest = self.highest.max(index);
        self.count += 1;
        if last {
            self.last = Some(index);
        }
        Ok(())
    }
    fn finish(&self) -> Result<()> {
        // Unique nonnegative indices, highest=count-1, prove there are no gaps.
        if self.count > 0 && self.last == Some(self.count - 1) && self.highest == self.count - 1 {
            Ok(())
        } else {
            Err(codec("Incomplete JPEG XL fragment sequence or last marker"))
        }
    }
}

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
    let mut version = None;
    let mut level = false;
    let mut single = false;
    let mut fragments = Fragments::default();
    while offset < bytes.len() {
        check()?;
        count += 1;
        if count > BOXES_MAX {
            return Err(resource("JPEG XL has too many container boxes"));
        }
        let header = bytes
            .get(offset..offset.saturating_add(8))
            .ok_or_else(|| codec("Truncated JPEG XL box header"))?;
        let wire_length = u32::from_be_bytes(header[..4].try_into().unwrap());
        let mut header_len = 8usize;
        let length = match wire_length {
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
        let payload = &bytes[offset + header_len..end];
        if version.is_none() && kind != b"ftyp" {
            return Err(codec("JPEG XL file type must follow its signature"));
        }
        match kind {
            b"ftyp" => {
                if version.is_some()
                    || payload.len() < 12
                    || !payload.len().is_multiple_of(4)
                    || &payload[..4] != b"jxl "
                {
                    return Err(codec("Invalid or duplicate JPEG XL file type"));
                }
                let value = u32::from_be_bytes(payload[4..8].try_into().unwrap());
                if value > 1 {
                    return Err(unsupported("JPEG XL file type version is not qualified"));
                }
                version = Some(value);
            }
            b"jxlc" => {
                if single || fragments.count != 0 {
                    return Err(codec("Mixed or duplicate JPEG XL codestream boxes"));
                }
                if payload.is_empty() {
                    return Err(codec("Empty JPEG XL codestream box"));
                }
                single = true;
            }
            b"jxlp" => {
                if single {
                    return Err(codec("Mixed JPEG XL whole and partial codestreams"));
                }
                fragments.add(payload, version.unwrap(), wire_length == 0)?;
            }
            b"jxll" => {
                if level || single || fragments.count != 0 || payload.len() != 1 {
                    return Err(codec("Invalid, duplicate or misplaced JPEG XL level box"));
                }
                // A Level 10 declaration can contain a simpler image (the
                // independent RGBA16 fixture does). This does not raise the
                // selected decoder's Main5 complexity allowance.
                if !matches!(payload[0], 5 | 10) {
                    return Err(codec("Invalid JPEG XL container level"));
                }
                level = true;
            }
            b"JXL " => return Err(codec("Duplicate JPEG XL signature box")),
            _ => {
                if kind == b"brob" {
                    let inner = payload
                        .get(..4)
                        .ok_or_else(|| codec("Truncated JPEG XL compressed metadata type"))?;
                    if matches!(inner, b"brob" | [b'j', b'x', b'l', _] | b"jbrd") {
                        return Err(codec("Invalid JPEG XL compressed metadata wrapper"));
                    }
                    // Metadata is not requested from the selected decoder, so
                    // compressed auxiliary bytes are skipped, never expanded.
                }
                has_metadata = true;
                metadata = metadata
                    .checked_add(payload.len())
                    .ok_or_else(|| resource("JPEG XL metadata length overflow"))?;
                if metadata > METADATA_BYTES_MAX {
                    return Err(resource("JPEG XL exceeds the metadata byte limit"));
                }
            }
        }
        offset = end;
    }
    if version.is_none() {
        return Err(codec("Missing JPEG XL file type"));
    }
    if !single {
        fragments.finish()?;
    }
    Ok(has_metadata)
}
