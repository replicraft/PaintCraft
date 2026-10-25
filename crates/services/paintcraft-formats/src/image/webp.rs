//! RIFF/WebP still-image admission before codec allocation.
use super::*;

fn dimensions(bytes: &[u8], lossless: bool) -> Result<(u64, u64)> {
    if lossless {
        let h = bytes
            .get(..5)
            .ok_or_else(|| codec("truncated WebP lossless header"))?;
        if h[0] != 0x2f || h[4] & 0xe0 != 0 {
            return Err(codec("invalid WebP lossless signature/version"));
        }
        let value = u32::from_le_bytes(h[1..5].try_into().unwrap());
        Ok((
            u64::from(value & 0x3fff) + 1,
            u64::from((value >> 14) & 0x3fff) + 1,
        ))
    } else {
        let h = bytes
            .get(..10)
            .ok_or_else(|| codec("truncated WebP VP8 header"))?;
        if h[0] & 1 != 0 || h[3..6] != [0x9d, 0x01, 0x2a] {
            return Err(codec("WebP requires a VP8 key frame"));
        }
        if h[0] & 0x10 == 0 {
            return Err(unsupported("hidden VP8 frames are not qualified"));
        }
        let w = u16::from_le_bytes([h[6], h[7]]);
        let h = u16::from_le_bytes([h[8], h[9]]);
        if w & 0xc000 != 0 || h & 0xc000 != 0 {
            return Err(unsupported("scaled VP8 dimensions are not qualified"));
        }
        Ok((u64::from(w), u64::from(h)))
    }
}
fn u24(bytes: &[u8]) -> u64 {
    u64::from(bytes[0]) | u64::from(bytes[1]) << 8 | u64::from(bytes[2]) << 16
}
pub(super) fn admit(bytes: &[u8], pixels: u64) -> Result<Vec<String>> {
    let header = bytes
        .get(..12)
        .ok_or_else(|| codec("truncated WebP RIFF header"))?;
    if &header[..4] != b"RIFF" || &header[8..] != b"WEBP" {
        return Err(codec("invalid WebP RIFF signature"));
    }
    let size = u32::from_le_bytes(header[4..8].try_into().unwrap()) as u64 + 8;
    if size != bytes.len() as u64 {
        return Err(codec("WebP RIFF length mismatch"));
    }
    let mut pos = 12usize;
    let mut canvas = None;
    let mut flags = None;
    let mut lossless_alpha = false;
    let mut picture = None;
    let mut seen = Vec::<[u8; 4]>::new();
    let mut metadata = 0usize;
    let mut losses = Vec::new();
    while pos < bytes.len() {
        let h = bytes
            .get(pos..pos + 8)
            .ok_or_else(|| codec("truncated WebP chunk header"))?;
        let tag: [u8; 4] = h[..4].try_into().unwrap();
        let n = u32::from_le_bytes(h[4..8].try_into().unwrap()) as usize;
        pos += 8;
        let end = pos
            .checked_add(n)
            .ok_or_else(|| codec("WebP chunk offset overflow"))?;
        let data = bytes
            .get(pos..end)
            .ok_or_else(|| codec("truncated WebP chunk"))?;
        pos = end
            .checked_add(n & 1)
            .ok_or_else(|| codec("WebP chunk padding overflow"))?;
        if n & 1 != 0 && bytes.get(end) != Some(&0) {
            return Err(codec("invalid WebP chunk padding"));
        }
        if seen.contains(&tag) {
            return Err(codec("duplicate WebP chunk"));
        }
        if seen.len() >= 16 {
            return Err(Error::new(ErrorCode::ResourceLimit, "too many WebP chunks"));
        }
        seen.push(tag);
        match &tag {
            b"ANIM" | b"ANMF" => {
                return Err(unsupported("animated WebP requires explicit frame import"));
            }
            b"VP8X" => {
                if seen.len() != 1 || n != 10 || data[0] & 0xc1 != 0 || data[1..4] != [0; 3] {
                    return Err(codec("invalid WebP extended header"));
                }
                if data[0] & 2 != 0 {
                    return Err(unsupported("animated WebP requires explicit frame import"));
                }
                let dims = (u24(&data[4..7]) + 1, u24(&data[7..10]) + 1);
                check_size(dims.0, dims.1)?;
                if dims.0 * dims.1 > pixels {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "WebP canvas exceeds pixel budget",
                    ));
                }
                canvas = Some(dims);
                flags = Some(data[0]);
            }
            b"VP8 " | b"VP8L" => {
                if picture.is_some() {
                    return Err(codec("WebP contains multiple image bitstreams"));
                }
                let dims = dimensions(data, &tag == b"VP8L")?;
                check_size(dims.0, dims.1)?;
                if dims.0 * dims.1 > pixels {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "WebP exceeds pixel budget",
                    ));
                }
                if canvas.is_some_and(|canvas| canvas != dims) {
                    return Err(codec("WebP canvas and bitstream dimensions disagree"));
                }
                if n as u64 > dims.0 * dims.1 * 8 + (1 << 20) {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "WebP bitstream exceeds byte budget",
                    ));
                }
                if &tag == b"VP8L" {
                    lossless_alpha = data[4] & 0x10 != 0;
                }
                picture = Some(dims);
            }
            b"ALPH" => {
                if canvas.is_none() || picture.is_some() || n == 0 {
                    return Err(codec("invalid WebP alpha chunk placement"));
                }
                let (w, h) = canvas.unwrap();
                if n as u64 > w * h * 2 + 64 {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "WebP alpha data exceeds byte budget",
                    ));
                }
            }
            b"ICCP" | b"EXIF" | b"XMP " => {
                if canvas.is_none() {
                    return Err(codec("WebP metadata requires an extended header"));
                }
                metadata = metadata
                    .checked_add(n)
                    .ok_or_else(|| codec("WebP metadata size overflow"))?;
                if metadata > 1 << 20 {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "WebP metadata exceeds byte budget",
                    ));
                }
                if &tag == b"XMP " {
                    losses.push("webp-metadata-ignored".into());
                }
            }
            _ => return Err(unsupported("unknown WebP chunk is not qualified")),
        }
    }
    if picture.is_none() {
        return Err(codec("WebP has no image bitstream"));
    }
    if seen.contains(b"ALPH") && seen.contains(b"VP8L") {
        return Err(codec("lossless WebP cannot have a separate alpha chunk"));
    }
    if let Some(flags) = flags {
        for (flag, present) in [
            (0x20, seen.contains(b"ICCP")),
            (0x08, seen.contains(b"EXIF")),
            (0x04, seen.contains(b"XMP ")),
            (0x10, seen.contains(b"ALPH") || lossless_alpha),
        ] {
            if (flags & flag != 0) != present {
                return Err(codec("WebP extended feature flags disagree with chunks"));
            }
        }
    }
    Ok(losses)
}
pub(super) fn encode(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    ::image::codecs::webp::WebPEncoder::new_lossless(&mut output)
        .write_image(rgba, width, height, ::image::ExtendedColorType::Rgba8)
        .map_err(image_error)?;
    Ok(output)
}
