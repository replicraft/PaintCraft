//! New registry corpus stays independent of all historical seed manifests.
use super::*;
use crate::{Outcome, Target, classify};
use paintcraft_core::types::json::{self, Value};
use std::{cell::Cell, path::Path};
const BASE: &str = "crates/services/paintcraft-formats/tests/pcnd-point-fixtures/";
fn source(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(BASE)
            .join(name),
    )
    .unwrap()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
#[test]
fn independent_29_point_seeds_keep_exact_raw_and_outer_only_outcomes() {
    let manifest = json::parse(include_bytes!("../../pcnd-point-seeds.json")).unwrap();
    let rows = manifest.get("seeds").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), 29);
    let mut positive = 0;
    for row in rows {
        let name = row.get("name").unwrap().as_str().unwrap();
        let bytes = source(name);
        assert_eq!(
            bytes.len() as i64,
            row.get("bytes").unwrap().as_i64().unwrap()
        );
        assert_eq!(
            hex(&Sha256::digest(&bytes)),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        let expected = if row.get("accepted").unwrap().as_bool().unwrap() {
            positive += 1;
            Outcome::Accepted
        } else {
            Outcome::Rejected
        };
        assert_eq!(
            classify(crate::exercise(Target::Pcnd, &bytes)),
            expected,
            "raw {name}"
        );
        assert_eq!(
            classify(crate::exercise_pcnd_checksum_aware(&bytes)),
            expected,
            "outer only {name}"
        );
        if expected == Outcome::Rejected {
            let expected = if name.starts_with("unknown-") {
                ErrorCode::CapabilityUnsupported
            } else {
                ErrorCode::CodecFailed
            };
            assert_eq!(
                exercise(&bytes, &|| Ok(())).unwrap_err().code,
                expected,
                "{name}"
            );
        }
    }
    assert_eq!(positive, 23);
}
#[test]
fn point_seed_storage_matches_independent_descriptors_masks_hidden_rgb_and_attachment() {
    let oracle = json::parse(include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/pcnd-point-fixtures/oracle.json"
    ))
    .unwrap();
    let mut count = 0;
    for row in oracle.get("fixtures").unwrap().as_array().unwrap() {
        if row.get("accepted").unwrap().as_bool() != Some(true) {
            continue;
        }
        count += 1;
        let bytes = source(row.get("file").unwrap().as_str().unwrap());
        let metadata = SharedBudget::new(METADATA_BYTES_MAX);
        let coverage = SharedBudget::new(2 * MASK_BYTES_MAX);
        let doc = pcnd::read_with_admission_checked(
            &bytes,
            PIXELS_MAX,
            STAGED_PIXELS_MAX,
            LAYERS_MAX,
            admission(&metadata, &coverage),
            &|| Ok(()),
        )
        .unwrap();
        let layer = &doc.layers[&doc.roots[0]];
        let raw: Vec<_> = (0..2).flat_map(|x| layer.raster.get(x, 0).0).collect();
        let expected: Vec<_> = row
            .get("raw_rgba")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|n| u8::try_from(n.as_i64().unwrap()).unwrap())
            .collect();
        assert_eq!(raw, expected);
        assert_eq!(
            hex(&layer.filters.digest()),
            row.get("stack_sha256").unwrap().as_str().unwrap()
        );
        let filters = row.get("filters").unwrap().as_array().unwrap();
        let digests = row.get("mask_sha256").unwrap().as_array().unwrap();
        assert_eq!(layer.filters.len(), filters.len());
        for ((filter, expected), digest) in layer.filters.iter().zip(filters).zip(digests) {
            assert_eq!(
                filter.id().to_string(),
                expected.get("id").unwrap().as_str().unwrap()
            );
            assert_eq!(
                filter.enabled(),
                expected.get("enabled").unwrap().as_bool().unwrap()
            );
            assert_eq!(
                filter.descriptor().to_value(),
                *expected.get("descriptor").unwrap()
            );
            match filter.mask() {
                None => assert!(matches!(digest, Value::Null)),
                Some(mask) => {
                    assert!(mask.enabled());
                    assert_eq!(hex(&mask.coverage().digest()), digest.as_str().unwrap());
                    assert_eq!(mask.coverage().coverage(0, 0), 128);
                    // Independent generator declares 2x1 but stores only the
                    // left pixel's run; declared domain is not full coverage.
                    assert_eq!(
                        mask.coverage().domain(),
                        paintcraft_core::types::Rect::new(0, 0, 2, 1).unwrap()
                    );
                    assert_eq!(mask.coverage().coverage(1, 0), 0);
                    assert_eq!(mask.coverage().stats().bands, 1);
                    assert_eq!(mask.coverage().stats().runs, 1);
                }
            }
        }
        assert_eq!(
            doc.source_metadata.is_some(),
            row.get("attachment").unwrap().as_bool().unwrap()
        );
        if let Some(source) = &doc.source_metadata {
            assert_eq!(source.profile().unwrap().len(), 3024);
        }
        drop(doc);
        assert_eq!(metadata.usage().reserved, 0);
        assert_eq!(coverage.usage().reserved, 0);
    }
    assert_eq!(count, 23);
}
#[test]
fn point_profiles_cancellation_quota_and_checksum_body_route_remain_bounded() {
    for name in ["source-all-six.pcnd", "tag3-masked.pcnd"] {
        let bytes = source(name);
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
                            "point fuzz cancellation",
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
        assert_eq!(bytes, source(name));
    }
    let source_all = source("source-all-six.pcnd");
    let denied = SharedBudget::new(0);
    let allowed = SharedBudget::new(2 * MASK_BYTES_MAX);
    assert_eq!(
        pcnd::read_with_admission_checked(
            &source_all,
            PIXELS_MAX,
            STAGED_PIXELS_MAX,
            LAYERS_MAX,
            admission(&denied, &allowed),
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(denied.usage().peak, 0);
    assert_eq!(allowed.usage().reserved, 0);
    let masked = source("tag3-masked.pcnd");
    let metadata = SharedBudget::new(METADATA_BYTES_MAX);
    assert_eq!(
        pcnd::read_with_admission_checked(
            &masked,
            PIXELS_MAX,
            STAGED_PIXELS_MAX,
            LAYERS_MAX,
            admission(&metadata, &denied),
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(denied.usage().peak, 0);
    assert_eq!(metadata.usage().reserved, 0);
    let mut changed = source_all.clone();
    changed[8] ^= 1;
    assert!(crate::exercise(Target::Pcnd, &changed).is_err());
    assert!(crate::exercise_pcnd_checksum_aware(&changed).is_ok());
    assert_eq!(source_all, source("source-all-six.pcnd"));
}
