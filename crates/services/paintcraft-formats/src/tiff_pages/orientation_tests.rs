use super::*;
use paintcraft_core::types::json;

fn fixtures() -> json::Value {
    json::parse(include_bytes!(
        "../../tests/tiff-orientation-fixtures/oracle.json"
    ))
    .unwrap()
}
fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/tiff-orientation-fixtures")
            .join(name),
    )
    .unwrap()
}

#[test]
fn ordinary_single_image_facade_normalizes_both_byte_orders_without_widening_multipage_or_graya() {
    for fixture in fixtures().get("fixtures").unwrap().as_array().unwrap() {
        let name = fixture.get("file").unwrap().as_str().unwrap();
        let input = bytes(name);
        let pages = fixture.get("pages").unwrap().as_array().unwrap();
        if pages.len() != 1 || fixture.get("gray").unwrap().as_bool() == Some(true) {
            assert!(crate::image::decode(&input).is_err(), "{name}");
            continue;
        }
        let page = &pages[0];
        let rgba: Vec<u8> = page
            .get("rgba")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as u8)
            .collect();
        let decoded = crate::image::decode(&input).unwrap();
        assert_eq!(
            decoded.width,
            page.get("width").unwrap().as_i64().unwrap() as u32
        );
        assert_eq!(
            decoded.height,
            page.get("height").unwrap().as_i64().unwrap() as u32
        );
        assert_eq!(decoded.rgba, rgba, "{name}");
        assert!(
            decoded.losses.is_empty(),
            "{name}: applying orientation is no metadata omission"
        );
        let key = crate::image::document_conversion_key(&input, &decoded, &|| Ok(())).unwrap();
        assert_ne!(
            key.unwrap().digest(),
            <[u8; 32]>::from(Sha256::digest(&input))
        );
        assert_eq!(
            key,
            crate::image::document_conversion_key(&input, &decoded, &|| Ok(())).unwrap()
        );
    }
}

#[test]
fn explicit_coordinate_oracles_normalize_all_eight_orientations_and_bind_source_facts() {
    for fixture in fixtures().get("fixtures").unwrap().as_array().unwrap() {
        let name = fixture.get("file").unwrap().as_str().unwrap();
        let source = bytes(name);
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&source)),
            fixture.get("sha256").unwrap().as_str().unwrap()
        );
        let inventory = inspect(&source, &|| Ok(())).unwrap();
        for (index, expected) in fixture
            .get("pages")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let orientation = expected.get("orientation").unwrap().as_i64().unwrap() as u32;
            let width = expected.get("width").unwrap().as_i64().unwrap() as u32;
            let height = expected.get("height").unwrap().as_i64().unwrap() as u32;
            let rgba: Vec<u8> = expected
                .get("rgba")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as u8)
                .collect();
            let page = &inventory.pages()[index];
            assert_eq!((page.width, page.height), (3, 2));
            assert_eq!(page.descriptor.orientation, orientation);
            assert!(page.unsupported_reason.is_none(), "{name} {index}");
            let decoded = decode_page(&source, index as u16, PIXELS_MAX, &|| Ok(())).unwrap();
            assert_eq!(
                (decoded.width, decoded.height),
                (width, height),
                "{name} {index}"
            );
            assert_eq!(decoded.rgba, rgba, "{name} {index}");
            let report = decoded.assessment;
            assert_eq!(report.source_dimensions(), (3, 2));
            assert_eq!(report.descriptor().orientation, orientation);
            assert_eq!(report.normalized_orientation(), 1);
            assert_eq!(
                report.output_sha256(),
                <[u8; 32]>::from(Sha256::digest(&rgba))
            );
            assert_eq!(Assessment::decode(&report.encode()).unwrap(), report);
            report
                .validate_result(
                    inventory.source_sha256(),
                    source.len() as u64,
                    index as u16,
                    width,
                    height,
                    &rgba,
                )
                .unwrap();
            if orientation >= 5 {
                assert!(
                    report
                        .validate_result(
                            inventory.source_sha256(),
                            source.len() as u64,
                            index as u16,
                            3,
                            2,
                            &rgba
                        )
                        .is_err()
                );
            }
            let mut legacy = report.encode();
            legacy[..8].copy_from_slice(b"PCTIFF01");
            legacy[8..10].copy_from_slice(&1u16.to_be_bytes());
            assert!(Assessment::decode(&legacy).is_err());
            let mut changed = report.encode();
            changed[183] = if orientation == 1 { 2 } else { 1 };
            let changed = Assessment::decode(&changed).unwrap();
            assert_ne!(changed.document_identity(), report.document_identity());
            assert_ne!(report.document_identity(), report.source_sha256());
            assert_eq!(
                decode_page(&source, index as u16, PIXELS_MAX, &|| Ok(()))
                    .unwrap()
                    .assessment
                    .document_identity(),
                report.document_identity()
            );
        }
    }
}

#[test]
fn orientation_declaration_must_be_single_short_in_range_and_cancellation_remains_effective() {
    let original = bytes("rgba8-le-orientation-6.tif");
    let at = (0..13)
        .map(|index| 10 + index * 12)
        .find(|at| u16::from_le_bytes(original[*at..*at + 2].try_into().unwrap()) == 274)
        .unwrap();
    for (kind, count, value) in [(4u16, 1u32, 6u16), (3, 2, 6), (3, 1, 0), (3, 1, 9)] {
        let mut source = original.clone();
        source[at + 2..at + 4].copy_from_slice(&kind.to_le_bytes());
        source[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
        source[at + 8..at + 10].copy_from_slice(&value.to_le_bytes());
        assert!(crate::image::decode(&source).is_err());
        assert!(
            inspect(&source, &|| Ok(())).unwrap().pages()[0]
                .unsupported_reason
                .is_some()
        );
        assert!(decode_page(&source, 0, PIXELS_MAX, &|| Ok(())).is_err());
    }
    let calls = std::cell::Cell::new(0);
    let completed = decode_page(&original, 0, PIXELS_MAX, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert_eq!((completed.width, completed.height), (2, 3));
    for stop in [1, calls.get() / 2, calls.get()] {
        let seen = std::cell::Cell::new(0);
        let error = decode_page(&original, 0, PIXELS_MAX, &|| {
            seen.set(seen.get() + 1);
            if seen.get() >= stop {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "orientation cancelled",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
}
