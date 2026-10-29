//! Scoped raster conversion with bounded codec dispatch and explicit loss reporting.
//!
//! Decoders read only a declared subset, check every dimension against the
//! decode budget before allocating, and say what they could not keep. Nothing
//! is converted silently: a 16-bit PNG decodes with a recorded depth reduction,
//! an unsupported BMP variant is refused, and JPEG output (lossy, no alpha) is
//! reported as a loss the caller must acknowledge.

use ::image::{DynamicImage, ImageDecoder, ImageEncoder};
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::io::Cursor;

mod dib;
pub use dib::{DibKind, decode_dib_with_pixel_limit};
mod extended;
mod specialist;
pub use extended::ENCODE_PIXELS_MAX as EXTENDED_ENCODE_PIXELS_MAX;
pub(crate) use extended::inspect_pcx;
pub mod bmp_metadata;
pub mod bmp_retention;
mod bmp_rle;
mod gif;
mod identity;
pub use identity::{JPEG_IDENTITY_PROFILE, TIFF_IDENTITY_PROFILE, document_conversion_key};
pub mod jpeg;
mod jpeg_encode;
pub use jpeg_encode::{JpegMatte, encode_checked as encode_jpeg_checked};
pub(crate) mod orientation;
mod png_indexed;
pub mod png_metadata;
pub mod png_retention;
pub mod raster_source_metadata;
#[cfg(test)]
mod raster_tests;
mod retained_raster;
mod tga;
pub(crate) mod tiff;
mod webp;

/// Untrusted-decode pixel budget (the pack's soft limit).
pub const DECODE_PIXELS_MAX: u64 = 268_435_456;
const SIDE_MAX: u32 = 1_048_576;

/// Required conversion acknowledgements: actual omissions and retained source
/// facts that do not activate working color/print interpretation.
pub const KNOWN_IMPORT_LOSSES: [&str; 27] = [
    png_retention::ICC_RETAINED_UNAPPLIED,
    png_retention::RESOLUTION_RETAINED_INACTIVE,
    "dib-alpha-unspecified",
    "dib-alpha-mode-unspecified",
    "dib-metadata-ignored",
    "depth-16-reduced-to-8",
    "icc-profile-ignored",
    "exif-metadata-ignored",
    "png-color-metadata-ignored",
    "png-metadata-ignored",
    "bmp-color-metadata-ignored",
    "bmp-metadata-ignored",
    "resolution-metadata-ignored",
    "jpeg-metadata-ignored",
    "jpeg-cmyk-to-rgb",
    "jpeg-color-model-assumed",
    "animation-first-frame-only",
    "palette-indices-expanded",
    "gif-metadata-ignored",
    "tiff-metadata-ignored",
    "tga-metadata-ignored",
    "webp-metadata-ignored",
    "pnm-metadata-ignored",
    "pcx-metadata-ignored",
    "msp-metadata-ignored",
    "dcx-other-pages-not-imported",
    crate::dds::OTHER_MIPS_LOSS,
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
    Qoi,
    Pnm,
    Farbfeld,
    Pcx,
    Msp,
    Jxl,
    Dds,
    Avif,
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
        } else if bytes.starts_with(b"qoif") {
            Some(ImageFormat::Qoi)
        } else if bytes.starts_with(b"DanM") || bytes.starts_with(b"LinS") {
            Some(ImageFormat::Msp)
        } else if bytes.starts_with(b"farbfeld") {
            Some(ImageFormat::Farbfeld)
        } else if bytes.len() >= 3
            && bytes[0] == b'P'
            && matches!(bytes[1], b'1'..=b'7')
            && bytes[2].is_ascii_whitespace()
        {
            Some(ImageFormat::Pnm)
        } else if bytes.len() >= 4
            && bytes[0] == 10
            && matches!(bytes[1], 0 | 2 | 3 | 4 | 5)
            && bytes[2] <= 1
            && matches!(bytes[3], 1 | 2 | 4 | 8)
        {
            Some(ImageFormat::Pcx)
        } else if crate::jxl::sniff(bytes) {
            Some(ImageFormat::Jxl)
        } else if crate::dds::sniff(bytes) {
            Some(ImageFormat::Dds)
        } else if specialist::sniff_avif(bytes) {
            Some(ImageFormat::Avif)
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
            "qoi" => Some(ImageFormat::Qoi),
            "pnm" | "pbm" | "pgm" | "ppm" | "pam" => Some(ImageFormat::Pnm),
            "ff" => Some(ImageFormat::Farbfeld),
            "pcx" => Some(ImageFormat::Pcx),
            "msp" => Some(ImageFormat::Msp),
            "jxl" => Some(ImageFormat::Jxl),
            "dds" => Some(ImageFormat::Dds),
            "avif" => Some(ImageFormat::Avif),
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
            ImageFormat::Qoi => "QOI",
            ImageFormat::Pnm => "PNM",
            ImageFormat::Farbfeld => "farbfeld",
            ImageFormat::Pcx => "PCX",
            ImageFormat::Msp => "MSP",
            ImageFormat::Jxl => "JXL",
            ImageFormat::Dds => "DDS",
            ImageFormat::Avif => "AVIF",
        }
    }

    /// Precise losses for actual straight RGBA8 export samples.
    pub fn export_losses_for_rgba(self, rgba: &[u8]) -> Vec<&'static str> {
        if self == Self::Avif {
            return crate::avif::export_losses(
                rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255),
                rgba.as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[3] == 0 && p[..3] != [0, 0, 0]),
            );
        }
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
            | ImageFormat::WebP
            | ImageFormat::Qoi
            | ImageFormat::Pnm
            | ImageFormat::Farbfeld
            | ImageFormat::Jxl
            | ImageFormat::Dds
            | ImageFormat::Msp => Vec::new(),
            ImageFormat::Pcx => {
                if has_transparency {
                    vec!["alpha-discarded"]
                } else {
                    Vec::new()
                }
            }
            ImageFormat::Avif => crate::avif::export_losses(has_transparency, has_transparency),
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
    /// Required conversion acknowledgements, including actual omissions and
    /// source facts retained without activating color/print interpretations.
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

fn admit_buffer_bytes(bytes: u64, address_limit: u64) -> Result<()> {
    if bytes > address_limit {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "image buffer exceeds the target allocation address space",
        ));
    }
    Ok(())
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
    if format == ImageFormat::Dds {
        return specialist::decode_dds(bytes, pixels);
    }
    if matches!(format, ImageFormat::Jxl | ImageFormat::Avif) {
        return Err(unsupported(
            "this specialist raster decoder is not enabled in the image facade",
        ));
    }
    if format == ImageFormat::Qoi {
        return extended::decode_qoi(bytes, pixels);
    }
    if format == ImageFormat::Msp {
        let header = crate::msp::inspect(bytes, &|| Ok(()))?;
        if u64::from(header.width()) * u64::from(header.height()) > pixels {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "MSP exceeds caller pixel allowance",
            ));
        }
        let bitmap = crate::msp::decode(bytes, &|| Ok(()))?;
        return Ok(Decoded {
            format,
            width: header.width(),
            height: header.height(),
            rgba: bitmap.rgba(&|| Ok(()))?,
            losses: vec!["msp-metadata-ignored".into()],
        });
    }
    if format == ImageFormat::Pcx {
        return extended::decode_pcx(bytes, pixels);
    }
    if format == ImageFormat::Gif {
        return gif::decode(bytes, pixels);
    }
    if format == ImageFormat::Bmp && bmp_rle::is_rle(bytes) {
        return bmp_rle::decode(bytes, pixels, &|| Ok(()));
    }
    if format == ImageFormat::Jpeg {
        let converted = jpeg::decode(bytes, pixels, &|| Ok(()))?;
        return Ok(Decoded {
            format,
            width: converted.width,
            height: converted.height,
            rgba: converted.rgba,
            losses: converted.losses,
        });
    }
    if format == ImageFormat::Tiff {
        let (source, losses) = tiff::admit_source(bytes, pixels, &|| Ok(()))?;
        let mut reader =
            ::image::ImageReader::with_format(Cursor::new(bytes), ::image::ImageFormat::Tiff);
        reader.limits(decode_limits(pixels));
        let decoder = reader.into_decoder().map_err(image_error)?;
        if decoder.dimensions() != (source.width, source.height) {
            return Err(codec("TIFF decoder source geometry changed"));
        }
        let decoded = decode_pixels(decoder, format, losses, pixels)?;
        let (width, height, rgba) = orientation::normalize(
            source.width,
            source.height,
            source.orientation,
            decoded.rgba,
            &|| Ok(()),
        )?;
        return Ok(Decoded {
            format,
            width,
            height,
            rgba,
            losses: decoded.losses,
        });
    }
    if format == ImageFormat::Png {
        let source = png_metadata::inspect(bytes, pixels, &|| Ok(()))?;
        if source.color_type == 3 {
            return png_indexed::decode(bytes, &source, pixels, &|| Ok(()));
        }
        return png_retention::decode_legacy_pixels(bytes, &source, pixels, &|| Ok(()));
    }
    let mut losses = match format {
        ImageFormat::Tga => tga::admit(bytes, pixels)?,
        ImageFormat::WebP => webp::admit(bytes, pixels)?,
        ImageFormat::Pnm => extended::admit_pnm(bytes, pixels)?,
        ImageFormat::Farbfeld => extended::admit_farbfeld(bytes, pixels)?,
        _ => Vec::new(),
    };
    if format == ImageFormat::Bmp {
        admit_bmp(bytes, pixels)?;
        losses.extend(
            bmp_metadata::inspect(bytes, pixels, &|| Ok(()))?
                .losses()
                .into_iter()
                .map(str::to_owned),
        );
    }
    let mut reader =
        ::image::ImageReader::with_format(Cursor::new(bytes), external_format(format)?);
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
    losses: Vec<String>,
    pixels: u64,
) -> Result<Decoded> {
    decode_pixels_metadata(decoder, format, losses, pixels, true)
}

fn decode_pixels_metadata(
    decoder: impl ImageDecoder,
    format: ImageFormat,
    mut losses: Vec<String>,
    pixels: u64,
    read_metadata: bool,
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
    admit_buffer_bytes(decoder.total_bytes(), isize::MAX as u64)?;
    if depth16 {
        // into_rgba16 below expands even grayscale/RGB source buffers to eight
        // bytes per pixel. Vec allocations must also fit isize on wasm32.
        admit_buffer_bytes(u64::from(width) * u64::from(height) * 8, isize::MAX as u64)?;
    }
    if depth16 {
        losses.insert(0, "depth-16-reduced-to-8".to_owned());
    }
    if read_metadata
        && decoder.icc_profile().map_err(image_error)?.is_some()
        && !losses.iter().any(|loss| loss == "icc-profile-ignored")
    {
        losses.push("icc-profile-ignored".to_owned());
    }
    if read_metadata
        && decoder.exif_metadata().map_err(image_error)?.is_some()
        && !losses.iter().any(|loss| loss == "exif-metadata-ignored")
    {
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

/// The selected TIFF view retains original absolute file offsets. Its caller
/// has already inventoried the complete source and admitted this exact IFD.
pub(crate) fn decode_tiff_selected(
    reader: impl std::io::BufRead + std::io::Seek,
    pixels: u64,
    losses: Vec<String>,
) -> Result<Decoded> {
    let mut decoder = ::image::codecs::tiff::TiffDecoder::new(reader).map_err(image_error)?;
    decoder
        .set_limits(decode_limits(pixels))
        .map_err(image_error)?;
    // Complete preflight owns bounded original ICC/metadata omissions; the
    // optional facade metadata lookup is unnecessary for this selected view.
    decode_pixels_metadata(decoder, ImageFormat::Tiff, losses, pixels, false)
}

fn external_format(format: ImageFormat) -> Result<::image::ImageFormat> {
    Ok(match format {
        ImageFormat::Png => ::image::ImageFormat::Png,
        ImageFormat::Bmp => ::image::ImageFormat::Bmp,
        ImageFormat::Jpeg => ::image::ImageFormat::Jpeg,
        ImageFormat::Gif => ::image::ImageFormat::Gif,
        ImageFormat::Tiff => ::image::ImageFormat::Tiff,
        ImageFormat::Tga => ::image::ImageFormat::Tga,
        ImageFormat::WebP => ::image::ImageFormat::WebP,
        ImageFormat::Qoi => ::image::ImageFormat::Qoi,
        ImageFormat::Pnm => ::image::ImageFormat::Pnm,
        ImageFormat::Farbfeld => ::image::ImageFormat::Farbfeld,
        ImageFormat::Jxl | ImageFormat::Dds | ImageFormat::Avif => {
            return Err(unsupported(
                "specialist raster requires its selected codec route",
            ));
        }
        ImageFormat::Pcx | ImageFormat::Msp => {
            return Err(unsupported(
                "legacy raster requires its selected dedicated codec",
            ));
        }
    })
}

pub fn encode(
    format: ImageFormat,
    width: u32,
    height: u32,
    rgba: &[u8],
    jpeg_quality: u8,
) -> Result<Vec<u8>> {
    encode_checked(format, width, height, rgba, jpeg_quality, &|| Ok(()))
}

/// Encode privately, checking cancellation around all codecs and at the selected
/// specialist adapters' checkpoints. Opaque compression loops remain non-preemptible.
pub fn encode_checked(
    format: ImageFormat,
    width: u32,
    height: u32,
    rgba: &[u8],
    quality: u8,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    let bytes = encode_inner(format, width, height, rgba, quality, check)?;
    check()?;
    Ok(bytes)
}
fn encode_inner(
    format: ImageFormat,
    width: u32,
    height: u32,
    rgba: &[u8],
    jpeg_quality: u8,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check_size(u64::from(width), u64::from(height))?;
    if rgba.len() as u64 != u64::from(width) * u64::from(height) * 4 {
        return Err(Error::new(
            ErrorCode::InvariantViolation,
            "pixel buffer does not match the image size",
        ));
    }
    let mut out = Vec::new();
    match format {
        ImageFormat::Jxl | ImageFormat::Dds | ImageFormat::Avif => {
            return specialist::encode(format, width, height, rgba, jpeg_quality, check);
        }
        ImageFormat::Qoi | ImageFormat::Pnm | ImageFormat::Farbfeld => {
            return extended::encode(format, width, height, rgba, check);
        }
        ImageFormat::Pcx => return crate::pcx::encode_rgba(width, height, rgba, check),
        ImageFormat::Msp => return crate::msp::encode_v1_rgba_exact(width, height, rgba, check),
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
            return jpeg_encode::encode_checked(
                width,
                height,
                rgba,
                jpeg_quality,
                JpegMatte::WHITE,
                check,
            );
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
fn admit_bmp(bytes: &[u8], pixels: u64) -> Result<()> {
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
    if u64::from(width) * u64::from(height) > pixels {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "BMP exceeds the caller pixel allowance",
        ));
    }
    if !matches!(bpp, 1 | 4 | 8 | 16 | 24 | 32) {
        return Err(unsupported("unsupported BMP bit depth"));
    }
    let masks_size = match compression {
        0 => 0,
        3 if matches!(bpp, 16 | 32) => {
            let masks = (le32(bytes, 54)?, le32(bytes, 58)?, le32(bytes, 62)?);
            let alpha = if dib >= 108 { le32(bytes, 66)? } else { 0 };
            let supported = match bpp {
                16 => {
                    matches!(masks, (0x7c00, 0x03e0, 0x001f) | (0xf800, 0x07e0, 0x001f))
                        && alpha == 0
                }
                _ => {
                    masks == (0x00ff0000, 0x0000ff00, 0x000000ff) && matches!(alpha, 0 | 0xff000000)
                }
            };
            if !supported {
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
