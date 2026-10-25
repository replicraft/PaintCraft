//! Independent hand-built TGA pixels and explicit WebP RIFF/chunk fixtures.
use paintcraft_core::types::ErrorCode;
use paintcraft_formats::image::{self, ImageFormat};

fn tga(kind: u8, depth: u8, descriptor: u8, width: u16, height: u16, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 18];
    bytes[2] = kind;
    bytes[12..14].copy_from_slice(&width.to_le_bytes());
    bytes[14..16].copy_from_slice(&height.to_le_bytes());
    bytes[16] = depth;
    bytes[17] = descriptor;
    bytes.extend(pixels);
    bytes
}
fn riff(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut bytes = b"RIFF\0\0\0\0WEBP".to_vec();
    for (tag, data) in chunks {
        bytes.extend(*tag);
        bytes.extend((data.len() as u32).to_le_bytes());
        bytes.extend(data);
        if data.len() % 2 != 0 {
            bytes.push(0);
        }
    }
    let length = bytes.len() as u32 - 8;
    bytes[4..8].copy_from_slice(&length.to_le_bytes());
    bytes
}
fn chunks(bytes: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
    let mut result = Vec::new();
    let mut pos = 12;
    while pos < bytes.len() {
        let n = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        result.push((
            bytes[pos..pos + 4].try_into().unwrap(),
            bytes[pos + 8..pos + 8 + n].to_vec(),
        ));
        pos += 8 + n + (n & 1);
    }
    result
}
fn extended(flags: u8, width: u32, height: u32) -> Vec<u8> {
    let mut out = vec![flags, 0, 0, 0];
    out.extend(&(width - 1).to_le_bytes()[..3]);
    out.extend(&(height - 1).to_le_bytes()[..3]);
    out
}
fn webp() -> Vec<u8> {
    image::encode(ImageFormat::WebP, 2, 1, &[1, 2, 3, 0, 80, 90, 100, 128], 90).unwrap()
}

#[test]
fn tga_raw_orientation_rgb_alpha_and_gray_match_independent_pixels() {
    let colors = [
        [1, 2, 3, 255],
        [10, 20, 30, 255],
        [40, 50, 60, 255],
        [70, 80, 90, 255],
    ];
    for descriptor in [0, 0x10, 0x20, 0x30] {
        let mut stored = Vec::new();
        for row in 0..2 {
            for col in 0..2 {
                let y = if descriptor & 0x20 != 0 { row } else { 1 - row };
                let x = if descriptor & 0x10 != 0 { 1 - col } else { col };
                let c = colors[y * 2 + x];
                stored.extend([c[2], c[1], c[0]]);
            }
        }
        let decoded = image::decode(&tga(2, 24, descriptor, 2, 2, &stored)).unwrap();
        assert_eq!(decoded.rgba, colors.concat());
        assert!(decoded.losses.is_empty());
    }
    assert_eq!(
        image::decode(&tga(2, 32, 0x28, 2, 1, &[3, 2, 1, 0, 6, 5, 4, 128]))
            .unwrap()
            .rgba,
        [1, 2, 3, 0, 4, 5, 6, 128]
    );
    assert_eq!(
        image::decode(&tga(3, 8, 0x20, 2, 1, &[10, 90]))
            .unwrap()
            .rgba,
        [10, 10, 10, 255, 90, 90, 90, 255]
    );
    assert_eq!(
        image::decode(&tga(3, 16, 0x28, 2, 1, &[10, 0, 90, 128]))
            .unwrap()
            .rgba,
        [10, 10, 10, 0, 90, 90, 90, 128]
    );
}

#[test]
fn tga_rle_palette_offsets_and_truncations_are_strict() {
    // Run: two red; raw packet: green then blue.
    let bytes = tga(
        10,
        24,
        0x20,
        4,
        1,
        &[0x81, 0, 0, 255, 1, 0, 255, 0, 255, 0, 0],
    );
    assert_eq!(
        image::decode(&bytes).unwrap().rgba,
        [
            255, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255
        ]
    );
    for end in 0..bytes.len() {
        assert!(image::decode(&bytes[..end]).is_err(), "prefix {end}");
    }
    let mut palette = tga(1, 8, 0x20, 2, 1, &[]);
    palette[1] = 1;
    palette[3..5].copy_from_slice(&5u16.to_le_bytes());
    palette[5..7].copy_from_slice(&2u16.to_le_bytes());
    palette[7] = 24;
    palette.extend([3, 2, 1, 6, 5, 4, 6, 5]);
    let decoded = image::decode(&palette).unwrap();
    assert_eq!(decoded.rgba, [4, 5, 6, 255, 1, 2, 3, 255]);
    assert_eq!(decoded.losses, ["palette-indices-expanded"]);
    *palette.last_mut().unwrap() = 4;
    assert_eq!(
        image::decode(&palette).unwrap_err().code,
        ErrorCode::CodecFailed
    );
    assert_eq!(
        image::decode(&tga(10, 24, 0x20, 1, 1, &[0x81, 1, 2, 3]))
            .unwrap_err()
            .code,
        ErrorCode::CodecFailed
    );
    assert_eq!(
        image::decode_with_pixel_limit(&bytes, 3).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn tga_exports_preserve_hidden_rgb_and_refuse_ambiguous_profiles() {
    let rgba = [1, 2, 3, 0, 4, 5, 6, 1, 7, 8, 9, 128, 20, 30, 40, 255];
    let encoded = image::encode(ImageFormat::Tga, 2, 2, &rgba, 90).unwrap();
    assert_eq!(image::decode(&encoded).unwrap().rgba, rgba);
    assert!(encoded.ends_with(b"TRUEVISION-XFILE.\0"));
    let ambiguous = tga(2, 32, 0x20, 1, 1, &[1, 2, 3, 255]);
    assert_eq!(
        image::decode(&ambiguous).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let mut extension = encoded.clone();
    let at = extension.len() - 26;
    extension[at..at + 4].copy_from_slice(&18u32.to_le_bytes());
    assert_eq!(
        image::decode(&extension).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let mut metadata = tga(2, 24, 0x20, 1, 1, &[]);
    metadata[0] = 3;
    metadata.extend(b"abc");
    metadata.extend([1, 2, 3]);
    assert_eq!(
        image::decode(&metadata).unwrap().losses,
        ["tga-metadata-ignored"]
    );
    assert!(ImageFormat::Tga.export_losses_for_rgba(&rgba).is_empty());
}

#[test]
fn webp_lossless_preserves_every_alpha_and_hidden_rgb() {
    let rgba: Vec<u8> = (0..256u32)
        .flat_map(|alpha| [(alpha % 255) as u8, 80, 170, alpha as u8])
        .collect();
    let encoded = image::encode(ImageFormat::WebP, 16, 16, &rgba, 90).unwrap();
    let decoded = image::decode(&encoded).unwrap();
    assert_eq!(decoded.rgba, rgba);
    assert!(decoded.losses.is_empty());
    assert!(ImageFormat::WebP.export_losses_for_rgba(&rgba).is_empty());
    assert_eq!(
        image::decode_with_pixel_limit(&encoded, 255)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let rgb = image::encode(ImageFormat::WebP, 1, 1, &[9, 8, 7, 255], 90).unwrap();
    assert_eq!(image::decode(&rgb).unwrap().rgba, [9, 8, 7, 255]);
}

#[test]
fn webp_animation_unknown_chunks_feature_mismatches_and_metadata_are_explicit() {
    let base = webp();
    let chunks = chunks(&base);
    let image = chunks
        .iter()
        .find(|(tag, _)| tag == b"VP8L")
        .unwrap()
        .1
        .clone();
    let bytes = riff(&[
        (b"VP8X", extended(0x3c, 2, 1)),
        (b"ICCP", vec![1, 2, 3, 4]),
        (b"VP8L", image.clone()),
        (b"EXIF", b"II*\0\x08\0\0\0\0\0\0\0\0\0".to_vec()),
        (b"XMP ", b"hello".to_vec()),
    ]);
    let decoded = image::decode(&bytes).unwrap();
    assert!(decoded.losses.iter().any(|s| s == "icc-profile-ignored"));
    assert!(decoded.losses.iter().any(|s| s == "exif-metadata-ignored"));
    assert!(decoded.losses.iter().any(|s| s == "webp-metadata-ignored"));
    for anim in [
        riff(&[(b"VP8X", extended(2, 2, 1)), (b"VP8L", image.clone())]),
        riff(&[(b"ANMF", vec![0; 16]), (b"VP8L", image.clone())]),
    ] {
        assert_eq!(
            image::decode(&anim).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
    assert_eq!(
        image::decode(&riff(&[(b"ZZZZ", vec![0]), (b"VP8L", image.clone())]))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        image::decode(&riff(&[
            (b"VP8X", extended(0x10, 3, 1)),
            (b"VP8L", image.clone())
        ]))
        .unwrap_err()
        .code,
        ErrorCode::CodecFailed
    );
    assert_eq!(
        image::decode(&riff(&[
            (b"VP8X", extended(0x30, 2, 1)),
            (b"VP8L", image.clone())
        ]))
        .unwrap_err()
        .code,
        ErrorCode::CodecFailed
    );
    assert_eq!(
        image::decode(&riff(&[
            (b"VP8X", extended(0x30, 2, 1)),
            (b"ICCP", vec![0; 1_048_577]),
            (b"VP8L", image)
        ]))
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn webp_lengths_truncation_and_claimed_geometry_are_bounded() {
    let bytes = webp();
    for end in 0..bytes.len() {
        assert!(image::decode(&bytes[..end]).is_err(), "prefix {end}");
    }
    let mut oversized = bytes.clone();
    oversized[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        image::decode(&oversized).unwrap_err().code,
        ErrorCode::CodecFailed
    );
    let huge = riff(&[(b"VP8X", extended(0, 1_000_000, 1_000_000))]);
    assert_eq!(
        image::decode(&huge).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn independent_libwebp_fixtures_decode_expected_samples() {
    let alpha = include_bytes!("raster-fixtures/alpha.webp");
    let expected: Vec<u8> = (0..256u32)
        .flat_map(|a| [(a % 255) as u8, 80, 170, a as u8])
        .collect();
    let decoded = image::decode(alpha).unwrap();
    assert_eq!((decoded.width, decoded.height), (16, 16));
    assert_eq!(decoded.rgba, expected);
    let opaque = image::decode(include_bytes!("raster-fixtures/opaque.webp")).unwrap();
    assert_eq!((opaque.width, opaque.height), (3, 2));
    for px in opaque.rgba.as_chunks::<4>().0 {
        for (actual, expected) in px.iter().zip([80u8, 90, 100, 255]) {
            assert!(actual.abs_diff(expected) <= 3, "{px:?}");
        }
    }
}

/// Explicit opt-in: executes the independent libwebp decoder, without UI.
#[test]
#[ignore = "requires the installed dwebp command"]
fn external_libwebp_decodes_paintcraft_lossless_export_exactly() {
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let directory =
        Directory(std::env::temp_dir().join(format!("paintcraft-webp-{}", std::process::id())));
    std::fs::create_dir(&directory.0).unwrap();
    let expected: Vec<u8> = (0..256u32)
        .flat_map(|a| [(a % 255) as u8, 80, 170, a as u8])
        .collect();
    let encoded = image::encode(ImageFormat::WebP, 16, 16, &expected, 90).unwrap();
    let input = directory.0.join("paintcraft.webp");
    let output = directory.0.join("oracle.pam");
    std::fs::write(&input, encoded).unwrap();
    let result = std::process::Command::new("dwebp")
        .arg("-quiet")
        .arg("-pam")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let decoded = std::fs::read(output).unwrap();
    let marker = b"ENDHDR\n";
    let at = decoded
        .windows(marker.len())
        .position(|w| w == marker)
        .unwrap()
        + marker.len();
    let header = std::str::from_utf8(&decoded[..at]).unwrap();
    assert!(header.contains("WIDTH 16\n"));
    assert!(header.contains("HEIGHT 16\n"));
    assert!(header.contains("DEPTH 4\n"));
    assert_eq!(&decoded[at..], expected);
}
