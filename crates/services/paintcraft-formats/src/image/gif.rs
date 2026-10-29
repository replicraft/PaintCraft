//! Bounded still-GIF composition; no animation or disposal timeline is imported.
use super::*;

#[derive(Clone, Copy)]
struct Control {
    transparent: Option<u8>,
    disposal: u8,
    delay: u16,
    user_input: bool,
}
#[derive(Clone, Copy)]
struct Frame {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    interlaced: bool,
    control: Option<Control>,
}
struct Inspection<'a> {
    width: u32,
    height: u32,
    frame: Frame,
    background: Option<[u8; 4]>,
    palette: &'a [u8],
    global: Option<&'a [u8]>,
    local: bool,
    losses: Vec<String>,
}
fn gif_error(error: ::gif::DecodingError) -> Error {
    match error {
        ::gif::DecodingError::MemoryLimit | ::gif::DecodingError::OutOfMemory => {
            Error::new(ErrorCode::ResourceLimit, "GIF decoder allocation allowance")
        }
        _ => codec(error.to_string()),
    }
}
fn buffer(size: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "GIF output allocation"))?;
    bytes.resize(size, 0);
    Ok(bytes)
}
/// Decode actual indices with the selected GIF backend, then compose only the
/// qualified still policy. Hashes/previews never supply source palette facts.
pub(super) fn decode(bytes: &[u8], pixels: u64) -> Result<Decoded> {
    let mut profile = inspect(bytes, pixels)?;
    let mut options = ::gif::DecodeOptions::new();
    options.set_color_output(::gif::ColorOutput::Indexed);
    options.set_memory_limit(::gif::MemoryLimit::Bytes(
        std::num::NonZeroU64::new(u64::from(profile.frame.width) * u64::from(profile.frame.height))
            .unwrap(),
    ));
    options.check_frame_consistency(true);
    options.check_lzw_end_code(true);
    let mut reader = options.read_info(Cursor::new(bytes)).map_err(gif_error)?;
    if (u32::from(reader.width()), u32::from(reader.height())) != (profile.width, profile.height)
        || reader.global_palette() != profile.global
    {
        return Err(codec(
            "GIF backend logical screen or global palette mismatch",
        ));
    }
    let backend = reader
        .next_frame_info()
        .map_err(gif_error)?
        .ok_or_else(|| codec("GIF backend lacks the admitted frame"))?;
    let frame = profile.frame;
    if (
        u32::from(backend.left),
        u32::from(backend.top),
        u32::from(backend.width),
        u32::from(backend.height),
        backend.interlaced,
    ) != (
        frame.left,
        frame.top,
        frame.width,
        frame.height,
        frame.interlaced,
    ) || backend.palette.is_some() != profile.local
        || backend.palette.as_deref().or(profile.global) != Some(profile.palette)
        || backend.transparent != frame.control.and_then(|c| c.transparent)
    {
        return Err(codec("GIF backend frame or palette mismatch"));
    }
    if let Some(control) = frame.control
        && (backend.delay != control.delay
            || backend.dispose as u8 != control.disposal
            || backend.needs_user_input != control.user_input)
    {
        return Err(codec("GIF backend graphic control mismatch"));
    }
    let frame_pixels = u64::from(frame.width) * u64::from(frame.height);
    let screen_bytes = u64::from(profile.width) * u64::from(profile.height) * 4;
    admit_buffer_bytes(screen_bytes, isize::MAX as u64)?;
    let mut indices =
        buffer(usize::try_from(frame_pixels).map_err(|_| codec("GIF frame address extent"))?)?;
    // A single full index output avoids the selected backend's strict-EOI
    // NoProgress failure on row-sized interlace buffers. This API returns the
    // encoded row order; compose the explicit GIF89a four passes below.
    if !reader.fill_buffer(&mut indices).map_err(gif_error)? {
        return Err(codec("GIF has fewer samples than its admitted frame"));
    }
    // Filling the expected output alone does not prove an end marker or rule
    // out extra samples. Drain with strict EOI validation before publishing.

    if reader.next_frame_info().map_err(gif_error)?.is_some() {
        return Err(codec("GIF backend returned an unadmitted frame"));
    }
    if reader.icc_profile().is_some() {
        profile.losses.push("icc-profile-ignored".into());
    }
    let mut rgba = buffer(
        usize::try_from(screen_bytes).map_err(|_| codec("GIF logical-screen address extent"))?,
    )?;
    if let Some(background) = profile.background {
        for pixel in rgba.as_chunks_mut::<4>().0 {
            *pixel = background;
        }
    }
    let mut rows = indices.chunks_exact(frame.width as usize);
    let passes: &[(u32, u32)] = if frame.interlaced {
        &[(0, 8), (4, 8), (2, 4), (1, 2)]
    } else {
        &[(0, 1)]
    };
    for &(start, step) in passes {
        for y in (start..frame.height).step_by(step as usize) {
            let row = rows
                .next()
                .ok_or_else(|| codec("GIF interlace row count mismatch"))?;
            for (x, &index) in row.iter().enumerate() {
                let at = usize::from(index) * 3;
                let color = profile
                    .palette
                    .get(at..at + 3)
                    .ok_or_else(|| codec("GIF sample index lies outside its color table"))?;
                // Inspector proved every frame row lies inside the logical
                // screen; the admitted screen byte extent also fits usize.
                let dst = ((u64::from(frame.top + y) * u64::from(profile.width)
                    + u64::from(frame.left)
                    + x as u64)
                    * 4) as usize;
                rgba[dst..dst + 3].copy_from_slice(color);
                rgba[dst + 3] = if frame.control.and_then(|c| c.transparent) == Some(index) {
                    0
                } else {
                    255
                };
            }
        }
    }
    if rows.next().is_some() || !rows.remainder().is_empty() {
        return Err(codec("GIF has extra interlace rows"));
    }
    Ok(Decoded {
        format: ImageFormat::Gif,
        width: profile.width,
        height: profile.height,
        rgba,
        losses: profile.losses,
    })
}

fn inspect(bytes: &[u8], pixels: u64) -> Result<Inspection<'_>> {
    if bytes.len() as u64 > 1 << 30 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "GIF encoded source allowance",
        ));
    }
    let header = bytes
        .get(..13)
        .ok_or_else(|| codec("truncated GIF header"))?;
    if !matches!(&header[..6], b"GIF87a" | b"GIF89a") {
        return Err(codec("invalid GIF version"));
    }
    let word = |at| u32::from(u16::from_le_bytes([header[at], header[at + 1]]));
    let (width, height) = (word(6), word(8));
    check_size(u64::from(width), u64::from(height))?;
    if u64::from(width) * u64::from(height) > pixels {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "GIF exceeds the pixel budget",
        ));
    }
    let mut pos = 13;
    let global = if header[10] & 128 != 0 {
        Some(take(bytes, &mut pos, 3usize << ((header[10] & 7) + 1))?)
    } else {
        None
    };
    let mut frame = None;
    let mut source_palette = None;
    let mut source_local = false;
    let mut background = None;
    let mut control = None;
    let mut metadata = 0usize;
    let mut omitted = header[12] != 0;
    loop {
        match take(bytes, &mut pos, 1)?[0] {
            0x3b => {
                let frame = frame.ok_or_else(|| codec("GIF requires exactly one frame"))?;
                if pos != bytes.len() || control.is_some() {
                    return Err(codec("GIF has trailing data or an orphan graphic control"));
                }
                let mut losses = vec!["palette-indices-expanded".into()];
                if omitted {
                    losses.push("gif-metadata-ignored".into());
                }
                return Ok(Inspection {
                    width,
                    height,
                    frame,
                    background,
                    palette: source_palette.ok_or_else(|| codec("GIF source palette missing"))?,
                    global,
                    local: source_local,
                    losses,
                });
            }
            0x21 => {
                let label = take(bytes, &mut pos, 1)?[0];
                if label == 0x01 {
                    return Err(unsupported("GIF plain-text rendering is not qualified"));
                }
                if label == 0xf9 {
                    if control.is_some() {
                        return Err(codec("GIF has duplicate graphic control for one image"));
                    }
                    if take(bytes, &mut pos, 1)?[0] != 4 {
                        return Err(codec("invalid GIF graphic control length"));
                    }
                    let gce = take(bytes, &mut pos, 4)?;
                    if take(bytes, &mut pos, 1)?[0] != 0 || gce[0] & 0xe0 != 0 {
                        return Err(codec("invalid GIF graphic control fields"));
                    }
                    if (gce[0] >> 2) & 7 > 3 {
                        return Err(unsupported("reserved GIF disposal mode"));
                    }
                    // Disposal happens after display, so modes0..3 cannot erase
                    // the one still image being imported. Timing is not retained.
                    omitted |= gce[0] & 2 != 0 || gce[1] != 0 || gce[2] != 0;
                    control = Some(Control {
                        transparent: (gce[0] & 1 != 0).then_some(gce[3]),
                        disposal: (gce[0] >> 2) & 7,
                        delay: u16::from_le_bytes([gce[1], gce[2]]),
                        user_input: gce[0] & 2 != 0,
                    });
                    metadata = metadata
                        .checked_add(6)
                        .ok_or_else(|| codec("GIF metadata size overflow"))?;
                    if metadata > 1 << 20 {
                        return Err(Error::new(
                            ErrorCode::ResourceLimit,
                            "GIF metadata exceeds the byte budget",
                        ));
                    }
                } else {
                    omitted = true;
                    blocks(bytes, &mut pos, Some(&mut metadata))?;
                }
            }
            0x2c => {
                if frame.is_some() {
                    return Err(unsupported(
                        "animated GIF requires explicit frame import; still-image decoding refuses animation",
                    ));
                }
                let image = take(bytes, &mut pos, 9)?;
                let word = |at| u32::from(u16::from_le_bytes([image[at], image[at + 1]]));
                let current = Frame {
                    left: word(0),
                    top: word(2),
                    width: word(4),
                    height: word(6),
                    interlaced: image[8] & 64 != 0,
                    control: control.take(),
                };
                if current.width == 0
                    || current.height == 0
                    || current.left + current.width > width
                    || current.top + current.height > height
                {
                    return Err(codec("GIF frame lies outside its logical screen"));
                }
                if image[8] & 0x18 != 0 {
                    return Err(codec("reserved GIF image descriptor bits"));
                }
                let local = if image[8] & 128 != 0 {
                    Some(take(bytes, &mut pos, 3usize << ((image[8] & 7) + 1))?)
                } else {
                    None
                };
                let palette = local
                    .or(global)
                    .ok_or_else(|| unsupported("standalone GIF requires a color table"))?;
                source_palette = Some(palette);
                source_local = local.is_some();
                let transparent = current.control.and_then(|control| control.transparent);
                if transparent.is_some_and(|index| usize::from(index) >= palette.len() / 3) {
                    return Err(codec("GIF transparent index lies outside its color table"));
                }
                let partial = current.left != 0
                    || current.top != 0
                    || current.width != width
                    || current.height != height;
                if partial {
                    background = Some(if transparent.is_some() {
                        [0, 0, 0, 0]
                    } else {
                        let global = global.ok_or_else(|| {
                            unsupported("opaque partial GIF requires a declared global background")
                        })?;
                        let at = usize::from(header[11]) * 3;
                        let color = global.get(at..at + 3).ok_or_else(|| {
                            codec("GIF background index lies outside its global color table")
                        })?;
                        [color[0], color[1], color[2], 255]
                    });
                }
                let size = take(bytes, &mut pos, 1)?[0];
                if !(2..=8).contains(&size) {
                    return Err(codec("invalid GIF LZW code size"));
                }
                blocks(bytes, &mut pos, None)?;
                frame = Some(current);
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

#[cfg(test)]
#[path = "gif/tests.rs"]
mod tests;
