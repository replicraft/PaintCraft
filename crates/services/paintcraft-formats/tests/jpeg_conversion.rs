//! The public raster facade must publish the independently qualified JPEG pixels.
use paintcraft_core::types::{ErrorCode, json, policy::hex};
use paintcraft_formats::image::{self, ImageFormat};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/jpeg-fixtures")
            .join(name),
    )
    .unwrap()
}

#[test]
fn facade_materializes_independent_orientation_and_device_color_oracles_once() {
    let oracle = json::parse(&fixture("oracle.json")).unwrap();
    let cases = oracle.get("cases").unwrap().as_array().unwrap();
    assert_eq!(cases.len(), 54);
    for case in cases {
        let name = case.get("file").unwrap().as_str().unwrap();
        let bytes = fixture(name);
        let expected = fixture(case.get("rgba_file").unwrap().as_str().unwrap());
        assert_eq!(
            hex(&Sha256::digest(&expected)),
            case.get("rgba_sha256").unwrap().as_str().unwrap()
        );
        let width = case.get("expected_width").unwrap().as_i64().unwrap() as u32;
        let height = case.get("expected_height").unwrap().as_i64().unwrap() as u32;
        let pixels = u64::from(width) * u64::from(height);
        let decoded = image::decode_with_pixel_limit(&bytes, pixels)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            (decoded.format, decoded.width, decoded.height),
            (ImageFormat::Jpeg, width, height),
            "{name}"
        );
        assert_eq!(
            hex(&Sha256::digest(&decoded.rgba)),
            hex(&Sha256::digest(&expected)),
            "{name}"
        );
        assert!(
            decoded
                .losses
                .iter()
                .all(|loss| image::KNOWN_IMPORT_LOSSES.contains(&loss.as_str())),
            "{name}: {:?}",
            decoded.losses
        );
        let key = image::document_conversion_key(&bytes, &decoded, &|| Ok(()))
            .unwrap()
            .unwrap();
        assert_eq!(
            Some(key),
            image::document_conversion_key(&bytes, &decoded, &|| Ok(())).unwrap()
        );
        let normalized =
            paintcraft_document::import_image(width, height, &decoded.rgba, &key.hex()).unwrap();
        let legacy = paintcraft_document::import_image(
            width,
            height,
            &decoded.rgba,
            &hex(&Sha256::digest(&bytes)),
        )
        .unwrap();
        assert_ne!(normalized.document_id, legacy.document_id, "{name}");
        assert_eq!(
            image::decode_with_pixel_limit(&bytes, pixels - 1)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit,
            "{name}"
        );
    }
}
