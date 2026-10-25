use super::*;

// Hand-built GIF: 2 palette entries, explicit LZW clear/pixels/end. This does
// not use the production encoder as a decoder oracle.
fn gif_fixture() -> Vec<u8> {
    let mut bytes = b"GIF89a\x02\0\x01\0\x80\0\0".to_vec();
    bytes.extend([10, 20, 30, 90, 80, 70]);
    bytes.extend([0x21, 0xf9, 4, 1, 0, 0, 1, 0]); // palette index 1 transparent
    bytes.extend([0x2c, 0, 0, 0, 0, 2, 0, 1, 0, 0, 2, 2, 0x44, 0x0a, 0, 0x3b]);
    bytes
}

// Uncompressed classic TIFF assembled from specification field types. Both
// byte orders and arbitrary additional tags exercise the admission parser.
fn tiff_fixture(
    little: bool,
    depth: u16,
    channels: u16,
    data: &[u8],
    extra: &[(u16, u16, u32, u32)],
) -> Vec<u8> {
    let word = |v: u16| {
        if little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    };
    let long = |v: u32| {
        if little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    };
    let mut fields = vec![
        (256, 4, 1, 2),
        (257, 4, 1, 1),
        (258, 3, 1, u32::from(depth)),
        (259, 3, 1, 1),
        (262, 3, 1, if channels >= 3 { 2 } else { 1 }),
        (273, 4, 1, 0),
        (277, 3, 1, u32::from(channels)),
        (278, 4, 1, 1),
        (279, 4, 1, data.len() as u32),
    ];
    if channels == 4 || channels == 2 {
        fields.push((338, 3, 1, 2));
    }
    fields.extend_from_slice(extra);
    fields.sort_by_key(|f| f.0);
    let start = 8 + 2 + fields.len() * 12 + 4;
    fields.iter_mut().find(|f| f.0 == 273).unwrap().3 = start as u32;
    let mut bytes = if little {
        b"II".to_vec()
    } else {
        b"MM".to_vec()
    };
    bytes.extend(word(42));
    bytes.extend(long(8));
    bytes.extend(word(fields.len() as u16));
    for (tag, kind, count, value) in fields {
        bytes.extend(word(tag));
        bytes.extend(word(kind));
        bytes.extend(long(count));
        if kind == 3 && count == 1 {
            bytes.extend(word(value as u16));
            bytes.extend([0, 0]);
        } else {
            bytes.extend(long(value));
        }
    }
    bytes.extend(long(0));
    bytes.extend(data);
    bytes
}

#[test]
fn gif_palette_and_one_bit_transparency_are_explicit() {
    let decoded = decode(&gif_fixture()).unwrap();
    assert_eq!((decoded.width, decoded.height), (2, 1));
    assert_eq!(decoded.rgba, [10, 20, 30, 255, 90, 80, 70, 0]);
    assert_eq!(decoded.losses, ["palette-indices-expanded"]);
    let rgba = [
        10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 128, 100, 110, 120, 255,
    ];
    let bytes = encode(ImageFormat::Gif, 4, 1, &rgba, 90).unwrap();
    let output = decode(&bytes).unwrap();
    assert_eq!(
        output
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[3])
            .collect::<Vec<_>>(),
        [0, 255, 255, 255]
    );
    assert_eq!(
        &output.rgba[4..],
        &[40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255]
    );
    assert_eq!(
        ImageFormat::Gif.export_losses(true),
        [
            "palette-quantized",
            "alpha-binarized",
            "transparent-rgb-discarded"
        ]
    );
    assert_eq!(ImageFormat::Gif.export_losses(false), ["palette-quantized"]);
}

#[test]
fn gif_animation_and_hostile_headers_are_refused_before_codec() {
    let fixture = gif_fixture();
    let mut animated = fixture[..fixture.len() - 1].to_vec();
    animated.extend_from_slice(&fixture[27..]);
    assert_eq!(
        decode(&animated).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    for length in 0..fixture.len() {
        assert!(decode(&fixture[..length]).is_err(), "prefix {length}");
    }
    assert_eq!(
        decode_with_pixel_limit(&fixture, 1).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let mut out_of_bounds = fixture.clone();
    out_of_bounds[32] = 3;
    assert!(decode(&out_of_bounds).is_err());
    let mut huge = fixture.clone();
    huge[6..10].copy_from_slice(&[255, 255, 255, 255]);
    assert_eq!(decode(&huge).unwrap_err().code, ErrorCode::ResourceLimit);
    let mut metadata = fixture[..19].to_vec();
    metadata.extend([0x21, 0xfe]);
    for _ in 0..4113 {
        metadata.push(255);
        metadata.extend([1; 255]);
    }
    metadata.push(0);
    metadata.extend_from_slice(&fixture[19..]);
    assert_eq!(
        decode(&metadata).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn tiff_uint_samples_endian_depth_and_hidden_rgb() {
    for little in [true, false] {
        let rgba = [1, 2, 3, 0, 200, 100, 50, 128];
        let decoded = decode(&tiff_fixture(little, 8, 4, &rgba, &[])).unwrap();
        assert_eq!(decoded.rgba, rgba);
        assert!(decoded.losses.is_empty());
        let deep = if little {
            [0xff, 0x12, 0x00, 0xab]
        } else {
            [0x12, 0xff, 0xab, 0x00]
        };
        let decoded = decode(&tiff_fixture(little, 16, 1, &deep, &[])).unwrap();
        assert_eq!(decoded.rgba, [0x12, 0x12, 0x12, 255, 0xab, 0xab, 0xab, 255]);
        assert_eq!(decoded.losses, ["depth-16-reduced-to-8"]);
    }
    let rgba = [99, 88, 77, 0, 66, 55, 44, 129];
    let decoded = decode(&encode(ImageFormat::Tiff, 2, 1, &rgba, 90).unwrap()).unwrap();
    assert_eq!(decoded.rgba, rgba);
    assert_eq!(decoded.losses, ["tiff-metadata-ignored"]);
}

#[test]
fn tiff_structures_precision_and_metadata_are_never_silently_lost() {
    let fixture = tiff_fixture(true, 8, 1, &[10, 20], &[]);
    let mut pages = fixture.clone();
    let count = u16::from_le_bytes([pages[8], pages[9]]) as usize;
    pages[10 + count * 12..14 + count * 12].copy_from_slice(&8u32.to_le_bytes());
    assert_eq!(
        decode(&pages).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    for extra in [
        (274, 3, 1, 6),
        (339, 3, 1, 3),
        (338, 3, 1, 1),
        (330, 4, 1, 8),
    ] {
        assert_eq!(
            decode(&tiff_fixture(true, 8, 1, &[10, 20], &[extra]))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    assert_eq!(
        decode_with_pixel_limit(&fixture, 1).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let metadata = decode(&tiff_fixture(
        true,
        8,
        1,
        &[10, 20],
        &[(270, 2, 4, 0x00636261)],
    ))
    .unwrap();
    assert_eq!(metadata.losses, ["tiff-metadata-ignored"]);
    for length in 0..fixture.len() {
        assert!(decode(&fixture[..length]).is_err(), "prefix {length}");
    }
}

#[test]
fn precise_gif_losses_distinguish_binary_and_partial_alpha() {
    assert_eq!(
        ImageFormat::Gif.export_losses_for_rgba(&[0, 0, 0, 0]),
        ["palette-quantized"]
    );
    assert_eq!(
        ImageFormat::Gif.export_losses_for_rgba(&[1, 2, 3, 255]),
        ["palette-quantized"]
    );
    assert_eq!(
        ImageFormat::Gif.export_losses_for_rgba(&[1, 2, 3, 0]),
        ["palette-quantized", "transparent-rgb-discarded"]
    );
    assert_eq!(
        ImageFormat::Gif.export_losses_for_rgba(&[1, 2, 3, 128]),
        ["palette-quantized", "alpha-binarized"]
    );
}

#[test]
fn tiff_profile_bounds_and_unqualified_models_are_explicit() {
    let profile = decode(&tiff_fixture(
        true,
        8,
        1,
        &[10, 20],
        &[(34675, 7, 4, 0x04030201)],
    ))
    .unwrap();
    assert_eq!(profile.losses, ["icc-profile-ignored"]);
    let oversized = tiff_fixture(true, 8, 1, &[10, 20], &[(34675, 7, 1_048_577, 0)]);
    assert_eq!(
        decode(&oversized).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let bad_offset = tiff_fixture(false, 8, 1, &[10, 20], &[(270, 2, 40, u32::MAX)]);
    assert_eq!(
        decode(&bad_offset).unwrap_err().code,
        ErrorCode::CodecFailed
    );
    let mut cmyk = tiff_fixture(true, 8, 4, &[1; 8], &[]);
    // Locate photometric interpretation in the independent IFD.
    let count = u16::from_le_bytes([cmyk[8], cmyk[9]]) as usize;
    for index in 0..count {
        let at = 10 + 12 * index;
        if u16::from_le_bytes([cmyk[at], cmyk[at + 1]]) == 262 {
            cmyk[at + 8..at + 10].copy_from_slice(&5u16.to_le_bytes());
        }
    }
    assert_eq!(
        decode(&cmyk).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let mut oversized = tiff_fixture(true, 8, 1, &[10, 20], &[]);
    oversized[18..22].copy_from_slice(&1_000_000u32.to_le_bytes());
    // Both width and height are admitted before codec allocation.
    oversized[30..34].copy_from_slice(&1_000_000u32.to_le_bytes());
    assert_eq!(
        decode(&oversized).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn tiff_one_bit_and_rgba16_use_declared_sample_semantics() {
    let one = decode(&tiff_fixture(true, 1, 1, &[0b0100_0000], &[])).unwrap();
    assert_eq!(one.rgba, [0, 0, 0, 255, 255, 255, 255, 255]);
    for little in [true, false] {
        let samples: [u16; 8] = [0x12ff, 0x3400, 0x56fe, 0, 0xabcd, 0x9876, 0x7654, 0x8080];
        let raw: Vec<u8> = samples
            .into_iter()
            .flat_map(|v| {
                if little {
                    v.to_le_bytes()
                } else {
                    v.to_be_bytes()
                }
            })
            .collect();
        let decoded = decode(&tiff_fixture(little, 16, 4, &raw, &[])).unwrap();
        assert_eq!(decoded.rgba, [0x12, 0x34, 0x56, 0, 0xab, 0x98, 0x76, 0x80]);
        assert_eq!(decoded.losses, ["depth-16-reduced-to-8"]);
    }
}

#[test]
fn gif_truecolor_export_quantizes_to_at_most_256_colors() {
    let rgba: Vec<u8> = (0..1024u32)
        .flat_map(|n| [(n % 256) as u8, (n / 4) as u8, (n / 256) as u8, 255])
        .collect();
    let bytes = encode(ImageFormat::Gif, 32, 32, &rgba, 90).unwrap();
    let decoded = decode(&bytes).unwrap();
    let colors: std::collections::BTreeSet<_> = decoded.rgba.as_chunks::<4>().0.iter().collect();
    assert!(colors.len() <= 256);
    assert_ne!(decoded.rgba, rgba);
    assert_eq!(
        ImageFormat::Gif.export_losses_for_rgba(&rgba),
        ["palette-quantized"]
    );
}
