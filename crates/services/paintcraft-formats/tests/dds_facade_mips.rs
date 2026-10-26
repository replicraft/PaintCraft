//! The image facade carries actual mip omission into the shared consent envelope.
use paintcraft_core::types::ErrorCode;
use paintcraft_formats::{dds, image};
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/dds-mip-fixtures/{name}.dds",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
fn put(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
#[test]
fn facade_imports_top_pixels_with_actual_omission_while_strict_api_stays_strict() {
    let expected = include_bytes!("dds-mip-fixtures/top.rgba");
    for name in [
        "dx10-rgba",
        "dx10-bgra",
        "dx10-bc1-srgb",
        "dx10-bc2-srgb",
        "dx10-bc3-srgb",
    ] {
        let bytes = fixture(name);
        let original = bytes.clone();
        let decoded = image::decode_with_pixel_limit(&bytes, 35).unwrap();
        assert_eq!(decoded.format, image::ImageFormat::Dds);
        assert_eq!((decoded.width, decoded.height), (7, 5));
        assert_eq!(&decoded.rgba, expected);
        assert_eq!(decoded.losses, [dds::OTHER_MIPS_LOSS]);
        assert!(image::KNOWN_IMPORT_LOSSES.contains(&dds::OTHER_MIPS_LOSS));
        assert!(dds::inspect_checked(&bytes, 35, &|| Ok(())).is_err());
        assert!(dds::decode_checked(&bytes, 35, &|| Ok(())).is_err());
        assert_eq!(bytes, original);
    }
}
#[test]
fn facade_still_refuses_linear_unknown_and_associated_interpretations() {
    for name in [
        "legacy-rgba",
        "legacy-bc1",
        "dx10-bc1",
        "dx10-bc2",
        "dx10-bc3",
    ] {
        assert_eq!(
            image::decode(&fixture(name)).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
    for alpha in [0, 2, 4, 8] {
        let mut bytes = fixture("dx10-rgba");
        put(&mut bytes, 144, alpha);
        assert_eq!(
            image::decode(&bytes).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
    let mut opaque = fixture("dx10-rgba");
    put(&mut opaque, 144, 3);
    opaque[151] = 0;
    let decoded = image::decode(&opaque).unwrap();
    assert_eq!(&decoded.rgba, include_bytes!("dds-mip-fixtures/top.rgba"));
    assert_eq!(decoded.losses, [dds::OTHER_MIPS_LOSS]);
}
#[test]
fn facade_cannot_hide_resource_topology_or_complete_tail_errors() {
    let source = fixture("dx10-rgba");
    assert_eq!(
        image::decode_with_pixel_limit(&source, 34)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    for (offset, value) in [
        (140, 2),
        (136, 4),
        (132, 4),
        (112, 0x200),
        (28, 4),
        (28, 0),
        (8, 0x100f),
        (108, 0x1000),
        (20, 32),
    ] {
        let mut bytes = source.clone();
        put(&mut bytes, offset, value);
        assert!(image::decode(&bytes).is_err(), "offset {offset}: {value}");
    }
    // A full top level is insufficient: every declared lower level is required.
    for end in 148 + 140..source.len() {
        assert!(image::decode(&source[..end]).is_err());
    }
    let mut trailing = source.clone();
    trailing.push(0);
    assert!(image::decode(&trailing).is_err());
}
#[test]
fn single_level_has_no_invented_loss_and_does_not_lose_hidden_rgb() {
    let rgba = [9, 17, 31, 0, 23, 42, 99, 255];
    let bytes = dds::encode_rgba8(2, 1, &rgba, dds::Transfer::Srgb).unwrap();
    let decoded = image::decode(&bytes).unwrap();
    assert_eq!(decoded.rgba, rgba);
    assert!(decoded.losses.is_empty());
    let strict = dds::decode_checked(&bytes, 2, &|| Ok(())).unwrap();
    assert_eq!(decoded.rgba, strict.rgba);
}
