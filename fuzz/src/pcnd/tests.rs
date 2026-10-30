use super::*;
use crate::{Outcome, Target, classify};
use paintcraft_core::types::json;
use std::{cell::Cell, path::Path};
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("pcnd-fixtures")
            .join(name),
    )
    .unwrap()
}
fn original(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures")
            .join(name),
    )
    .unwrap()
}
#[test]
fn independent_and_derived_pcnd_seeds_reach_public_reader_with_exact_outcomes() {
    let manifest = json::parse(include_bytes!("../../pcnd-seeds.json")).unwrap();
    let rows = manifest.get("seeds").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), 14);
    for row in rows {
        let source = row.get("source").unwrap().as_str().unwrap();
        let bytes = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join(source),
        )
        .unwrap();
        let result = crate::exercise(Target::Pcnd, &bytes);
        let detail = format!("{result:?}");
        assert_eq!(
            classify(result),
            if row.get("accepted").unwrap().as_bool().unwrap() {
                Outcome::Accepted
            } else {
                Outcome::Rejected
            },
            "{source}: {detail}"
        );
    }
}
#[test]
fn huge_sparse_logical_layer_is_not_charged_as_materialized_pixels() {
    let bytes = fixture("huge-default-only.pcnd");
    let budget = SharedBudget::new(METADATA_BYTES_MAX);
    let doc = pcnd::read_with_metadata_admission_checked(
        &bytes,
        PIXELS_MAX,
        STAGED_PIXELS_MAX,
        8,
        pcnd::MetadataAdmission {
            bytes: METADATA_BYTES_MAX,
            budget: &budget,
        },
        &|| Ok(()),
    )
    .unwrap();
    let layer = &doc.layers[&doc.roots[0]];
    assert_eq!(
        (layer.raster.width(), layer.raster.height()),
        (1_048_576, 1_048_576)
    );
    assert_eq!(layer.raster.resident_tiles(), 0);
    assert_eq!(layer.raster.get(0, 0).0, [13, 71, 201, 0]);
    assert!(exercise(&bytes, &|| Ok(())).is_ok());
    assert!(exercise(&fixture("two-tiles.pcnd"), &|| Ok(())).is_ok());
    assert_eq!(
        exercise(&fixture("three-tiles.pcnd"), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn source_attachment_admission_and_cancellation_are_bounded_and_fallible() {
    let bytes = original("rgb-profile.pcnd");
    let denied = SharedBudget::new(0);
    assert_eq!(
        pcnd::read_with_metadata_admission_checked(
            &bytes,
            PIXELS_MAX,
            STAGED_PIXELS_MAX,
            8,
            pcnd::MetadataAdmission {
                bytes: METADATA_BYTES_MAX,
                budget: &denied
            },
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(denied.usage().peak, 0);
    let calls = Cell::new(0);
    exercise(&bytes, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in 1..=calls.get() {
        let at = Cell::new(0);
        assert_eq!(
            exercise(&bytes, &|| {
                at.set(at.get() + 1);
                if at.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel PCND fuzz control",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
    }
    assert_eq!(
        crate::exercise(Target::Pcnd, &vec![0; INPUT_MAX + 1])
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn bounded_raw_sink_rejects_capacity_without_partial_append() {
    let mut output = Output {
        bytes: vec![0; OUTPUT_MAX],
        check: &|| Ok(()),
        capacity_denied: false,
        error: None,
    };
    assert!(output.write(&[1]).is_err());
    assert!(output.capacity_denied);
    assert_eq!(output.bytes.len(), OUTPUT_MAX);
}

#[test]
fn wrapper_checksum_branch_reaches_body_without_repairing_inner_attachment() {
    let source = original("legacy-v1.pcnd");
    let mut changed = source.clone();
    changed[8] ^= 1; // A different valid document ID, but stale outer checksum.
    assert!(crate::exercise(Target::Pcnd, &changed).is_err());
    assert!(crate::exercise_pcnd_checksum_aware(&changed).is_ok());
    let repaired = repair_outer_sha(&changed, &|| Ok(())).unwrap();
    assert_eq!(
        &repaired[..repaired.len() - 32],
        &changed[..changed.len() - 32]
    );
    assert_ne!(repaired, changed);
    assert_eq!(source, original("legacy-v1.pcnd"));
    // This fixture already has correct outer SHA but a bad inner attachment SHA.
    // The wrapper must leave that genuine parser rejection intact.
    let inner = fixture("inner-digest-mismatch.pcnd");
    assert_eq!(repair_outer_sha(&inner, &|| Ok(())).unwrap(), inner);
    assert!(crate::exercise_pcnd_checksum_aware(&inner).is_err());
    assert_eq!(
        repair_outer_sha(&[0; 31], &|| Ok(())).unwrap_err().code,
        ErrorCode::CodecFailed
    );
    assert_eq!(
        repair_outer_sha(&vec![0; INPUT_MAX + 1], &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        repair_outer_sha(&source, &|| Err(Error::new(
            ErrorCode::OperationCancelled,
            "stop outer hash"
        )))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
}

fn filter_fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("crates/services/paintcraft-formats/tests/pcnd-filter-fixtures")
            .join(name),
    )
    .unwrap()
}
#[test]
fn separately_inventoried_pcnd3_reader_seeds_keep_exact_outcomes_and_roundtrip() {
    let manifest = json::parse(include_bytes!("../../pcnd-filter-seeds.json")).unwrap();
    let rows = manifest.get("seeds").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), 16);
    for row in rows {
        let name = row.get("name").unwrap().as_str().unwrap();
        let bytes = filter_fixture(name);
        let result = crate::exercise(Target::Pcnd, &bytes);
        let detail = format!("{result:?}");
        assert_eq!(
            classify(result),
            if row.get("accepted").unwrap().as_bool().unwrap() {
                Outcome::Accepted
            } else {
                Outcome::Rejected
            },
            "{name}: {detail}"
        );
    }
}
#[test]
fn pcnd3_disabled_stack_and_original_pixels_survive_real_reader_writer_and_cancellation() {
    use paintcraft_core::types::filters::Descriptor;
    for name in ["stack.pcnd", "source-stack.pcnd", "disabled.pcnd"] {
        let bytes = filter_fixture(name);
        let budget = SharedBudget::new(METADATA_BYTES_MAX);
        let doc = pcnd::read_with_metadata_admission_checked(
            &bytes,
            PIXELS_MAX,
            STAGED_PIXELS_MAX,
            8,
            pcnd::MetadataAdmission {
                bytes: METADATA_BYTES_MAX,
                budget: &budget,
            },
            &|| Ok(()),
        )
        .unwrap();
        let layer = &doc.layers[&doc.roots[0]];
        let stack = &layer.filters;
        assert_eq!(layer.raster.get(0, 0).0, [17, 32, 255, 0]);
        assert_eq!(layer.raster.get(1, 0).0, [0, 127, 200, 255]);
        assert_eq!(stack.as_slice()[0].id().as_u128(), 0);
        assert!(!stack.as_slice()[0].enabled());
        if name == "disabled.pcnd" {
            assert_eq!(stack.len(), 1);
            assert_eq!(stack.as_slice()[0].descriptor(), Descriptor::InvertRgbV1);
        } else {
            assert_eq!(stack.len(), 3);
            assert_eq!(stack.as_slice()[1].descriptor(), Descriptor::InvertRgbV1);
            let Descriptor::BrightnessContrastV1(p) = stack.as_slice()[0].descriptor() else {
                panic!("BC")
            };
            assert_eq!((p.brightness(), p.contrast()), (-100, 99));
        }
        let raw = pcnd::write(&doc).unwrap();
        assert_eq!(raw[4], 3, "even all-disabled stack stays PCND3");
        assert_eq!(
            raw, bytes,
            "independent canonical specimen must remain byteexact"
        );
        let calls = Cell::new(0);
        exercise(&bytes, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        for stop in 1..=calls.get() {
            let at = Cell::new(0);
            assert_eq!(
                exercise(&bytes, &|| {
                    at.set(at.get() + 1);
                    if at.get() == stop {
                        Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "cancel PCND3 fuzz control",
                        ))
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err()
                .code,
                ErrorCode::OperationCancelled
            );
        }
        assert_eq!(bytes, filter_fixture(name));
        drop(doc);
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn independent_pcnd4_mask_seeds_reach_admitted_decoder_and_preserve_roundtrip() {
    let manifest = json::parse(include_bytes!("../../pcnd-mask-seeds.json")).unwrap();
    let rows = manifest.get("seeds").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), 24);
    for row in rows {
        let source = row.get("source").unwrap().as_str().unwrap();
        let bytes = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join(source),
        )
        .unwrap();
        let result = crate::exercise(Target::Pcnd, &bytes);
        let detail = format!("{result:?}");
        assert_eq!(
            classify(result),
            if row.get("accepted").unwrap().as_bool().unwrap() {
                Outcome::Accepted
            } else {
                Outcome::Rejected
            },
            "{source}:{detail}"
        );
    }
}
#[test]
fn mask_roundtrip_uses_bounded_case_realm_and_cancellation_is_not_swallowed() {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("crates/services/paintcraft-formats/tests/pcnd-mask-fixtures/repeated.pcnd"),
    )
    .unwrap();
    let count = Cell::new(0);
    exercise(&bytes, &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    for stop in [1, count.get() / 2, count.get()] {
        let seen = Cell::new(0);
        assert_eq!(
            exercise(&bytes, &|| {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "mask roundtrip stop",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
    }
    let metadata = SharedBudget::new(METADATA_BYTES_MAX);
    let coverage = SharedBudget::new(0);
    assert_eq!(
        pcnd::read_with_admission_checked(
            &bytes,
            PIXELS_MAX,
            STAGED_PIXELS_MAX,
            LAYERS_MAX,
            admission(&metadata, &coverage),
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(coverage.activity(), 0);
    assert_eq!(metadata.usage().reserved, 0);
}
