use super::*;
use paintcraft_core::types::json::{self, Value};
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/gif-still-fixtures")
            .join(name),
    )
    .unwrap()
}
fn number(value: &Value, key: &str) -> u32 {
    value.get(key).unwrap().as_i64().unwrap() as u32
}
fn samples(value: &Value) -> Vec<u8> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| u8::try_from(v.as_i64().unwrap()).unwrap())
        .collect()
}
#[test]
fn independent_partial_still_vectors_keep_declared_background_placement_interlace_and_raw_alpha() {
    let manifest = json::parse(include_bytes!(
        "../../../tests/gif-still-fixtures/oracle.json"
    ))
    .unwrap();
    let mut accepted = 0;
    for case in manifest.get("cases").unwrap().as_array().unwrap() {
        let name = case.get("file").unwrap().as_str().unwrap();
        let bytes = fixture(name);
        let result = crate::image::decode(&bytes);
        if case.get("supported") == Some(&Value::Bool(false)) {
            assert_eq!(result.unwrap_err().code, ErrorCode::CapabilityUnsupported);
            continue;
        }
        let decoded = result.unwrap_or_else(|error| panic!("{name}: {error}"));
        let descriptor = case.get("descriptor").unwrap();
        assert_eq!(
            (decoded.width, decoded.height),
            (number(descriptor, "width"), number(descriptor, "height")),
            "{name}"
        );
        assert_eq!(decoded.rgba, samples(case.get("rgba").unwrap()), "{name}");
        assert_eq!(decoded.losses, ["palette-indices-expanded"], "{name}");
        accepted += 1;
    }
    assert_eq!(accepted, 9);
}

#[test]
fn actual_lzw_palette_overflow_is_refused_after_valid_descriptor_preflight_without_an_output_prefix()
 {
    let bytes = fixture("invalid-palette-index.gif");
    let source = json::parse(include_bytes!(
        "../../../tests/gif-still-fixtures/invalid-palette-index.json"
    ))
    .unwrap();
    assert_eq!(source.get("palette").unwrap().as_array().unwrap().len(), 2);
    assert!(
        source
            .get("indices")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|index| index.as_i64().unwrap() >= 2)
    );
    assert!(
        inspect(&bytes, 20).is_ok(),
        "metadata/background/bounds are valid; actual decoded samples must cause this refusal"
    );
    let error = crate::image::decode(&bytes).unwrap_err();
    assert_eq!(error.code, ErrorCode::CodecFailed);
    assert!(error.message.contains("sample index"), "{error}");
}
#[test]
fn malformed_geometry_controls_animation_and_resource_bounds_never_return_a_raster() {
    let bytes = fixture("offset-background2.gif");
    assert_eq!(bytes[33], 0x2c);
    for prefix in 0..bytes.len() {
        assert!(
            crate::image::decode(&bytes[..prefix]).is_err(),
            "prefix {prefix}"
        );
    }
    for (at, word) in [(34, 4u16), (36, 3), (38, 0), (40, 0)] {
        let mut bad = bytes.clone();
        bad[at..at + 2].copy_from_slice(&word.to_le_bytes());
        assert_eq!(
            crate::image::decode(&bad).unwrap_err().code,
            ErrorCode::CodecFailed
        );
    }
    let mut bad = bytes.clone();
    bad[11] = 255;
    assert!(crate::image::decode(&bad).is_err());
    let mut bad = bytes.clone();
    bad[28] = 0x10;
    assert_eq!(
        crate::image::decode(&bad).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let mut bad = bytes.clone();
    bad[28] = 0x20;
    assert!(crate::image::decode(&bad).is_err());
    let mut bad = bytes.clone();
    bad[27] = 3;
    assert!(crate::image::decode(&bad).is_err());
    let mut bad = bytes.clone();
    bad[42] |= 0x08;
    assert!(crate::image::decode(&bad).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(crate::image::decode(&trailing).is_err());
    let mut animated = bytes[..bytes.len() - 1].to_vec();
    animated.extend_from_slice(&bytes[25..]);
    assert_eq!(
        crate::image::decode(&animated).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        crate::image::decode_with_pixel_limit(&bytes, 19)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        buffer(usize::MAX).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    // A frame may exactly touch the right/bottom edge; never silently clip it.
    let mut edge = bytes.clone();
    edge[34..36].copy_from_slice(&2u16.to_le_bytes());
    edge[36..38].copy_from_slice(&2u16.to_le_bytes());
    let output = crate::image::decode(&edge).unwrap();
    assert_eq!((output.width, output.height), (5, 4));
    assert_eq!(
        &output.rgba[(3 * 5 + 4) * 4..(3 * 5 + 4) * 4 + 4],
        &[90, 80, 70, 255]
    );
}
#[test]
fn still_timing_and_color_profile_omissions_remain_explicit() {
    let mut bytes = fixture("offset-background2.gif");
    bytes[29] = 7;
    assert_eq!(
        crate::image::decode(&bytes).unwrap().losses,
        ["palette-indices-expanded", "gif-metadata-ignored"]
    );
    let original = fixture("offset-background2.gif");
    let mut icc = original[..25].to_vec();
    icc.extend_from_slice(b"\x21\xff\x0bICCRGBG1012\x05\x01\x01abc\0");
    icc.extend_from_slice(&original[25..]);
    let decoded = crate::image::decode(&icc).unwrap();
    assert_eq!(
        decoded.losses,
        [
            "palette-indices-expanded",
            "gif-metadata-ignored",
            "icc-profile-ignored"
        ]
    );
}

#[test]
fn complete_sample_count_still_requires_eoi_and_rejects_short_or_extra_output() {
    let valid = fixture("literal-three.gif");
    let oracle = json::parse(include_bytes!(
        "../../../tests/gif-still-fixtures/literal-three.json"
    ))
    .unwrap();
    assert_eq!(samples(oracle.get("indices").unwrap()), [0, 1, 0]);
    assert_eq!(
        crate::image::decode(&valid).unwrap().rgba,
        [10, 20, 30, 255, 90, 80, 70, 255, 10, 20, 30, 255]
    );
    let mut no_eoi = valid.clone();
    assert_eq!(no_eoi[40], 0x50);
    no_eoi[40] = 0;
    // The declared samples can be filled before the malformed tail is seen.
    // The production path must also drain/check EOI before returning pixels.
    let mut options = ::gif::DecodeOptions::new();
    options.set_color_output(::gif::ColorOutput::Indexed);
    options.check_lzw_end_code(true);
    let mut reader = options.read_info(Cursor::new(&no_eoi)).unwrap();
    reader.next_frame_info().unwrap().unwrap();
    let mut indices = [0; 3];
    assert!(reader.fill_buffer(&mut indices).unwrap());
    assert_eq!(indices, [0, 1, 0]);
    assert!(reader.next_frame_info().is_err());
    assert_eq!(
        crate::image::decode(&no_eoi).unwrap_err().code,
        ErrorCode::CodecFailed
    );
    for width in [2u16, 4] {
        let mut mismatch = valid.clone();
        mismatch[6..8].copy_from_slice(&width.to_le_bytes());
        mismatch[32..34].copy_from_slice(&width.to_le_bytes());
        assert!(inspect(&mismatch, 4).is_ok());
        assert_eq!(
            crate::image::decode(&mismatch).unwrap_err().code,
            ErrorCode::CodecFailed,
            "declared width {width}"
        );
    }
}

#[test]
fn short_interlace_passes_keep_one_and_two_row_raw_samples_in_order() {
    for (name, flags) in [("literal-three.gif", 36), ("offset-background2.gif", 42)] {
        let bytes = fixture(name);
        let expected = crate::image::decode(&bytes).unwrap().rgba;
        let mut interlaced = bytes;
        interlaced[flags] |= 0x40;
        assert_eq!(
            crate::image::decode(&interlaced).unwrap().rgba,
            expected,
            "{name}"
        );
    }
}

#[test]
fn legacy_full_screen_without_graphic_control_retains_87a_samples() {
    let original = fixture("literal-three.gif");
    let expected = crate::image::decode(&original).unwrap().rgba;
    let mut bytes = original[..19].to_vec();
    bytes.extend_from_slice(&original[27..]);
    bytes[..6].copy_from_slice(b"GIF87a");
    assert_eq!(crate::image::decode(&bytes).unwrap().rgba, expected);
}
