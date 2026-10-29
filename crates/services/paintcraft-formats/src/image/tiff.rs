//! Classic TIFF preflight. BigTIFF and nested/page structures are refused.
use super::*;

struct Directory<'a> {
    bytes: &'a [u8],
    little: bool,
}
impl Directory<'_> {
    fn field<const N: usize>(&self, pos: usize) -> Result<[u8; N]> {
        let end = pos
            .checked_add(N)
            .ok_or_else(|| codec("TIFF field offset overflow"))?;
        self.bytes
            .get(pos..end)
            .ok_or_else(|| codec("truncated TIFF field"))?
            .try_into()
            .map_err(|_| codec("invalid TIFF field length"))
    }
    fn word(&self, pos: usize) -> Result<u16> {
        let value = self.field(pos)?;
        Ok(if self.little {
            u16::from_le_bytes(value)
        } else {
            u16::from_be_bytes(value)
        })
    }
    fn long(&self, pos: usize) -> Result<u32> {
        let value = self.field(pos)?;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Source {
    pub width: u32,
    pub height: u32,
    pub orientation: u8,
}
pub(super) fn admit_source(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Source, Vec<String>)> {
    admit_directory(bytes, pixels, None, check)
}

#[cfg(test)]
pub(super) fn admit(bytes: &[u8], pixels: u64) -> Result<Vec<String>> {
    admit_directory(bytes, pixels, None, &|| Ok(())).map(|(_, losses)| losses)
}

pub(crate) fn admit_page(
    bytes: &[u8],
    offset: usize,
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<String>> {
    admit_directory(bytes, pixels, Some(offset), check).map(|(_, losses)| losses)
}

fn admit_directory(
    bytes: &[u8],
    pixels: u64,
    selected: Option<usize>,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Source, Vec<String>)> {
    check()?;
    let directory = Directory {
        bytes,
        little: bytes.starts_with(b"II"),
    };
    let offset = selected.unwrap_or(directory.long(4)? as usize);
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
    if selected.is_none() && directory.long(end)? != 0 {
        return Err(unsupported(
            "multipage TIFF requires explicit page import; still-image decoding refuses additional pages",
        ));
    }
    let (mut width, mut height) = (None, None);
    let mut orientation = 1u8;
    let (mut photometric, mut channels, mut alpha) = (None, None, false);
    let mut metadata = 0usize;
    let mut losses: Vec<String> = Vec::new();
    let mut seen = [0u64; 1024];
    for index in 0..count {
        check()?;
        let at = offset + 2 + index * 12;
        let (tag, kind, n) = (
            directory.word(at)?,
            directory.word(at + 2)?,
            directory.long(at + 4)? as usize,
        );
        let (slot, bit) = (usize::from(tag / 64), 1u64 << (tag % 64));
        if seen[slot] & bit != 0 {
            return Err(codec("duplicate TIFF field"));
        }
        seen[slot] |= bit;
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
                if n != 1 {
                    return Err(codec("invalid TIFF photometric tag"));
                }
                let model = directory.value(data, kind, 0)?;
                if !matches!(model, 0..=2) {
                    return Err(unsupported(
                        "TIFF palette/CMYK/other photometric models are not qualified",
                    ));
                }
                photometric = Some(model);
            }
            274 => {
                if kind != 3 || n != 1 {
                    return Err(codec("TIFF Orientation must be one SHORT"));
                }
                let value = directory.value(data, kind, 0)?;
                if !(1..=8).contains(&value) {
                    return Err(codec("TIFF Orientation is outside 1 through 8"));
                }
                orientation = value as u8;
            }
            277 => {
                if n != 1 {
                    return Err(codec("invalid TIFF samples-per-pixel tag"));
                }
                let samples = directory.value(data, kind, 0)?;
                if !(1..=4).contains(&samples) {
                    return Err(unsupported("TIFF channel count is not qualified"));
                }
                channels = Some(samples);
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
                alpha = true;
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
            // Facade metadata lookup may swallow an error after reducing its
            // value-buffer limit to image bytes. The original tag was already
            // bounded here, so its omission always needs explicit consent.
            34675 => {
                if !losses.iter().any(|v| v == "icc-profile-ignored") {
                    losses.push("icc-profile-ignored".into());
                }
            }
            259 | 266 | 273 | 278 | 279 | 317 | 322..=325 => {}
            _ => {
                if !losses.iter().any(|v| v == "tiff-metadata-ignored") {
                    losses.push("tiff-metadata-ignored".into());
                }
            }
        }
    }
    let color_channels = match photometric {
        Some(0 | 1) => 1,
        Some(2) => 3,
        _ => return Err(codec("missing TIFF photometric model")),
    };
    if channels.unwrap_or(1) != color_channels + u32::from(alpha) {
        return Err(unsupported(
            "TIFF extra samples must be explicitly declared as unassociated alpha",
        ));
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
    Ok((
        Source {
            width,
            height,
            orientation,
        },
        ["tiff-metadata-ignored", "icc-profile-ignored"]
            .into_iter()
            .filter(|code| losses.iter().any(|loss| loss == code))
            .map(str::to_owned)
            .collect(),
    ))
}

pub(super) fn encode(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    // image's RGBA writer does not declare ExtraSamples. Use its underlying
    // codec's explicit RGB + unassociated-alpha API to avoid ambiguous opacity.
    let error = |error: ::tiff::TiffError| codec(format!("TIFF encode failed: {error}"));
    {
        let mut encoder = ::tiff::encoder::TiffEncoder::new(&mut output).map_err(error)?;
        let mut image = encoder
            .new_image::<::tiff::encoder::colortype::RGB8>(width, height)
            .map_err(error)?;
        image
            .extra_samples(&[::tiff::tags::ExtraSamples::UnassociatedAlpha])
            .map_err(error)?;
        image.write_data(rgba).map_err(error)?;
    }
    Ok(output.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflowing_directory_offsets_fail_without_panicking() {
        let directory = Directory {
            bytes: &[],
            little: true,
        };
        assert!(directory.word(usize::MAX).is_err());
        assert!(directory.long(usize::MAX - 2).is_err());
        let bytes = [b'I', b'I', 42, 0, 255, 255, 255, 255];
        assert!(admit(&bytes, 1).is_err());
    }
}
