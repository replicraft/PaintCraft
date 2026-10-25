//! Bounded single-frame GIF admission before the codec reads extensions.
use super::*;

pub(super) fn admit(bytes: &[u8], pixels: u64) -> Result<Vec<String>> {
    let header = bytes
        .get(..13)
        .ok_or_else(|| codec("truncated GIF header"))?;
    let word = |at| u16::from_le_bytes([header[at], header[at + 1]]) as u64;
    let (width, height) = (word(6), word(8));
    check_size(width, height)?;
    if width * height > pixels {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "GIF exceeds the pixel budget",
        ));
    }
    let mut pos = 13;
    if header[10] & 128 != 0 {
        take(bytes, &mut pos, 3usize << ((header[10] & 7) + 1))?;
    }
    let mut frames = 0;
    let mut metadata = 0usize;
    let mut extensions = false;
    loop {
        match take(bytes, &mut pos, 1)?[0] {
            0x3b => {
                if frames != 1 || pos != bytes.len() {
                    return Err(codec("GIF requires exactly one frame and a final trailer"));
                }
                let mut losses = vec!["palette-indices-expanded".into()];
                if extensions {
                    losses.push("gif-metadata-ignored".into());
                }
                return Ok(losses);
            }
            0x21 => {
                let label = take(bytes, &mut pos, 1)?[0];
                if label == 0x01 {
                    return Err(unsupported("GIF plain-text rendering is not qualified"));
                }
                extensions |= label != 0xf9;
                blocks(bytes, &mut pos, Some(&mut metadata))?;
            }
            0x2c => {
                frames += 1;
                if frames > 1 {
                    return Err(unsupported(
                        "animated GIF requires explicit frame import; still-image decoding refuses animation",
                    ));
                }
                let frame = take(bytes, &mut pos, 9)?;
                let word = |at| u16::from_le_bytes([frame[at], frame[at + 1]]) as u64;
                let (left, top, w, h) = (word(0), word(2), word(4), word(6));
                if w == 0 || h == 0 || left + w > width || top + h > height {
                    return Err(codec("GIF frame lies outside its logical screen"));
                }
                if left != 0 || top != 0 || w != width || h != height {
                    return Err(unsupported(
                        "partial GIF frames require explicit logical-screen background composition",
                    ));
                }
                if frame[8] & 128 != 0 {
                    take(bytes, &mut pos, 3usize << ((frame[8] & 7) + 1))?;
                }
                let code_size = take(bytes, &mut pos, 1)?[0];
                if !(2..=8).contains(&code_size) {
                    return Err(codec("invalid GIF LZW code size"));
                }
                blocks(bytes, &mut pos, None)?;
            }
            _ => return Err(codec("invalid GIF block")),
        }
    }
}

fn take<'a>(bytes: &'a [u8], pos: &mut usize, count: usize) -> Result<&'a [u8]> {
    let end = pos
        .checked_add(count)
        .ok_or_else(|| codec("GIF offset overflow"))?;
    let result = bytes
        .get(*pos..end)
        .ok_or_else(|| codec("truncated GIF block"))?;
    *pos = end;
    Ok(result)
}
fn blocks(bytes: &[u8], pos: &mut usize, mut metadata: Option<&mut usize>) -> Result<()> {
    loop {
        let length = take(bytes, pos, 1)?[0] as usize;
        if let Some(total) = metadata.as_deref_mut() {
            *total += length + 1;
            if *total > 1 << 20 {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "GIF metadata exceeds the byte budget",
                ));
            }
        }
        if length == 0 {
            return Ok(());
        }
        take(bytes, pos, length)?;
    }
}

pub(super) fn encode(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    if width > u16::MAX as u32 || height > u16::MAX as u32 {
        return Err(unsupported("GIF supports at most 65535 pixels per side"));
    }
    let mut output = Vec::new();
    ::image::codecs::gif::GifEncoder::new(&mut output)
        .encode(rgba, width, height, ::image::ExtendedColorType::Rgba8)
        .map_err(image_error)?;
    Ok(output)
}
