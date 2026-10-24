//! Flat raster images: PNG, BMP and JPEG, in and out, with explicit loss reporting.
//!
//! Decoders read only a declared subset, check every dimension against the
//! decode budget before allocating, and say what they could not keep. Nothing
//! is converted silently: a 16-bit PNG decodes with a recorded depth reduction,
//! an unsupported BMP variant is refused, and JPEG output (lossy, no alpha) is
//! reported as a loss the caller must acknowledge.

use ::image::{DynamicImage, ImageDecoder, ImageEncoder};
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::io::Cursor;

/// Untrusted-decode pixel budget (the pack's soft limit).
pub const DECODE_PIXELS_MAX: u64 = 268_435_456;
const SIDE_MAX: u32 = 1_048_576;

/// Source information the current RGBA8 import cannot retain.
pub const KNOWN_IMPORT_LOSSES: [&str; 4] = [
    "depth-16-reduced-to-8",
    "icc-profile-ignored",
    "exif-metadata-ignored",
    "animation-first-frame-only",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Bmp,
    Jpeg,
}

impl ImageFormat {
    pub fn sniff(bytes: &[u8]) -> Option<ImageFormat> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(ImageFormat::Png)
        } else if bytes.starts_with(b"BM") {
            Some(ImageFormat::Bmp)
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(ImageFormat::Jpeg)
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
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ImageFormat::Png => "PNG",
            ImageFormat::Bmp => "BMP",
            ImageFormat::Jpeg => "JPEG",
        }
    }

    /// Loss codes that exporting to this format incurs for an image with or
    /// without any non-opaque pixel.
    pub fn export_losses(self, has_transparency: bool) -> Vec<&'static str> {
        match self {
            ImageFormat::Png | ImageFormat::Bmp => Vec::new(),
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
    let format = ImageFormat::sniff(bytes).ok_or_else(|| codec("not a PNG, BMP or JPEG image"))?;
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
    if format == ImageFormat::Bmp {
        admit_bmp(bytes)?;
    }
    let mut reader = ::image::ImageReader::with_format(Cursor::new(bytes), external_format(format));
    reader.limits(decode_limits(pixels));
    decode_pixels(
        reader.into_decoder().map_err(image_error)?,
        format,
        Vec::new(),
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
mod tests {
    use super::*;

    fn sample(w: u32, h: u32) -> Vec<u8> {
        (0..w * h)
            .flat_map(|i| {
                [
                    (i * 7 % 256) as u8,
                    (i * 13 % 256) as u8,
                    (i * 29 % 256) as u8,
                    if i % 5 == 0 {
                        0
                    } else {
                        255 - (i % 3) as u8 * 40
                    },
                ]
            })
            .collect()
    }

    #[test]
    fn png_and_bmp_round_trip_exactly_including_alpha() {
        let px = sample(37, 19);
        for f in [ImageFormat::Png, ImageFormat::Bmp] {
            let bytes = encode(f, 37, 19, &px, 90).unwrap();
            assert_eq!(ImageFormat::sniff(&bytes), Some(f));
            let d = decode(&bytes).unwrap();
            assert_eq!((d.format, d.width, d.height), (f, 37, 19));
            // Fully transparent pixels keep alpha 0; PNG/BMP keep their RGB too.
            assert_eq!(d.rgba, px, "{}", f.name());
            assert!(d.losses.is_empty());
        }
    }

    #[test]
    fn jpeg_is_lossy_composites_over_white_and_reports_it() {
        let flat: Vec<u8> = (0..64 * 64).flat_map(|_| [200u8, 40, 40, 255]).collect();
        let d = decode(&encode(ImageFormat::Jpeg, 64, 64, &flat, 95).unwrap()).unwrap();
        assert_eq!((d.width, d.height), (64, 64));
        for px in d.rgba.as_chunks::<4>().0 {
            assert!(
                px[3] == 255
                    && px[0].abs_diff(200) < 6
                    && px[1].abs_diff(40) < 6
                    && px[2].abs_diff(40) < 6,
                "{px:?}"
            );
        }
        let clear = vec![0u8; 16 * 16 * 4];
        let d = decode(&encode(ImageFormat::Jpeg, 16, 16, &clear, 90).unwrap()).unwrap();
        assert!(
            d.rgba
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[0] > 250 && p[1] > 250 && p[2] > 250),
            "transparent becomes white"
        );
        assert_eq!(
            ImageFormat::Jpeg.export_losses(true),
            ["lossy-jpeg", "alpha-discarded"]
        );
        assert!(ImageFormat::Png.export_losses(true).is_empty());
        assert!(encode(ImageFormat::Jpeg, 70_000, 1, &vec![0; 70_000 * 4], 90).is_err());
    }

    #[test]
    fn png_variants_expand_and_report_what_they_lose() {
        let enc = |color: png::ColorType,
                   depth: png::BitDepth,
                   data: &[u8],
                   plte: Option<&[u8]>,
                   trns: Option<&[u8]>| {
            let mut out = Vec::new();
            let mut e = png::Encoder::new(&mut out, 2, 1);
            e.set_color(color);
            e.set_depth(depth);
            if let Some(p) = plte {
                e.set_palette(p.to_vec());
            }
            if let Some(t) = trns {
                e.set_trns(t.to_vec());
            }
            e.write_header().unwrap().write_image_data(data).unwrap();
            out
        };
        let gray = decode(&enc(
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            &[10, 200],
            None,
            None,
        ))
        .unwrap();
        assert_eq!(gray.rgba, [10, 10, 10, 255, 200, 200, 200, 255]);
        let ga = decode(&enc(
            png::ColorType::GrayscaleAlpha,
            png::BitDepth::Eight,
            &[10, 20, 200, 255],
            None,
            None,
        ))
        .unwrap();
        assert_eq!(ga.rgba, [10, 10, 10, 20, 200, 200, 200, 255]);
        let pal = decode(&enc(
            png::ColorType::Indexed,
            png::BitDepth::Eight,
            &[1, 0],
            Some(&[1, 2, 3, 4, 5, 6]),
            Some(&[128, 255]),
        ))
        .unwrap();
        assert_eq!(pal.rgba, [4, 5, 6, 255, 1, 2, 3, 128]);
        let deep = decode(&enc(
            png::ColorType::Rgb,
            png::BitDepth::Sixteen,
            &[
                0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44,
            ],
            None,
            None,
        ))
        .unwrap();
        assert_eq!(deep.rgba[..4], [0x12, 0x56, 0x9a, 255]);
        assert_eq!(deep.losses, ["depth-16-reduced-to-8"]);
    }

    fn bmp(
        bpp: u16,
        w: i32,
        h: i32,
        compression: u32,
        palette: &[[u8; 3]],
        pixels: &[u8],
    ) -> Vec<u8> {
        let off = 14 + 40 + palette.len() * 4;
        let mut v = b"BM".to_vec();
        v.extend_from_slice(&((off + pixels.len()) as u32).to_le_bytes());
        v.extend_from_slice(&[0; 4]);
        v.extend_from_slice(&(off as u32).to_le_bytes());
        v.extend_from_slice(&40u32.to_le_bytes());
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&bpp.to_le_bytes());
        v.extend_from_slice(&compression.to_le_bytes());
        v.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
        v.extend_from_slice(&[0; 8]);
        v.extend_from_slice(&(palette.len() as u32).to_le_bytes());
        v.extend_from_slice(&[0; 4]);
        for c in palette {
            v.extend_from_slice(&[c[2], c[1], c[0], 0]);
        }
        v.extend_from_slice(pixels);
        v
    }

    #[test]
    fn bmp_variants_decode_with_correct_orientation_padding_and_palettes() {
        // 3x2, 24-bit, bottom-up, rows padded to 12 bytes. Bottom row first: blue, green, red / white, black, grey.
        let rows = [
            [0, 0, 255, 0, 255, 0, 255, 0, 0, 0, 0, 0],
            [255, 255, 255, 0, 0, 0, 128, 128, 128, 0, 0, 0],
        ];
        let up = decode(&bmp(24, 3, 2, 0, &[], &rows.concat())).unwrap();
        assert_eq!(
            up.rgba[..12],
            [255, 255, 255, 255, 0, 0, 0, 255, 128, 128, 128, 255],
            "top row is the last stored"
        );
        let down = decode(&bmp(24, 3, -2, 0, &[], &rows.concat())).unwrap();
        assert_eq!(
            down.rgba[..4],
            [255, 0, 0, 255],
            "top-down keeps stored order (BGR to RGB)"
        );
        // 4-bit palette: two pixels per byte, high nibble first, rows padded to 4 bytes.
        let p4 = decode(&bmp(
            4,
            3,
            1,
            0,
            &[[1, 1, 1], [2, 2, 2], [3, 3, 3]],
            &[0x01, 0x20, 0, 0],
        ))
        .unwrap();
        assert_eq!(p4.rgba, [1, 1, 1, 255, 2, 2, 2, 255, 3, 3, 3, 255]);
        // 1-bit palette, most significant bit first.
        let p1 = decode(&bmp(
            1,
            3,
            1,
            0,
            &[[0, 0, 0], [9, 9, 9]],
            &[0b0100_0000, 0, 0, 0],
        ))
        .unwrap();
        assert_eq!(p1.rgba, [0, 0, 0, 255, 9, 9, 9, 255, 0, 0, 0, 255]);
    }

    #[test]
    fn hostile_images_are_refused_without_large_allocations() {
        let good_bmp = bmp(24, 1, 1, 0, &[], &[1, 2, 3, 0]);
        let mut cases: Vec<(Vec<u8>, ErrorCode)> = vec![
            (b"GIF89a".to_vec(), ErrorCode::CodecFailed),
            (vec![], ErrorCode::CodecFailed),
            (
                good_bmp[..good_bmp.len() - 1].to_vec(),
                ErrorCode::CodecFailed,
            ),
            (
                bmp(24, 1, 1, 1, &[], &[0; 4]),
                ErrorCode::CapabilityUnsupported,
            ),
            (
                bmp(16, 1, 1, 0, &[], &[0; 4]),
                ErrorCode::CapabilityUnsupported,
            ),
            (bmp(24, 0, 1, 0, &[], &[]), ErrorCode::CodecFailed),
            (bmp(24, 1, i32::MIN, 0, &[], &[]), ErrorCode::CodecFailed),
            (bmp(24, 2_000_000, 1, 0, &[], &[]), ErrorCode::CodecFailed),
            (
                bmp(32, 1_000_000, 1_000_000, 0, &[], &[]),
                ErrorCode::ResourceLimit,
            ),
            (
                bmp(8, 1, 1, 0, &[[0, 0, 0]], &[5, 0, 0, 0]),
                ErrorCode::CodecFailed,
            ),
        ];
        let png = encode(ImageFormat::Png, 2, 2, &[0; 16], 90).unwrap();
        cases.push((png[..png.len() / 2].to_vec(), ErrorCode::CodecFailed));
        let mut huge = png.clone();
        huge[16..20].copy_from_slice(&0x7FFF_FFFFu32.to_be_bytes()); // width in IHDR; CRC now wrong
        cases.push((huge, ErrorCode::CodecFailed));
        cases.push((
            vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0],
            ErrorCode::CodecFailed,
        ));
        for (bytes, code) in cases {
            assert_eq!(
                decode(&bytes).unwrap_err().code,
                code,
                "{:02x?}",
                &bytes[..bytes.len().min(8)]
            );
        }
    }
    #[test]
    fn depth_reduction_keeps_high_bytes_and_header_views_are_exact() {
        assert_eq!(std::mem::size_of::<BmpHeader>(), 54);
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Sixteen);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[
                0, 255, 1, 0, 127, 255, 255, 255, 255, 0, 128, 0, 1, 255, 0, 0,
            ])
            .unwrap();
        let result = decode(&bytes).unwrap();
        assert_eq!(result.rgba, [0, 1, 127, 255, 255, 128, 1, 0]);
        assert_eq!(result.losses, ["depth-16-reduced-to-8"]);
    }

    #[test]
    fn jpeg_discarded_metadata_is_reported() {
        let mut bytes = Vec::new();
        let mut encoder = ::image::codecs::jpeg::JpegEncoder::new(&mut bytes);
        encoder.set_icc_profile(vec![1, 2, 3, 4]).unwrap();
        encoder
            .set_exif_metadata(b"II*\0\x08\0\0\0\0\0\0\0\0\0".to_vec())
            .unwrap();
        encoder
            .encode(&[10, 20, 30], 1, 1, ::image::ExtendedColorType::Rgb8)
            .unwrap();
        let result = decode(&bytes).unwrap();
        assert_eq!(
            result.losses,
            ["icc-profile-ignored", "exif-metadata-ignored"]
        );
    }

    #[test]
    fn bmp_pixel_offset_cannot_overlap_its_header() {
        let mut bytes = bmp(24, 1, 1, 0, &[], &[1, 2, 3, 0]);
        bytes[10..14].copy_from_slice(&14u32.to_le_bytes());
        assert_eq!(decode(&bytes).unwrap_err().code, ErrorCode::CodecFailed);
    }
}
