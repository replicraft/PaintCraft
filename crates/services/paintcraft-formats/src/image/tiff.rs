//! Classic TIFF preflight. BigTIFF and nested/page structures are refused.
use super::*;

struct Directory<'a> {
    bytes: &'a [u8],
    little: bool,
}
impl Directory<'_> {
    fn word(&self, pos: usize) -> Result<u16> {
        let value: [u8; 2] = self
            .bytes
            .get(pos..pos + 2)
            .ok_or_else(|| codec("truncated TIFF word"))?
            .try_into()
            .unwrap();
        Ok(if self.little {
            u16::from_le_bytes(value)
        } else {
            u16::from_be_bytes(value)
        })
    }
    fn long(&self, pos: usize) -> Result<u32> {
        let value: [u8; 4] = self
            .bytes
            .get(pos..pos + 4)
            .ok_or_else(|| codec("truncated TIFF offset"))?
            .try_into()
            .unwrap();
        Ok(if self.little {
            u32::from_le_bytes(value)
        } else {
            u32::from_be_bytes(value)
        })
    }
    fn value(&self, pos: usize, kind: u16, index: usize) -> Result<u32> {
        match kind {
            3 => self.word(pos + index * 2).map(u32::from),
            4 => self.long(pos + index * 4),
            _ => Err(unsupported("TIFF integer tag type is not qualified")),
        }
    }
}

pub(super) fn admit(bytes: &[u8], pixels: u64) -> Result<Vec<String>> {
    let directory = Directory {
        bytes,
        little: bytes.starts_with(b"II"),
    };
    let offset = directory.long(4)? as usize;
    if offset < 8 {
        return Err(codec("TIFF directory overlaps header"));
    }
    let count = directory.word(offset)? as usize;
    if count > 4096 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "TIFF has too many tags",
        ));
    }
    let end = offset
        .checked_add(2 + count * 12)
        .ok_or_else(|| codec("TIFF directory overflow"))?;
    if directory.long(end)? != 0 {
        return Err(unsupported(
            "multipage TIFF requires explicit page import; still-image decoding refuses additional pages",
        ));
    }
    let (mut width, mut height) = (None, None);
    let mut metadata = 0usize;
    let mut losses = Vec::new();
    for index in 0..count {
        let at = offset + 2 + index * 12;
        let (tag, kind, n) = (
            directory.word(at)?,
            directory.word(at + 2)?,
            directory.long(at + 4)? as usize,
        );
        let size = match kind {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 => 4,
            5 | 10 | 12 => 8,
            _ => return Err(unsupported("TIFF field type is not qualified")),
        };
        let length = n
            .checked_mul(size)
            .ok_or_else(|| codec("TIFF tag size overflow"))?;
        metadata = metadata
            .checked_add(length)
            .ok_or_else(|| codec("TIFF metadata overflow"))?;
        if metadata > 1 << 20 {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "TIFF metadata exceeds the byte budget",
            ));
        }
        let data = if length <= 4 {
            at + 8
        } else {
            directory.long(at + 8)? as usize
        };
        let data_end = data
            .checked_add(length)
            .ok_or_else(|| codec("TIFF tag offset overflow"))?;
        bytes
            .get(data..data_end)
            .ok_or_else(|| codec("truncated TIFF tag"))?;
        if n == 0 {
            return Err(codec("empty TIFF tag"));
        }
        match tag {
            256 | 257 => {
                if n != 1 {
                    return Err(codec("invalid TIFF dimensions tag"));
                }
                let value = directory.value(data, kind, 0)?;
                if tag == 256 {
                    if width.replace(value).is_some() {
                        return Err(codec("duplicate TIFF width"));
                    }
                } else if height.replace(value).is_some() {
                    return Err(codec("duplicate TIFF height"));
                }
            }
            258 => {
                if n > 4 {
                    return Err(unsupported("TIFF channel count is not qualified"));
                }
                let first = directory.value(data, kind, 0)?;
                if !matches!(first, 1 | 8 | 16) {
                    return Err(unsupported("TIFF depth is not qualified for RGBA8 import"));
                }
                for channel in 1..n {
                    if directory.value(data, kind, channel)? != first {
                        return Err(unsupported("mixed TIFF component depths are not qualified"));
                    }
                }
            }
            262 => {
                if !matches!(directory.value(data, kind, 0)?, 0..=2) {
                    return Err(unsupported(
                        "TIFF palette/CMYK/other photometric models are not qualified",
                    ));
                }
            }
            274 => {
                if directory.value(data, kind, 0)? != 1 {
                    return Err(unsupported(
                        "oriented TIFF requires an explicit orientation conversion",
                    ));
                }
            }
            277 => {
                if directory.value(data, kind, 0)? > 4 {
                    return Err(unsupported("TIFF channel count is not qualified"));
                }
            }
            284 => {
                if directory.value(data, kind, 0)? != 1 {
                    return Err(unsupported("planar TIFF is not qualified"));
                }
            }
            338 => {
                if n != 1 || directory.value(data, kind, 0)? != 2 {
                    return Err(unsupported("TIFF alpha must be explicitly unassociated"));
                }
            }
            339 => {
                if n > 4 {
                    return Err(unsupported("TIFF sample count is not qualified"));
                }
                for sample in 0..n {
                    if directory.value(data, kind, sample)? != 1 {
                        return Err(unsupported(
                            "signed/float TIFF samples require explicit precision conversion",
                        ));
                    }
                }
            }
            330 | 34665 | 34853 | 40965 => {
                return Err(unsupported(
                    "nested TIFF metadata/directories are not qualified",
                ));
            }
            34675 => {} // ImageDecoder reports the bounded ICC loss.
            259 | 266 | 273 | 278 | 279 | 317 | 322..=325 => {}
            _ => {
                if !losses.iter().any(|v| v == "tiff-metadata-ignored") {
                    losses.push("tiff-metadata-ignored".into());
                }
            }
        }
    }
    let (width, height) = (
        width.ok_or_else(|| codec("missing TIFF width"))?,
        height.ok_or_else(|| codec("missing TIFF height"))?,
    );
    check_size(width.into(), height.into())?;
    if u64::from(width) * u64::from(height) > pixels {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "TIFF exceeds the pixel budget",
        ));
    }
    Ok(losses)
}

pub(super) fn encode(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    ::image::codecs::tiff::TiffEncoder::new(&mut output)
        .write_image(rgba, width, height, ::image::ExtendedColorType::Rgba8)
        .map_err(image_error)?;
    Ok(output.into_inner())
}
