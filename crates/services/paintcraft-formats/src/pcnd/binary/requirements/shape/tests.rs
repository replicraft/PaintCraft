use super::*;
use std::cell::Cell;

#[test]
fn writer_versions_one_through_six_have_exact_wire_count_hash_and_cancel() {
    use paintcraft_core::types::{
        Rect,
        filters::{BoxBlurV1, Descriptor, VibranceRgbV1},
    };
    use paintcraft_document::{
        FilterInstance, FilterMask, FilterStack,
        source_metadata::{
            BYTES_MAX, OrientationPolicy, SourceColorModel, SourceDescriptor, SourceFormat,
            SourceMetadata,
        },
    };
    use paintcraft_raster::coverage_mask::{A8Mask, Limits};
    let base = paintcraft_document::import_image(1, 1, &[17, 33, 65, 255], "shape").unwrap();
    let budget = SharedBudget::new(8192);
    for version in 1..=6 {
        let mut doc = base.clone();
        if version == 2 {
            doc.source_metadata = Some(
                SourceMetadata::capture(
                    SourceDescriptor {
                        format: SourceFormat::Png,
                        source_model: SourceColorModel::Rgb,
                        source_sha256: [9; 32],
                        source_bytes: 1,
                        selection: None,
                        source_dimensions: (1, 1),
                        output_dimensions: (1, 1),
                        orientation: None,
                        orientation_policy: OrientationPolicy::Unmodified,
                        resolution: None,
                    },
                    None,
                    BYTES_MAX,
                    &budget,
                    &|| Ok(()),
                )
                .unwrap(),
            );
        }
        if version >= 3 {
            let descriptor = match version {
                5 => Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap()),
                6 => Descriptor::VibranceRgbV1(VibranceRgbV1::new(0).unwrap()),
                _ => Descriptor::InvertRgbV1,
            };
            let mut instance = FilterInstance::new(Uuid::from_u128(71), false, descriptor);
            if version == 4 {
                instance = instance.with_mask(Some(FilterMask::new(
                    false,
                    A8Mask::uniform(
                        Rect::new(0, 0, 1, 1).unwrap(),
                        0,
                        &budget,
                        Limits::default(),
                        &|| Ok(()),
                    )
                    .unwrap(),
                )));
            }
            doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
                FilterStack::new(vec![instance]).unwrap();
        }
        let bytes = crate::pcnd::write(&doc).unwrap();
        assert_eq!(bytes[4], version);
        let (verified, whole) =
            CheckedContainer::verify_with_source_hash(&bytes, &|| Ok(())).unwrap();
        assert_eq!(whole, <[u8; 32]>::from(Sha256::digest(&bytes)));
        let calls = Cell::new(0);
        assert_eq!(
            verified
                .snapshot_tile_records_checked(&|| {
                    calls.set(calls.get() + 1);
                    Ok(())
                })
                .unwrap(),
            1,
            "version {version}"
        );
        for stop in 1..=calls.get() {
            let seen = Cell::new(0);
            let error = verified
                .snapshot_tile_records_checked(&|| {
                    seen.set(seen.get() + 1);
                    if seen.get() == stop {
                        Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "version shape cancelled",
                        ))
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err();
            assert_eq!(
                error.code,
                ErrorCode::OperationCancelled,
                "version {version}"
            );
        }
    }
}

fn number(bytes: &mut Vec<u8>, mut n: u64) {
    while n >= 128 {
        bytes.push((n as u8 & 127) | 128);
        n >>= 7;
    }
    bytes.push(n as u8);
}
fn seal(mut bytes: Vec<u8>) -> Vec<u8> {
    let digest = Sha256::digest(&bytes);
    bytes.extend_from_slice(&digest);
    bytes
}
// Independent tiny PCND6 framing, including a physically present raw default
// tile. The normal writer omits that record, so it cannot construct this case.
fn specimen(encoding: u8, payload: &[u8]) -> Vec<u8> {
    let mut prelude = vec![1, 15];
    prelude.extend_from_slice(b"vibrance-rgb-v1");
    prelude.push(9);
    prelude.extend_from_slice(b"params-v1");
    let mut body = b"PCND\x06\r\n\x1a".to_vec();
    number(&mut body, prelude.len() as u64);
    body.extend(prelude);
    body.extend([1; 16]);
    body.extend([2; 16]);
    body.extend([1, 1, 0, 0, 1]); // Canvas, absent title/source, one layer.
    body.extend([3; 16]);
    body.extend([1, b'x', 1, 255, 0, 0, 1, 1, 0, 0, 0, 0, 1]);
    body.extend([4; 16]);
    body.extend([1, 0, 2, 0, 0, 0]); // Enabled/index/params2/amount0/mask absent.
    if encoding != 0 {
        body.extend([encoding, 0, 0]);
        number(&mut body, payload.len() as u64);
        body.extend_from_slice(payload);
    }
    body.push(0);
    seal(body)
}

#[test]
fn source_hash_and_wire_count_reuse_one_integrity_traversal_and_retain_explicit_default() {
    let bytes = specimen(1, &[0; 4]);
    PAYLOAD_HASH_PASSES.with(|passes| passes.set(0));
    let (verified, whole) = CheckedContainer::verify_with_source_hash(&bytes, &|| Ok(())).unwrap();
    assert_eq!(whole, <[u8; 32]>::from(Sha256::digest(&bytes)));
    assert_eq!(
        verified.snapshot_tile_records_checked(&|| Ok(())).unwrap(),
        1
    );
    let budget = SharedBudget::new(1 << 20);
    let doc = verified
        .read_with_admission_checked(
            1,
            65_536,
            1,
            ReadAdmission {
                metadata: MetadataAdmission {
                    bytes: 0,
                    budget: &budget,
                },
                coverage: CoverageAdmission {
                    bytes: 0,
                    budget: &budget,
                    limits: Default::default(),
                },
            },
            &|| Ok(()),
        )
        .unwrap();
    assert_eq!(doc.layers[&doc.roots[0]].raster.resident_tiles(), 1);
    PAYLOAD_HASH_PASSES.with(|passes| assert_eq!(passes.get(), 1));
    let written = crate::pcnd::write_compact(&doc).unwrap();
    let canonical = CheckedContainer::verify(&written, &|| Ok(())).unwrap();
    assert_eq!(
        canonical.snapshot_tile_records_checked(&|| Ok(())).unwrap(),
        0
    );
}

#[test]
fn shape_skips_cold_encoded_payload_without_decoder_and_checks_bad_geometry_and_trailing_data() {
    // Deliberately invalid Zstd: structural assessment counts without inflation;
    // later typed adoption remains responsible for codec validity.
    let bytes = specimen(3, &[255]);
    let verified = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    assert_eq!(
        verified.snapshot_tile_records_checked(&|| Ok(())).unwrap(),
        1
    );
    let mut bad = specimen(1, &[0; 4]);
    bad.truncate(bad.len() - 32);
    // First canvas dimension follows manifest and the two UUIDs.
    let offset = 8 + 1 + 1 + 1 + 15 + 1 + 9 + 32;
    bad[offset] = 0;
    let bad = seal(bad);
    assert!(
        CheckedContainer::verify(&bad, &|| Ok(()))
            .unwrap()
            .snapshot_tile_records_checked(&|| Ok(()))
            .is_err()
    );
    let mut tail = specimen(0, &[]);
    tail.truncate(tail.len() - 32);
    tail.push(0);
    let tail = seal(tail);
    assert!(
        CheckedContainer::verify(&tail, &|| Ok(()))
            .unwrap()
            .snapshot_tile_records_checked(&|| Ok(()))
            .is_err()
    );
}

#[test]
fn bad_trailer_precedes_callbacks_and_every_structural_checkpoint_cancels_without_adoption() {
    let mut bad = specimen(1, &[0; 4]);
    *bad.last_mut().unwrap() ^= 1;
    let calls = Cell::new(0);
    assert!(
        CheckedContainer::verify_with_source_hash(&bad, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .is_err()
    );
    assert_eq!(calls.get(), 1); // One body chunk, no structural walk.
    let bytes = specimen(1, &[0; 4]);
    let verified = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    calls.set(0);
    verified
        .snapshot_tile_records_checked(&|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    let count = calls.get();
    assert!(count > 4);
    for stop in 1..=count {
        calls.set(0);
        let error = verified
            .snapshot_tile_records_checked(&|| {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "shape cancelled"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(verified.bytes(), bytes);
    }
}
