use super::*;
use std::cell::Cell;
fn boxed(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&((8 + data.len()) as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out
}
fn ispe(width: u32, height: u32) -> Vec<u8> {
    boxed(
        b"ispe",
        &[&[0; 4][..], &width.to_be_bytes(), &height.to_be_bytes()].concat(),
    )
}
fn fixture(properties: &[u8]) -> Vec<u8> {
    [
        boxed(b"ftyp", b"avif\0\0\0\0avifmif1"),
        boxed(
            b"meta",
            &[&[0; 4][..], &boxed(b"iprp", &boxed(b"ipco", properties))].concat(),
        ),
        boxed(b"mdat", &[0]),
    ]
    .concat()
}
fn basic() -> Vec<u8> {
    fixture(&[ispe(4, 3), boxed(b"av1C", &[0x81, 0, 0, 0])].concat())
}
#[test]
fn declarations_are_indexed_not_guessed_to_be_the_primary_image() {
    let bytes = fixture(
        &[
            ispe(4, 3),
            boxed(b"auxC", b"not interpreted"),
            ispe(2, 2),
            boxed(b"av1C", &[0x81, 0, 0, 0]),
        ]
        .concat(),
    );
    let got = inspect_container(&bytes, 12, &|| Ok(())).unwrap();
    assert_eq!(got.property_count, 4);
    assert_eq!(
        got.spatial_extents,
        [
            SpatialExtent {
                property_index: 1,
                width: 4,
                height: 3
            },
            SpatialExtent {
                property_index: 3,
                width: 2,
                height: 2
            }
        ]
    );
    assert_eq!(got.av1_configurations[0].property_index, 4);
    assert_eq!(got.av1_configurations[0].bit_depth, 8);
    // No iloc/pitm/ipma or sequence-header validation is claimed or performed.
}
#[test]
fn actual_existing_opaque_and_alpha_writers_can_be_physically_inspected() {
    for alpha in [255, 128] {
        let bytes = encode_rgba8(4, 3, &[80, 90, 100, alpha].repeat(12), 100, &|| Ok(()))
            .unwrap()
            .bytes;
        let got = inspect_container(&bytes, INSPECT_PIXELS_MAX, &|| Ok(())).unwrap();
        assert!(
            got.spatial_extents
                .iter()
                .all(|p| (p.width, p.height) == (4, 3))
        );
        assert!(!got.av1_configurations.is_empty());
        // Alpha inspection is not alpha decoder admission or an enabled route.
    }
}
#[test]
fn every_truncation_and_checked_dimension_allowance_fails_closed() {
    let bytes = basic();
    for length in 0..bytes.len() {
        assert!(
            inspect_container(&bytes[..length], 12, &|| Ok(())).is_err(),
            "{length}"
        );
    }
    assert_eq!(
        inspect_container(&bytes, 11, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    for (width, height, code) in [
        (0, 3, ErrorCode::CodecFailed),
        (u32::MAX, u32::MAX, ErrorCode::ResourceLimit),
        (65536, 65, ErrorCode::ResourceLimit),
    ] {
        let bytes = fixture(&[ispe(width, height), boxed(b"av1C", &[0x81, 0, 0, 0])].concat());
        assert_eq!(
            inspect_container(&bytes, u64::MAX, &|| Ok(()))
                .unwrap_err()
                .code,
            code
        );
    }
}
#[test]
fn incompatible_sequences_duplicate_containers_and_bad_config_are_refused() {
    let mut sequence = basic();
    sequence[8..12].copy_from_slice(b"avis");
    assert_eq!(
        inspect_container(&sequence, 12, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let mut movie = basic();
    movie.extend(boxed(b"moov", &[]));
    assert!(inspect_container(&movie, 12, &|| Ok(())).is_err());
    let mut duplicate = basic();
    duplicate.extend(boxed(b"meta", &[0; 4]));
    assert!(inspect_container(&duplicate, 12, &|| Ok(())).is_err());
    for config in [
        &[][..],
        &[0x80, 0, 0, 0],
        &[0x81, 0, 0x20, 0],
        &[0x81, 0, 0, 1],
        &[0x81, 0xe0, 0, 0],
    ] {
        let bytes = fixture(&[ispe(4, 3), boxed(b"av1C", config)].concat());
        assert!(inspect_container(&bytes, 12, &|| Ok(())).is_err());
    }
}
#[test]
fn cancellation_and_collection_budgets_stop_inspection() {
    let bytes = basic();
    for stop in 1..=8 {
        let calls = Cell::new(0);
        let error = inspect_container(&bytes, 12, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
    let bytes = fixture(
        &[
            ispe(4, 3),
            boxed(b"av1C", &[0x81, 0, 0, 0]),
            boxed(b"free", &[]).repeat(1024),
        ]
        .concat(),
    );
    assert_eq!(
        inspect_container(&bytes, 12, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let bytes = fixture(
        &[
            ispe(4, 3),
            boxed(b"av1C", &[0x81, 0, 0, 0]),
            boxed(b"free", &vec![0; METADATA_BYTES_MAX]),
        ]
        .concat(),
    );
    assert_eq!(
        inspect_container(&bytes, 12, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}
