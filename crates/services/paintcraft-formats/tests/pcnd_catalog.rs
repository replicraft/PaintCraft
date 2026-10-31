//! Independent planned bytes become conformance controls, without product oracles.
use paintcraft_core::types::{
    ErrorCode, Result,
    json::{self, Value},
    resources::SharedBudget,
};
use paintcraft_document::Document;
use paintcraft_formats::pcnd;
use sha2::{Digest, Sha256};
use std::cell::Cell;
fn rows() -> Vec<Value> {
    json::parse(include_bytes!("pcnd-catalog-fixtures/oracle.json"))
        .unwrap()
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec()
}
fn unhex(value: &str) -> Vec<u8> {
    let (pairs, trailing) = value.as_bytes().as_chunks::<2>();
    assert!(trailing.is_empty(), "incomplete independent hex fixture");
    pairs
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn fixture(row: &Value) -> Vec<u8> {
    unhex(row.get("hex").unwrap().as_str().unwrap())
}
fn open(bytes: &[u8], budget: &SharedBudget, check: &dyn Fn() -> Result<()>) -> Result<Document> {
    pcnd::read_with_admission_checked(
        bytes,
        2,
        65536,
        1,
        pcnd::ReadAdmission {
            metadata: pcnd::MetadataAdmission {
                bytes: 1 << 20,
                budget,
            },
            coverage: pcnd::CoverageAdmission {
                bytes: 1 << 20,
                budget,
                limits: Default::default(),
            },
        },
        check,
    )
}
#[test]
fn independent_catalog_bytes_preserve_complete_intent_and_raw_compact_roundtrips() {
    let mut count = 0;
    for row in rows()
        .iter()
        .filter(|r| r.get("accepted_planned").unwrap().as_bool() == Some(true))
    {
        count += 1;
        let name = row.get("name").unwrap().as_str().unwrap();
        let bytes = fixture(row);
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&bytes)),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        let budget = SharedBudget::new(4 << 20);
        let doc = open(&bytes, &budget, &|| Ok(())).unwrap_or_else(|e| panic!("{name}: {e}"));
        let layer = &doc.layers[&doc.roots[0]];
        let raw: Vec<u8> = (0..2).flat_map(|x| layer.raster.get(x, 0).0).collect();
        let expected: Vec<u8> = row
            .get("raw_layer_rgba")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as u8)
            .collect();
        assert_eq!(raw, expected, "{name}");
        assert_eq!(
            paintcraft_document::hex(&layer.filters.digest()),
            row.get("stack_digest_v3").unwrap().as_str().unwrap(),
            "{name}"
        );
        assert_eq!(
            pcnd::write(&doc).unwrap(),
            bytes,
            "canonical independent {name}"
        );
        let compact = pcnd::write_compact(&doc).unwrap();
        assert_eq!(compact[4], 6);
        let restored = open(&compact, &budget, &|| Ok(())).unwrap();
        assert!(doc.same_semantics(&restored), "{name}");
        pcnd::check_metadata_admission(&doc, &|| Ok(())).unwrap();
        pcnd::check_compact_metadata_admission(&doc, &|| Ok(())).unwrap();
        let manifest = pcnd::catalog_manifest(&bytes).unwrap();
        let expected = row.get("manifest").unwrap().as_array().unwrap();
        assert_eq!(manifest.len(), expected.len());
        for (actual, expected) in manifest.iter().zip(expected) {
            assert_eq!(
                actual.profile_id,
                expected.get("profile").unwrap().as_str().unwrap()
            );
            assert_eq!(
                actual.codec_id,
                expected.get("codec").unwrap().as_str().unwrap()
            );
        }
        let header = pcnd::probe(&bytes).unwrap();
        header.require_supported().unwrap();
        assert_eq!(header.container_version, 6);
        assert!(
            header
                .required_features
                .iter()
                .any(|f| f == pcnd::LAYER_CATALOG_FEATURE)
        );
        let mut missing = header;
        missing
            .required_features
            .retain(|f| f != pcnd::LAYER_CATALOG_FEATURE);
        assert_eq!(
            missing.require_supported().unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
    assert_eq!(count, 8);
}
#[test]
fn independent_authenticated_catalog_negatives_refuse_without_weakening_old_codecs() {
    let mut count = 0;
    for row in rows()
        .iter()
        .filter(|r| r.get("accepted_planned").unwrap().as_bool() == Some(false))
    {
        count += 1;
        let name = row.get("name").unwrap().as_str().unwrap();
        let bytes = fixture(row);
        let budget = SharedBudget::new(4 << 20);
        let error = open(&bytes, &budget, &|| Ok(())).unwrap_err();
        assert_eq!(
            error.code.as_str(),
            row.get("code_planned").unwrap().as_str().unwrap(),
            "{name}: {error}"
        );
        if name.starts_with("unknown-") && !name.contains("mask") {
            assert_eq!(
                budget.usage().reserved,
                0,
                "unknown catalog retained payload: {name}"
            );
            assert_eq!(
                budget.usage().peak,
                0,
                "unknown catalog allocated payload: {name}"
            );
        }
    }
    assert_eq!(count, 37);
}
#[test]
fn catalog_assessment_reports_unknown_requirements_but_reader_never_adopts_them() {
    for row in rows().iter().filter(|r| {
        [
            "unknown-profile",
            "unknown-codec",
            "unknown-disabled-profile",
            "unknown-disabled-codec",
        ]
        .contains(&r.get("name").unwrap().as_str().unwrap())
    }) {
        let bytes = fixture(row);
        assert!(!pcnd::catalog_manifest(&bytes).unwrap().is_empty());
        let budget = SharedBudget::new(0);
        assert_eq!(
            open(&bytes, &budget, &|| Ok(())).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn every_catalog_read_and_assessment_checkpoint_can_cancel_without_retained_charge() {
    let rows = rows();
    let row = rows
        .iter()
        .find(|r| {
            r.get("source_attachment")
                .is_some_and(|v| !matches!(v, Value::Null))
                && r.get("accepted_planned").unwrap().as_bool() == Some(true)
        })
        .unwrap();
    let bytes = fixture(row);
    for assessment in [false, true] {
        let checks = Cell::new(0);
        let budget = SharedBudget::new(4 << 20);
        let check = || {
            checks.set(checks.get() + 1);
            Ok(())
        };
        if assessment {
            pcnd::catalog_manifest_checked(&bytes, &check).unwrap();
        } else {
            drop(open(&bytes, &budget, &check).unwrap());
        }
        assert_eq!(budget.usage().reserved, 0);
        for at in 1..=checks.get() {
            let visits = Cell::new(0);
            let check = || {
                visits.set(visits.get() + 1);
                if visits.get() == at {
                    Err(paintcraft_core::types::Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel catalog",
                    ))
                } else {
                    Ok(())
                }
            };
            let error = if assessment {
                pcnd::catalog_manifest_checked(&bytes, &check).unwrap_err()
            } else {
                open(&bytes, &budget, &check).unwrap_err()
            };
            assert_eq!(error.code, ErrorCode::OperationCancelled);
            assert_eq!(budget.usage().reserved, 0, "checkpoint{at}");
        }
    }
}

#[test]
fn catalog_source_and_coverage_quotas_fail_without_leaking_retained_payloads() {
    let rows = rows();
    let row = rows
        .iter()
        .find(|r| r.get("name").unwrap().as_str() == Some("box-levels-mask-source"))
        .unwrap();
    let bytes = fixture(row);
    let empty = SharedBudget::new(0);
    assert_eq!(
        open(&bytes, &empty, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(empty.usage().peak, 0);
    let source = SharedBudget::new(2 << 20);
    let error = pcnd::read_with_admission_checked(
        &bytes,
        2,
        65536,
        1,
        pcnd::ReadAdmission {
            metadata: pcnd::MetadataAdmission {
                bytes: 1 << 20,
                budget: &source,
            },
            coverage: pcnd::CoverageAdmission {
                bytes: 1 << 20,
                budget: &empty,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(source.usage().peak > 0);
    assert_eq!(source.usage().reserved, 0);
    assert_eq!(empty.usage().reserved, 0);
}
#[test]
fn removing_last_catalog_intention_restores_minimal_old_encoding_without_source_changes() {
    for row in rows()
        .iter()
        .filter(|r| r.get("accepted_planned").unwrap().as_bool() == Some(true))
    {
        let budget = SharedBudget::new(4 << 20);
        let mut doc = open(&fixture(row), &budget, &|| Ok(())).unwrap();
        let source = doc.source_metadata.clone();
        let raw: Vec<_> = (0..2)
            .map(|x| doc.layers[&doc.roots[0]].raster.get(x, 0))
            .collect();
        for layer in doc.layers.values_mut() {
            layer.filters = paintcraft_document::FilterStack::new(
                layer
                    .filters
                    .iter()
                    .filter(|f| !f.descriptor().is_catalog_only())
                    .cloned()
                    .collect(),
            )
            .unwrap();
        }
        let stored: Vec<_> = doc.layers.values().flat_map(|l| l.filters.iter()).collect();
        let version = if stored.iter().any(|f| f.descriptor().is_spatial()) {
            5
        } else if stored.iter().any(|f| f.mask().is_some()) {
            4
        } else if !stored.is_empty() {
            3
        } else if source.is_some() {
            2
        } else {
            1
        };
        let encoded = pcnd::write(&doc).unwrap();
        assert_eq!(encoded[4], version);
        assert!(pcnd::catalog_manifest(&encoded).unwrap().is_empty());
        let restored = open(&encoded, &budget, &|| Ok(())).unwrap();
        assert!(doc.same_semantics(&restored));
        assert_eq!(restored.source_metadata, source);
        assert_eq!(
            (0..2)
                .map(|x| restored.layers[&restored.roots[0]].raster.get(x, 0))
                .collect::<Vec<_>>(),
            raw
        );
    }
}
#[test]
fn disabled_repeated_levels_intentions_use_one_required_profile_without_becoming_legacy() {
    let rows = rows();
    let row = rows
        .iter()
        .find(|r| r.get("name").unwrap().as_str() == Some("levels-default"))
        .unwrap();
    let budget = SharedBudget::new(1 << 20);
    let mut doc = open(&fixture(row), &budget, &|| Ok(())).unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    let first = layer.filters.as_slice()[0].with_enabled(false);
    let second = first.with_id(uuid::Uuid::from_u128(999));
    layer.filters = paintcraft_document::FilterStack::new(vec![first, second]).unwrap();
    let encoded = pcnd::write(&doc).unwrap();
    assert_eq!(encoded[4], 6);
    let manifest = pcnd::catalog_manifest(&encoded).unwrap();
    assert_eq!(manifest.len(), 1);
    assert_eq!(manifest[0].profile_id, "levels-rgb-v1");
    let restored = open(&encoded, &budget, &|| Ok(())).unwrap();
    assert!(doc.same_semantics(&restored));
    assert!(
        restored.layers[&restored.roots[0]]
            .filters
            .iter()
            .all(|f| !f.enabled())
    );
}

#[test]
fn independent_catalog_overhead_obeys_exact_raw_and_compact_8mib_metadata_boundary() {
    const MAX: u64 = 8_388_608;
    let rows = rows();
    let row = rows
        .iter()
        .find(|r| r.get("name").unwrap().as_str() == Some("box-levels-mask-source"))
        .unwrap();
    let independent = fixture(row);
    let budget = SharedBudget::new(32 << 20);
    let mut doc = open(&independent, &budget, &|| Ok(())).unwrap();
    let original = doc.source_metadata.take().unwrap();
    let descriptor = *original.descriptor();
    // Independent specimen has exactly eight raw RGBA payload bytes. Every
    // other body byte, including catalog/mask framing, is metadata. Footer SHA
    // is not metadata. No product preflight/writer supplies this overhead.
    let overhead = independent.len() - 32 - 8 - original.profile().unwrap().len();
    let length = MAX as usize - overhead;
    let mut profile = original.profile().unwrap().to_vec();
    profile.resize(length, 0);
    profile[..4].copy_from_slice(&(length as u32).to_be_bytes());
    doc.source_metadata = Some(
        paintcraft_document::SourceMetadata::capture(
            descriptor,
            Some(&profile),
            MAX,
            &budget,
            &|| Ok(()),
        )
        .unwrap(),
    );
    pcnd::check_metadata_admission(&doc, &|| Ok(())).unwrap();
    pcnd::check_compact_metadata_admission(&doc, &|| Ok(())).unwrap();
    let raw = pcnd::write(&doc).unwrap();
    let compact = pcnd::write_compact(&doc).unwrap();
    assert_eq!(raw.len(), MAX as usize + 8 + 32);
    assert_eq!(compact.len(), raw.len());
    assert_eq!(raw[4], 6);
    for bytes in [&raw, &compact] {
        let reopened = pcnd::read_with_admission_checked(
            bytes,
            2,
            65536,
            1,
            pcnd::ReadAdmission {
                metadata: pcnd::MetadataAdmission {
                    bytes: MAX,
                    budget: &budget,
                },
                coverage: pcnd::CoverageAdmission {
                    bytes: 1 << 20,
                    budget: &budget,
                    limits: Default::default(),
                },
            },
            &|| Ok(()),
        )
        .unwrap();
        assert!(doc.same_semantics(&reopened));
    }
    // Adding one ASCII byte leaves the name's one-byte length framing unchanged.
    // Both conservative acceptance bounds and authoritative exact counters must
    // deny the resulting metadata overflow, including selected-compact output.
    doc.layers.get_mut(&doc.roots[0]).unwrap().name.push('x');
    assert_eq!(
        pcnd::check_metadata_admission(&doc, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        pcnd::check_compact_metadata_admission(&doc, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        pcnd::write(&doc).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        pcnd::write_compact(&doc).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}
