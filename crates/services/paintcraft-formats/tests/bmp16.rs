//! RGB555/565 conversion: rounded full-range RGB8, no implicit alpha or CMS.
use paintcraft_core::types::{ErrorCode, json};
use paintcraft_formats::image::{self, bmp_metadata};
use sha2::{Digest, Sha256};
use std::path::Path;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/bmp16-fixtures")
            .join(name),
    )
    .unwrap()
}

#[test]
fn rgb555_and_rgb565_keep_all_channel_levels_row_direction_padding_and_source_masks() {
    let oracle = json::parse(&fixture("oracle.json")).unwrap();
    for case in oracle.get("fixtures").unwrap().as_array().unwrap() {
        let name = case.get("file").unwrap().as_str().unwrap();
        let bytes = fixture(name);
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&bytes)),
            case.get("sha256").unwrap().as_str().unwrap()
        );
        let decoded = image::decode_with_pixel_limit(&bytes, 195).unwrap();
        assert_eq!((decoded.width, decoded.height), (65, 3));
        assert_eq!(
            decoded.rgba,
            fixture(&name.replace(".bmp", ".rgba")),
            "{name}"
        );
        let source = bmp_metadata::inspect(&bytes, 195, &|| Ok(())).unwrap();
        assert_eq!(source.bits_per_pixel, 16);
        assert_eq!(source.row_stride, Some(132));
        assert!(source.palette.rgb.is_empty());
        assert_eq!(source.masks[3], 0);
        for (actual, expected) in source.masks[..3]
            .iter()
            .zip(case.get("masks").unwrap().as_array().unwrap())
        {
            assert_eq!(i64::from(*actual), expected.as_i64().unwrap(), "{name}");
        }
        assert_eq!(decoded.losses, source.losses());
        assert_eq!(
            image::decode_with_pixel_limit(&bytes, 194)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
    }
}

#[test]
fn unsupported_masks_and_compression_refuse_before_backend_decode() {
    let bytes = fixture("masks565-v108-bottom.bmp");
    for (at, value) in [
        (54, 0x07e0u32),
        (54, 0x1f000),
        (58, 0x05e0),
        (66, 0x8000),
        (30, 6),
    ] {
        let mut bad = bytes.clone();
        bad[at..at + 4].copy_from_slice(&value.to_le_bytes());
        assert!(image::decode(&bad).is_err(), "field {at} value {value}");
        assert!(bmp_metadata::inspect(&bad, 195, &|| Ok(())).is_err());
    }
    let mut truncated = bytes;
    truncated.pop();
    assert!(image::decode(&truncated).is_err());
    assert!(bmp_metadata::inspect(&truncated, 195, &|| Ok(())).is_err());
    let mut overlap = fixture("masks555-v40-top.bmp");
    overlap[10..14].copy_from_slice(&54u32.to_le_bytes());
    assert!(image::decode(&overlap).is_err());
    assert!(bmp_metadata::inspect(&overlap, 195, &|| Ok(())).is_err());
}
