//! Strict packed Windows DIB admission; image owns pixel decoding.
use super::*;
use zerocopy::byteorder::{I32, LE, U16, U32};
use zerocopy::{FromBytes, Immutable, KnownLayout, Unaligned};
mod reader;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DibKind {
    Dib,
    DibV5,
}
#[derive(FromBytes, Immutable, KnownLayout, Unaligned)]
#[repr(C)]
struct Header {
    size: U32<LE>,
    width: I32<LE>,
    height: I32<LE>,
    planes: U16<LE>,
    bpp: U16<LE>,
    compression: U32<LE>,
    image_size: U32<LE>,
    x_resolution: I32<LE>,
    y_resolution: I32<LE>,
    colors: U32<LE>,
    important: U32<LE>,
}
struct Layout {
    offset: u32,
    losses: Vec<String>,
}
fn u32_at(bytes: &[u8], at: usize) -> Result<u32> {
    bytes
        .get(at..at + 4)
        .and_then(|v| v.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| codec("truncated packed DIB"))
}
fn admit(
    bytes: &[u8],
    kind: DibKind,
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Layout> {
    check()?;
    if bytes.len() as u64 > 1 << 30 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "encoded DIB exceeds the input budget",
        ));
    }
    let (header, _) = Header::read_from_prefix(bytes).map_err(|_| codec("truncated DIB header"))?;
    let size = header.size.get();
    if !matches!(size, 40 | 108 | 124) || (kind == DibKind::DibV5 && size != 124) {
        return Err(unsupported("unsupported packed DIB header"));
    }
    if bytes.len() < size as usize {
        return Err(codec("truncated DIB header"));
    }
    let (width, height, bpp, compression) = (
        header.width.get(),
        header.height.get(),
        header.bpp.get(),
        header.compression.get(),
    );
    if width <= 0 || height == 0 || height == i32::MIN || header.planes.get() != 1 {
        return Err(codec("invalid DIB dimensions"));
    }
    let (width, height) = (width as u32, height.unsigned_abs());
    check_size(u64::from(width), u64::from(height))?;
    if u64::from(width) * u64::from(height) > pixels {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "DIB exceeds the pixel budget",
        ));
    }
    if !matches!(bpp, 1 | 4 | 8 | 16 | 24 | 32) {
        return Err(unsupported("unsupported DIB bit depth"));
    }
    let (masks, alpha) = match compression {
        0 => (0, false),
        3 if matches!(bpp, 16 | 32) => {
            let (red, green, blue) = (u32_at(bytes, 40)?, u32_at(bytes, 44)?, u32_at(bytes, 48)?);
            let alpha = if size >= 108 { u32_at(bytes, 52)? } else { 0 };
            let valid = match bpp {
                32 => {
                    (red, green, blue) == (0xff0000, 0xff00, 0xff)
                        && matches!(alpha, 0 | 0xff000000)
                }
                _ => {
                    matches!(
                        (red, green, blue),
                        (0xf800, 0x7e0, 0x1f) | (0x7c00, 0x3e0, 0x1f)
                    ) && alpha == 0
                }
            };
            if !valid {
                return Err(unsupported("unsupported DIB channel masks"));
            }
            (if size == 40 { 12 } else { 0 }, alpha != 0)
        }
        _ => return Err(unsupported("unsupported DIB compression")),
    };
    let colors = if bpp <= 8 && header.colors.get() == 0 {
        1u32 << bpp
    } else {
        header.colors.get()
    };
    if colors > if bpp <= 8 { 1u32 << bpp } else { 256 } {
        return Err(unsupported("DIB palette exceeds the declared subset"));
    }
    let offset = u64::from(size) + masks + u64::from(colors) * 4;
    let stride = (u64::from(width) * u64::from(bpp)).div_ceil(32) * 4;
    let bit_bytes = stride * u64::from(height);
    if header.image_size.get() != 0 && u64::from(header.image_size.get()) != bit_bytes {
        return Err(codec("inconsistent DIB pixel length"));
    }
    let end = offset + bit_bytes;
    if end > bytes.len() as u64 {
        return Err(codec("truncated DIB pixels or palette"));
    }
    let mut losses = Vec::new();
    if bpp <= 8 {
        losses.push("palette-indices-expanded".into());
        for row in bytes[offset as usize..end as usize].chunks_exact(stride as usize) {
            check()?;
            for x in 0..width as usize {
                if x % 4096 == 0 {
                    check()?;
                }
                let index = match bpp {
                    8 => u32::from(row[x]),
                    4 => u32::from((row[x / 2] >> (if x % 2 == 0 { 4 } else { 0 })) & 15),
                    _ => u32::from((row[x / 8] >> (7 - x % 8)) & 1),
                };
                if index >= colors {
                    return Err(codec("DIB palette index out of range"));
                }
            }
        }
    }
    if bpp == 32 && !alpha {
        losses.push("dib-alpha-unspecified".into());
    } else if alpha {
        // A channel mask declares coverage bits, not whether stored RGB was
        // premultiplied by its producer. Do not guess from channel magnitudes.
        losses.push("dib-alpha-mode-unspecified".into());
    }
    let mut metadata = header.x_resolution.get() != 0
        || header.y_resolution.get() != 0
        || header.important.get() != 0
        || (bpp > 8 && colors != 0);
    if size >= 108 {
        let space = u32_at(bytes, 56)?;
        metadata |= !matches!(space, 0 | 0x73524742) || bytes[60..108].iter().any(|b| *b != 0);
    }
    if size == 124 {
        let (start, length) = (u32_at(bytes, 112)?, u32_at(bytes, 116)?);
        if start != 0 || length != 0 {
            if !matches!(u32_at(bytes, 56)?, 0x4c494e4b | 0x4d424544)
                || u64::from(start) < end
                || length == 0
                || u64::from(start) + u64::from(length) > bytes.len() as u64
            {
                return Err(codec("invalid or overlapping DIB profile range"));
            }
            metadata = true;
        } else if matches!(u32_at(bytes, 56)?, 0x4c494e4b | 0x4d424544) {
            return Err(codec("missing DIB profile"));
        }
        metadata |= u32_at(bytes, 108)? != 0 || u32_at(bytes, 120)? != 0;
    }
    if metadata {
        losses.push("dib-metadata-ignored".into());
    }
    Ok(Layout {
        offset: offset as u32,
        losses,
    })
}

/// Decode declared packed DIB bytes without allocating a second encoded buffer.
/// Undeclared 32-bit alpha is imported opaque and reported for explicit consent.
pub fn decode_dib_with_pixel_limit(
    bytes: &[u8],
    kind: DibKind,
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    let pixels = pixels.min(DECODE_PIXELS_MAX);
    let layout = admit(bytes, kind, pixels, check)?;
    // An explicit file-header offset avoids relying on inferred DIB offsets for
    // V4/V5 mask layouts. The header lives on the stack; body bytes stay borrowed.
    let decoder =
        ::image::codecs::bmp::BmpDecoder::new(reader::Prefixed::new(bytes, layout.offset))
            .map_err(image_error)?;
    check()?;
    let decoded = decode_pixels(decoder, ImageFormat::Bmp, layout.losses, pixels)?;
    check()?;
    Ok(decoded)
}

#[cfg(test)]
mod tests;
