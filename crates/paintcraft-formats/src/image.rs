//! Flat raster images: PNG, BMP and JPEG, in and out, with explicit loss reporting.
//!
//! Decoders read only a declared subset, check every dimension against the
//! decode budget before allocating, and say what they could not keep. Nothing
//! is converted silently: a 16-bit PNG decodes with a recorded depth reduction,
//! an unsupported BMP variant is refused, and JPEG output (lossy, no alpha) is
//! reported as a loss the caller must acknowledge.

use paintcraft_types::{Error, ErrorCode, Result};

/// Untrusted-decode pixel budget (the pack's soft limit).
pub const DECODE_PIXELS_MAX: u64 = 268_435_456;
const SIDE_MAX: u32 = 1_048_576;

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

pub fn decode(bytes: &[u8]) -> Result<Decoded> {
    match ImageFormat::sniff(bytes) {
        Some(ImageFormat::Png) => decode_png(bytes),
        Some(ImageFormat::Bmp) => decode_bmp(bytes),
        Some(ImageFormat::Jpeg) => decode_jpeg(bytes),
        None => Err(codec("not a PNG, BMP or JPEG image")),
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
    match format {
        ImageFormat::Png => crate::png::encode_rgba8(width, height, rgba),
        ImageFormat::Bmp => Ok(encode_bmp(width, height, rgba)),
        ImageFormat::Jpeg => encode_jpeg(width, height, rgba, jpeg_quality),
    }
}

// ---------------------------------------------------------------- PNG

fn decode_png(bytes: &[u8]) -> Result<Decoded> {
    let fail = |e: png::DecodingError| codec(format!("PNG: {e}"));
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(fail)?;
    let (width, height) = {
        let info = reader.info();
        (info.width, info.height)
    };
    check_size(u64::from(width), u64::from(height))?;
    let mut losses = Vec::new();
    {
        let info = reader.info();
        if info.bit_depth == png::BitDepth::Sixteen {
            losses.push("depth-16-reduced-to-8".to_owned());
        }
        if info.icc_profile.is_some() {
            losses.push("icc-profile-ignored".to_owned());
        }
        if info.animation_control.is_some() {
            losses.push("animation-first-frame-only".to_owned());
        }
    }
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| codec("PNG output size overflows"))?;
    if size as u64 > DECODE_PIXELS_MAX * 8 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "image exceeds the decode budget",
        ));
    }
    let mut buf = vec![0u8; size];
    let frame = reader.next_frame(&mut buf).map_err(fail)?;
    let px = &buf[..frame.buffer_size()];
    let n = (width as usize) * (height as usize);
    let mut rgba = Vec::with_capacity(n * 4);
    match frame.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(px),
        png::ColorType::Rgb => px
            .as_chunks::<3>()
            .0
            .iter()
            .for_each(|c| rgba.extend_from_slice(&[c[0], c[1], c[2], 255])),
        png::ColorType::GrayscaleAlpha => px
            .as_chunks::<2>()
            .0
            .iter()
            .for_each(|c| rgba.extend_from_slice(&[c[0], c[0], c[0], c[1]])),
        png::ColorType::Grayscale => px
            .iter()
            .for_each(|g| rgba.extend_from_slice(&[*g, *g, *g, 255])),
        png::ColorType::Indexed => return Err(codec("palette was not expanded")),
    }
    if rgba.len() != n * 4 {
        return Err(codec("PNG pixel data has the wrong size"));
    }
    Ok(Decoded {
        format: ImageFormat::Png,
        width,
        height,
        rgba,
        losses,
    })
}

// ---------------------------------------------------------------- BMP

fn le16(b: &[u8], at: usize) -> Result<u16> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| codec("truncated BMP"))
}

fn le32(b: &[u8], at: usize) -> Result<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| codec("truncated BMP"))
}

fn decode_bmp(b: &[u8]) -> Result<Decoded> {
    let data_offset = le32(b, 10)? as usize;
    let dib = le32(b, 14)?;
    if !matches!(dib, 40 | 108 | 124) {
        return Err(unsupported(format!(
            "BMP header size {dib} is not supported (40, 108 or 124)"
        )));
    }
    let (w, h_raw) = (le32(b, 18)? as i32, le32(b, 22)? as i32);
    let (planes, bpp, compression) = (le16(b, 26)?, le16(b, 28)?, le32(b, 30)?);
    if planes != 1 || w <= 0 || h_raw == i32::MIN || h_raw == 0 {
        return Err(codec("invalid BMP dimensions"));
    }
    let (width, height, top_down) = (w as u32, h_raw.unsigned_abs(), h_raw < 0);
    check_size(u64::from(width), u64::from(height))?;
    if !matches!(bpp, 1 | 4 | 8 | 24 | 32) {
        return Err(unsupported(format!(
            "BMP with {bpp} bits per pixel is not supported"
        )));
    }
    // Masks: only the standard BGRA layout is understood for bit-field files.
    let mut alpha_mask = 0u32;
    match compression {
        0 => {}
        3 if bpp == 32 => {
            let (r, g, bl) = (le32(b, 54)?, le32(b, 58)?, le32(b, 62)?);
            alpha_mask = if dib >= 108 { le32(b, 66)? } else { 0 };
            if (r, g, bl) != (0x00FF_0000, 0x0000_FF00, 0x0000_00FF)
                || !(alpha_mask == 0 || alpha_mask == 0xFF00_0000)
            {
                return Err(unsupported(
                    "BMP bit-field masks other than BGRA8888 are not supported",
                ));
            }
        }
        _ => {
            return Err(unsupported(format!(
                "BMP compression {compression} is not supported"
            )));
        }
    }
    let stride = (u64::from(width) * u64::from(bpp)).div_ceil(32) * 4;
    let need = stride
        .checked_mul(u64::from(height))
        .ok_or_else(|| codec("BMP size overflows"))?;
    if (data_offset as u64)
        .checked_add(need)
        .is_none_or(|end| end > b.len() as u64)
    {
        return Err(codec("BMP pixel data is truncated"));
    }
    let mut palette = Vec::new();
    if bpp <= 8 {
        let used = le32(b, 46)? as usize;
        let count = if used == 0 {
            1usize << bpp
        } else {
            used.min(1 << bpp)
        };
        let start = 14 + dib as usize;
        for i in 0..count {
            let e = b
                .get(start + i * 4..start + i * 4 + 4)
                .ok_or_else(|| codec("truncated BMP palette"))?;
            palette.push([e[2], e[1], e[0], 255]);
        }
    }
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    for row in 0..height as usize {
        let src_row = if top_down {
            row
        } else {
            height as usize - 1 - row
        };
        let line = &b[data_offset + src_row * stride as usize..][..stride as usize];
        let dst = &mut rgba[row * width as usize * 4..][..width as usize * 4];
        for (x, out) in dst.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            *out = match bpp {
                24 => [line[x * 3 + 2], line[x * 3 + 1], line[x * 3], 255],
                32 => [
                    line[x * 4 + 2],
                    line[x * 4 + 1],
                    line[x * 4],
                    if alpha_mask != 0 {
                        line[x * 4 + 3]
                    } else {
                        255
                    },
                ],
                8 => *palette
                    .get(line[x] as usize)
                    .ok_or_else(|| codec("BMP palette index out of range"))?,
                4 => *palette
                    .get(((line[x / 2] >> (if x % 2 == 0 { 4 } else { 0 })) & 15) as usize)
                    .ok_or_else(|| codec("BMP palette index out of range"))?,
                _ => *palette
                    .get(((line[x / 8] >> (7 - x % 8)) & 1) as usize)
                    .ok_or_else(|| codec("BMP palette index out of range"))?,
            };
        }
    }
    Ok(Decoded {
        format: ImageFormat::Bmp,
        width,
        height,
        rgba,
        losses: Vec::new(),
    })
}

/// 32-bit BGRA with a V4 header and explicit masks, bottom-up.
fn encode_bmp(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let pixels = width as usize * height as usize * 4;
    let offset = 14 + 108;
    let mut out = Vec::with_capacity(offset + pixels);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((offset + pixels) as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(offset as u32).to_le_bytes());
    out.extend_from_slice(&108u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&3u32.to_le_bytes()); // BI_BITFIELDS
    out.extend_from_slice(&(pixels as u32).to_le_bytes());
    out.extend_from_slice(&[0; 16]); // resolution, palette counts
    for mask in [0x00FF_0000u32, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000] {
        out.extend_from_slice(&mask.to_le_bytes());
    }
    out.extend_from_slice(&0x7352_4742u32.to_le_bytes()); // LCS_sRGB
    out.extend_from_slice(&[0; 48]); // endpoints and gamma
    for row in (0..height as usize).rev() {
        for px in rgba[row * width as usize * 4..][..width as usize * 4]
            .as_chunks::<4>()
            .0
        {
            out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }
    out
}

// ---------------------------------------------------------------- JPEG

fn decode_jpeg(bytes: &[u8]) -> Result<Decoded> {
    use zune_core::bytestream::ZCursor;
    use zune_core::colorspace::ColorSpace;
    use zune_core::options::DecoderOptions;
    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::RGBA)
        .set_max_width(SIDE_MAX as usize)
        .set_max_height(SIDE_MAX as usize)
        .set_strict_mode(false);
    let mut d = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    d.decode_headers()
        .map_err(|e| codec(format!("JPEG: {e:?}")))?;
    let info = d.info().ok_or_else(|| codec("JPEG has no header"))?;
    check_size(u64::from(info.width), u64::from(info.height))?;
    let rgba = d.decode().map_err(|e| codec(format!("JPEG: {e:?}")))?;
    if rgba.len() as u64 != u64::from(info.width) * u64::from(info.height) * 4 {
        return Err(codec("JPEG decoded to an unexpected size"));
    }
    Ok(Decoded {
        format: ImageFormat::Jpeg,
        width: u32::from(info.width),
        height: u32::from(info.height),
        rgba,
        losses: Vec::new(),
    })
}

/// JPEG has no alpha: pixels are composited over white first.
fn encode_jpeg(width: u32, height: u32, rgba: &[u8], quality: u8) -> Result<Vec<u8>> {
    let (w, h) = (u16::try_from(width), u16::try_from(height));
    let (Ok(w), Ok(h)) = (w, h) else {
        return Err(unsupported("JPEG supports at most 65535 pixels per side"));
    };
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for px in rgba.as_chunks::<4>().0 {
        let a = u32::from(px[3]);
        rgb.extend(
            px[..3]
                .iter()
                .map(|c| ((u32::from(*c) * a + 255 * (255 - a) + 127) / 255) as u8),
        );
    }
    let mut out = Vec::new();
    jpeg_encoder::Encoder::new(&mut out, quality.clamp(1, 100))
        .encode(&rgb, w, h, jpeg_encoder::ColorType::Rgb)
        .map_err(|e| codec(format!("JPEG encoding failed: {e}")))?;
    Ok(out)
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
}
