use super::*;
use paintcraft_core::types::{Error, ErrorCode, json};
use std::cell::Cell;
const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/raster-metadata-fixtures"
);
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{name}")).unwrap()
}
#[test]
fn independent_bmp_pixels_hidden_rgb_palette_reserved_and_storage_rows_are_exact() {
    let oracle = json::parse(&fixture("oracle.json")).unwrap();
    for row in oracle.get("fixtures").unwrap().as_array().unwrap() {
        let name = row.get("file").unwrap().as_str().unwrap();
        if !name.ends_with(".bmp") {
            continue;
        }
        let bytes = fixture(name);
        let descriptor = inspect(&bytes, 1_048_576, &|| Ok(())).unwrap();
        let decoded = super::super::decode_with_pixel_limit(&bytes, 1_048_576).unwrap();
        let expected: Vec<u8> = row
            .get("expected_rgba8")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap() as u8)
            .collect();
        assert_eq!(decoded.rgba, expected, "{name}");
        assert_eq!(
            (descriptor.width, descriptor.height),
            (decoded.width, decoded.height)
        );
        if name.starts_with("indexed") {
            assert_eq!(descriptor.palette_reserved, [137, 0]);
            assert!(descriptor.palette.alpha.is_empty());
            assert_eq!(descriptor.palette.rgb, [[13, 71, 201], [250, 1, 20]]);
            assert!(descriptor.losses().contains(&"palette-indices-expanded"));
        }
        if name == "rgb24-top-down.bmp" {
            assert!(descriptor.top_down);
            assert_eq!(descriptor.row_stride, 12);
            assert!(descriptor.losses().is_empty());
        }
    }
}
#[test]
fn original_v4_calibration_and_v5_profile_and_resolution_are_not_color_management() {
    let calibrated = inspect(&fixture("rgb24-v4-calibrated.bmp"), 1_048_576, &|| Ok(())).unwrap();
    assert_eq!(calibrated.color_space, Some(CALIBRATED_RGB));
    assert_eq!(calibrated.gamma_scaled, Some([65536; 3]));
    assert_eq!(calibrated.losses(), ["bmp-color-metadata-ignored"]);
    let bytes = fixture("rgb24-v5-icc.bmp");
    let descriptor = inspect(&bytes, 1, &|| Ok(()));
    assert!(matches!(descriptor,Err(e) if e.code==ErrorCode::ResourceLimit));
    let descriptor = inspect(&bytes, 6, &|| Ok(())).unwrap();
    assert_eq!(descriptor.color_space, Some(PROFILE_EMBEDDED));
    assert_eq!(descriptor.icc.as_ref().unwrap().color_space, *b"RGB ");
    assert_eq!(descriptor.resolution.unwrap().x, 3780);
    assert_eq!(descriptor.undeclared_bytes, 0);
    assert!(
        matches!(inspect_with_metadata_limit(&bytes,6,8,&||Ok(())),Err(e) if e.code==ErrorCode::ResourceLimit)
    );
}
#[test]
fn malformed_profile_extents_linked_profiles_masks_and_cancellation_refuse() {
    let original = fixture("rgb24-v5-icc.bmp");
    for (at, value) in [(126, 0u32), (126, u32::MAX), (130, u32::MAX), (134, 1)] {
        let mut bytes = original.clone();
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        assert!(inspect(&bytes, 1_048_576, &|| Ok(())).is_err());
    }
    let mut linked = original.clone();
    linked[70..74].copy_from_slice(&PROFILE_LINKED.to_le_bytes());
    assert!(
        matches!(inspect(&linked,1_048_576,&||Ok(())),Err(e) if e.code==ErrorCode::CapabilityUnsupported)
    );
    let mut overlap = original.clone();
    let pixels = u32::from_le_bytes(overlap[10..14].try_into().unwrap());
    overlap[126..130].copy_from_slice(&(pixels - 14).to_le_bytes());
    assert!(inspect(&overlap, 1_048_576, &|| Ok(())).is_err());
    let mut masks = fixture("rgba32-v4-hidden.bmp");
    masks[54..58].copy_from_slice(&0xF800u32.to_le_bytes());
    assert!(inspect(&masks, 1_048_576, &|| Ok(())).is_err());
    let calls = Cell::new(0);
    let result = inspect(&original, 1_048_576, &|| {
        calls.set(calls.get() + 1);
        if calls.get() > 4 {
            Err(Error::new(ErrorCode::OperationCancelled, "cancel metadata"))
        } else {
            Ok(())
        }
    });
    assert!(matches!(result,Err(e) if e.code==ErrorCode::OperationCancelled));
}
