//! Flat PNG, BMP, JPEG, still GIF and single-page TIFF with explicit loss reporting.
//!
//! Decoders read only a declared subset, check every dimension against the
//! decode budget before allocating, and say what they could not keep. Nothing
//! is converted silently: a 16-bit PNG decodes with a recorded depth reduction,
//! an unsupported BMP variant is refused, and JPEG output (lossy, no alpha) is
//! reported as a loss the caller must acknowledge.

use ::image::{DynamicImage, ImageDecoder, ImageEncoder};
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::io::Cursor;

mod gif;
#[cfg(test)]
mod raster_tests;
mod tga;
mod tiff;
mod webp;

/// Untrusted-decode pixel budget (the pack's soft limit).
pub const DECODE_PIXELS_MAX: u64 = 268_435_456;
const SIDE_MAX: u32 = 1_048_576;

/// Source information the current RGBA8 import cannot retain.
pub const KNOWN_IMPORT_LOSSES: [&str; 9] = [
    "depth-16-reduced-to-8",
    "icc-profile-ignored",
    "exif-metadata-ignored",
    "animation-first-frame-only",
    "palette-indices-expanded",
    "gif-metadata-ignored",
    "tiff-metadata-ignored",
    "tga-metadata-ignored",
    "webp-metadata-ignored",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Bmp,
    Jpeg,
    Gif,
    Tiff,
    Tga,
    WebP,
}

impl ImageFormat {
    pub fn sniff(bytes: &[u8]) -> Option<ImageFormat> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(ImageFormat::Png)
        } else if bytes.starts_with(b"BM") {
            Some(ImageFormat::Bmp)
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(ImageFormat::Jpeg)
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            Some(ImageFormat::Gif)
        } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
            Some(ImageFormat::Tiff)
        } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            Some(ImageFormat::WebP)
        } else if tga::sniff(bytes) {
            Some(ImageFormat::Tga)
        } else {
            None
        }
    }

    /// By file extension, case-insensitive.
    pub fn from_extension(path: &std::path::Path) -> Option<ImageFormat> {
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "png" => Some(ImageFormat::Png),
            "bmp" => Some(ImageFormat::Bmp),
            "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
            "gif" => Some(ImageFormat::Gif),
            "tif" | "tiff" => Some(ImageFormat::Tiff),
            "tga" => Some(ImageFormat::Tga),
            "webp" => Some(ImageFormat::WebP),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ImageFormat::Png => "PNG",
            ImageFormat::Bmp => "BMP",
            ImageFormat::Jpeg => "JPEG",
            ImageFormat::Gif => "GIF",
            ImageFormat::Tiff => "TIFF",
            ImageFormat::Tga => "TGA",
            ImageFormat::WebP => "WebP",
        }
    }

    /// Precise losses for actual straight RGBA8 export samples.
    pub fn export_losses_for_rgba(self, rgba: &[u8]) -> Vec<&'static str> {
        if self != Self::Gif {
            return self.export_losses(rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255));
        }
        let mut losses = vec!["palette-quantized"];
        if rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[3] > 0 && p[3] < 255)
        {
            losses.push("alpha-binarized");
        }
        if rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[3] == 0 && p[..3] != [0, 0, 0])
        {
            losses.push("transparent-rgb-discarded");
        }
        losses
    }

    /// Conservative losses when only transparency presence is known.
    /// Use `export_losses_for_rgba` for precise GIF alpha loss reporting.
    pub fn export_losses(self, has_transparency: bool) -> Vec<&'static str> {
        match self {
            ImageFormat::Png
            | ImageFormat::Bmp
            | ImageFormat::Tiff
            | ImageFormat::Tga
            | ImageFormat::WebP => Vec::new(),
            ImageFormat::Gif => {
                let mut losses = vec!["palette-quantized"];
                if has_transparency {
                    losses.extend(["alpha-binarized", "transparent-rgb-discarded"]);
                }
                losses
            }
            ImageFormat::Jpeg => {
                let mut v = vec!["lossy-jpeg"];
                if has_transparency {
                    v.push("alpha-discarded");
                }
                v
            }
        }
    }
}

#[derive(Debug)]
pub struct Decoded {
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    /// Straight RGBA8, row-major.
    pub rgba: Vec<u8>,
    /// Things the source had that this import could not keep.
    pub losses: Vec<String>,
}

fn codec(m: impl Into<String>) -> Error {
    Error::new(ErrorCode::CodecFailed, m)
}

fn unsupported(m: impl Into<String>) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, m)
}

fn check_size(w: u64, h: u64) -> Result<()> {
    if w == 0 || h == 0 || w > u64::from(SIDE_MAX) || h > u64::from(SIDE_MAX) {
        return Err(codec("image dimensions out of range"));
    }
    if w * h > DECODE_PIXELS_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "image exceeds the decode budget",
        ));
    }
    Ok(())
}

fn image_error(error: ::image::ImageError) -> Error {
    match error {
        ::image::ImageError::Limits(_) => Error::new(ErrorCode::ResourceLimit, error.to_string()),
        ::image::ImageError::Unsupported(_) => unsupported(error.to_string()),
        _ => codec(error.to_string()),
    }
}

fn decode_limits(pixels: u64) -> ::image::Limits {
    let mut limits = ::image::Limits::default();
    limits.max_image_width = Some(SIDE_MAX);
    limits.max_image_height = Some(SIDE_MAX);
    limits.max_alloc = Some(pixels.saturating_mul(8));
    limits
}

pub fn decode(bytes: &[u8]) -> Result<Decoded> {
    decode_with_pixel_limit(bytes, DECODE_PIXELS_MAX)
}

/// Constrained hosts can lower decoded admission before pixel buffers are allocated.
pub fn decode_with_pixel_limit(bytes: &[u8], pixels: u64) -> Result<Decoded> {
    let pixels = pixels.min(DECODE_PIXELS_MAX);
    let format =
        ImageFormat::sniff(bytes).ok_or_else(|| codec("unrecognized raster image signature"))?;
    if bytes.len() as u64 > 1 << 30 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "encoded image exceeds the input budget",
        ));
    }
    if format == ImageFormat::Png {
        let decoder = ::image::codecs::png::PngDecoder::with_limits(
            Cursor::new(bytes),
            decode_limits(pixels),
        )
        .map_err(image_error)?;
        let losses = if decoder.is_apng().map_err(image_error)? {
            vec!["animation-first-frame-only".to_owned()]
        } else {
            Vec::new()
        };
        return decode_pixels(decoder, format, losses, pixels);
    }
    let losses = match format {
        ImageFormat::Gif => gif::admit(bytes, pixels)?,
        ImageFormat::Tiff => tiff::admit(bytes, pixels)?,
        ImageFormat::Tga => tga::admit(bytes, pixels)?,
        ImageFormat::WebP => webp::admit(bytes, pixels)?,
        _ => Vec::new(),
    };
    if format == ImageFormat::Bmp {
        admit_bmp(bytes)?;
    }
    let mut reader = ::image::ImageReader::with_format(Cursor::new(bytes), external_format(format));
    reader.limits(decode_limits(pixels));
    decode_pixels(
        reader.into_decoder().map_err(image_error)?,
        format,
        losses,
        pixels,
    )
}

fn decode_pixels(
    decoder: impl ImageDecoder,
    format: ImageFormat,
    mut losses: Vec<String>,
    pixels: u64,
) -> Result<Decoded> {
    let mut decoder = decoder;
    let (width, height) = decoder.dimensions();
    check_size(u64::from(width), u64::from(height))?;
    if u64::from(width) * u64::from(height) > pixels
        || decoder.total_bytes() > pixels.saturating_mul(8)
    {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "decoded image exceeds the byte budget",
        ));
    }
    let depth16 = matches!(
        decoder.color_type(),
        ::image::ColorType::L16
            | ::image::ColorType::La16
            | ::image::ColorType::Rgb16
            | ::image::ColorType::Rgba16
    );
    if depth16 {
        losses.insert(0, "depth-16-reduced-to-8".to_owned());
    }
    if decoder.icc_profile().map_err(image_error)?.is_some() {
        losses.push("icc-profile-ignored".to_owned());
    }
    if decoder.exif_metadata().map_err(image_error)?.is_some() {
        losses.push("exif-metadata-ignored".to_owned());
    }
    let image = DynamicImage::from_decoder(decoder).map_err(image_error)?;
    // Preserve the established STRIP_16 policy: retain high bytes rather than
    // silently adopting a different 16-to-8 rounding rule from a generic conversion.
    let rgba = if depth16 {
        image
            .into_rgba16()
            .into_raw()
            .into_iter()
            .map(|v| (v >> 8) as u8)
            .collect()
    } else {
        image.into_rgba8().into_raw()
    };
    Ok(Decoded {
        format,
        width,
        height,
        rgba,
        losses,
    })
}

fn external_format(format: ImageFormat) -> ::image::ImageFormat {
    match format {
        ImageFormat::Png => ::image::ImageFormat::Png,
        ImageFormat::Bmp => ::image::ImageFormat::Bmp,
        ImageFormat::Jpeg => ::image::ImageFormat::Jpeg,
        ImageFormat::Gif => ::image::ImageFormat::Gif,
        ImageFormat::Tiff => ::image::ImageFormat::Tiff,
        ImageFormat::Tga => ::image::ImageFormat::Tga,
        ImageFormat::WebP => ::image::ImageFormat::WebP,
    }
}

pub fn encode(
    format: ImageFormat,
    width: u32,
    height: u32,
    rgba: &[u8],
    jpeg_quality: u8,
) -> Result<Vec<u8>> {
    if rgba.len() as u64 != u64::from(width) * u64::from(height) * 4 {
        return Err(Error::new(
            ErrorCode::InvariantViolation,
            "pixel buffer does not match the image size",
        ));
    }
    check_size(u64::from(width), u64::from(height))?;
    let mut out = Vec::new();
    match format {
        ImageFormat::Gif => return gif::encode(width, height, rgba),
        ImageFormat::Tiff => return tiff::encode(width, height, rgba),
        ImageFormat::Tga => return tga::encode(width, height, rgba),
        ImageFormat::WebP => return webp::encode(width, height, rgba),
        ImageFormat::Png => ::image::codecs::png::PngEncoder::new(&mut out)
            .write_image(rgba, width, height, ::image::ExtendedColorType::Rgba8)
            .map_err(image_error)?,
        ImageFormat::Bmp => ::image::codecs::bmp::BmpEncoder::new(&mut out)
            .encode(rgba, width, height, ::image::ExtendedColorType::Rgba8)
            .map_err(image_error)?,
        ImageFormat::Jpeg => {
            if width > u32::from(u16::MAX) || height > u32::from(u16::MAX) {
                return Err(unsupported("JPEG supports at most 65535 pixels per side"));
            }
            let rgb: Vec<u8> = rgba
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|px| {
                    let a = u32::from(px[3]);
                    [px[0], px[1], px[2]]
                        .map(|c| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8)
                })
                .collect();
            ::image::codecs::jpeg::JpegEncoder::new_with_quality(
                &mut out,
                jpeg_quality.clamp(1, 100),
            )
            .encode(&rgb, width, height, ::image::ExtendedColorType::Rgb8)
            .map_err(image_error)?;
        }
    }
    Ok(out)
}

// Image handles pixel decoding; PaintCraft retains strict admission for its
// declared BMP subset. A valid byte representation alone does not admit a file.
use zerocopy::byteorder::{I32, LE, U16, U32};
use zerocopy::{FromBytes, Immutable, KnownLayout, Unaligned};
#[derive(Debug, FromBytes, Immutable, KnownLayout, Unaligned)]
#[repr(C)]
struct BmpHeader {
    signature: [u8; 2],
    file_size: U32<LE>,
    reserved: [u8; 4],
    offset: U32<LE>,
    dib: U32<LE>,
    width: I32<LE>,
    height: I32<LE>,
    planes: U16<LE>,
    bpp: U16<LE>,
    compression: U32<LE>,
    image_size: U32<LE>,
    x_resolution: I32<LE>,
    y_resolution: I32<LE>,
    palette_count: U32<LE>,
    important: U32<LE>,
}
fn le32(bytes: &[u8], offset: usize) -> Result<u32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| codec("truncated BMP header"))?;
    Ok(u32::from_le_bytes(
        value.try_into().expect("four checked bytes"),
    ))
}
fn admit_bmp(bytes: &[u8]) -> Result<()> {
    let (header, _) =
        BmpHeader::read_from_prefix(bytes).map_err(|_| codec("truncated BMP header"))?;
    let (dib, w, h, bpp, compression) = (
        header.dib.get(),
        header.width.get(),
        header.height.get(),
        header.bpp.get(),
        header.compression.get(),
    );
    if !matches!(dib, 40 | 108 | 124) {
        return Err(unsupported("unsupported BMP header version"));
    }
    if header.planes.get() != 1 || w <= 0 || h == 0 || h == i32::MIN {
        return Err(codec("invalid BMP dimensions"));
    }
    let (width, height) = (w as u32, h.unsigned_abs());
    check_size(u64::from(width), u64::from(height))?;
    if !matches!(bpp, 1 | 4 | 8 | 24 | 32) {
        return Err(unsupported("unsupported BMP bit depth"));
    }
    let masks_size = match compression {
        0 => 0,
        3 if bpp == 32 => {
            let masks = (le32(bytes, 54)?, le32(bytes, 58)?, le32(bytes, 62)?);
            let alpha = if dib >= 108 { le32(bytes, 66)? } else { 0 };
            if masks != (0x00ff0000, 0x0000ff00, 0x000000ff) || !matches!(alpha, 0 | 0xff000000) {
                return Err(unsupported("unsupported BMP channel masks"));
            }
            if dib == 40 { 12 } else { 0 }
        }
        _ => return Err(unsupported("unsupported BMP compression")),
    };
    let stride = (u64::from(width) * u64::from(bpp)).div_ceil(32) * 4;
    let offset = u64::from(header.offset.get());
    let palette = if bpp <= 8 {
        let count = header.palette_count.get();
        if count == 0 { 1u32 << bpp } else { count }
    } else {
        0
    };
    if palette > 1u32 << bpp.min(8) {
        return Err(codec("BMP palette exceeds its domain"));
    }
    let metadata_end = 14 + u64::from(dib) + masks_size + u64::from(palette) * 4;
    if offset < metadata_end
        || offset
            .checked_add(stride * u64::from(height))
            .is_none_or(|end| end > bytes.len() as u64)
    {
        return Err(codec("BMP pixel data overlaps metadata or is truncated"));
    }
    // image fills undeclared palette entries with black; our strict import
    // contract rejects references outside the declared palette instead.
    if bpp <= 8 {
        for row in bytes[offset as usize..][..(stride * u64::from(height)) as usize]
            .chunks_exact(stride as usize)
        {
            for x in 0..width as usize {
                let index = match bpp {
                    8 => u32::from(row[x]),
                    4 => u32::from((row[x / 2] >> (if x % 2 == 0 { 4 } else { 0 })) & 15),
                    _ => u32::from((row[x / 8] >> (7 - x % 8)) & 1),
                };
                if index >= palette {
                    return Err(codec("BMP palette index out of range"));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
