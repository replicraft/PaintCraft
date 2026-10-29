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
        let expected: &[&str] = if f == ImageFormat::Bmp {
            &["bmp-color-metadata-ignored"]
        } else {
            &[]
        };
        assert_eq!(d.losses, expected, "{}", f.name());
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

fn bmp(bpp: u16, w: i32, h: i32, compression: u32, palette: &[[u8; 3]], pixels: &[u8]) -> Vec<u8> {
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

#[test]
fn bmp_caller_pixel_admission_precedes_palette_sample_scanning() {
    // The header alone exceeds the caller allowance. Do not walk potentially
    // huge packed rows before discovering that the caller cannot admit them.
    let bytes = bmp(8, 3, 1, 0, &[[13, 71, 201]], &[1, 1, 1, 0]);
    assert_eq!(
        decode_with_pixel_limit(&bytes, 2).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    // With enough admission the actual malformed palette sample is checked.
    assert_eq!(
        decode_with_pixel_limit(&bytes, 3).unwrap_err().code,
        ErrorCode::CodecFailed
    );
}
