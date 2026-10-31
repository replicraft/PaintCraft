//! Independent PCND5 wire/source preservation; rendering is a separate gate.
use super::*;
use paintcraft_core::types::{
    filters::Descriptor,
    json::{self, Value},
    policy::hex,
};
use paintcraft_document::FilterStack;
use paintcraft_raster::coverage_mask::Limits;
use std::{cell::Cell, path::Path};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/pcnd-spatial-fixtures")
            .join(name),
    )
    .unwrap()
}
fn manifest() -> Value {
    json::parse(&fixture("oracle.json")).unwrap()
}
fn get<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.get(key).unwrap()
}
fn array(value: &Value) -> &[Value] {
    value.as_array().unwrap()
}
fn number(value: &Value) -> i64 {
    value.as_i64().unwrap()
}
fn bytes(value: &Value) -> Vec<u8> {
    array(value).iter().map(|v| number(v) as u8).collect()
}
fn open(
    data: &[u8],
    profile: &SharedBudget,
    masks: &SharedBudget,
    mask_bytes: u64,
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Document> {
    read_with_admission_checked(
        data,
        2,
        65536,
        1,
        ReadAdmission {
            metadata: MetadataAdmission {
                bytes: DOCUMENT_MAX,
                budget: profile,
            },
            coverage: CoverageAdmission {
                bytes: mask_bytes,
                budget: masks,
                limits,
            },
        },
        check,
    )
}
fn load(name: &str) -> Document {
    open(
        &fixture(name),
        &SharedBudget::new(2 << 20),
        &SharedBudget::new(2 << 20),
        2 << 20,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn independent_pcnd5_descriptors_sources_masks_and_raw_bytes_roundtrip_exactly() {
    let oracle = manifest();
    let mut accepted = 0;
    for row in array(get(&oracle, "fixtures"))
        .iter()
        .filter(|row| get(row, "accepted").as_bool() == Some(true))
    {
        accepted += 1;
        let name = get(row, "file").as_str().unwrap();
        let original = fixture(name);
        assert_eq!(
            hex(&Sha256::digest(&original)),
            get(row, "sha256").as_str().unwrap(),
            "{name}"
        );
        let profile = SharedBudget::new(2 << 20);
        let masks = SharedBudget::new(2 << 20);
        let doc = open(
            &original,
            &profile,
            &masks,
            2 << 20,
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(
            doc.document_id.to_string(),
            get(&oracle, "document").as_str().unwrap()
        );
        assert_eq!(
            doc.state_id.to_string(),
            get(&oracle, "state").as_str().unwrap()
        );
        let layer = &doc.layers[&doc.roots[0]];
        assert_eq!(
            layer.id.to_string(),
            get(&oracle, "layer").as_str().unwrap()
        );
        assert_eq!(
            layer.origin,
            (
                number(&array(get(row, "origin"))[0]),
                number(&array(get(row, "origin"))[1])
            )
        );
        assert_eq!(
            (0..2)
                .flat_map(|x| layer.raster.get(x, 0).0)
                .collect::<Vec<_>>(),
            bytes(get(row, "raw_rgba"))
        );
        assert_eq!(
            hex(&layer.filters.digest()),
            get(row, "stack_sha256").as_str().unwrap()
        );
        assert_eq!(
            doc.source_metadata.is_some(),
            get(row, "attachment").as_bool().unwrap()
        );
        if let Some(source) = &doc.source_metadata {
            assert_eq!(
                hex(&source.digest()),
                get(&oracle, "attachment_digest").as_str().unwrap()
            );
            assert_eq!(
                hex(&Sha256::digest(source.profile().unwrap())),
                get(&oracle, "profile_sha256").as_str().unwrap()
            );
        }
        let expected = array(get(row, "filters"));
        assert_eq!(layer.filters.len(), expected.len());
        for (actual, expected) in layer.filters.iter().zip(expected) {
            assert_eq!(
                actual.id().to_string(),
                get(expected, "id").as_str().unwrap()
            );
            assert_eq!(
                actual.enabled(),
                get(expected, "enabled").as_bool().unwrap()
            );
            assert_eq!(actual.descriptor().to_value(), *get(expected, "descriptor"));
            match get(expected, "mask") {
                Value::Null => assert!(actual.mask().is_none()),
                value => {
                    let mask = actual.mask().unwrap();
                    assert_eq!(mask.enabled(), get(value, "enabled").as_bool().unwrap());
                    assert_eq!(
                        hex(&mask.coverage().digest()),
                        get(expected, "mask_sha256").as_str().unwrap()
                    );
                    let domain = array(get(value, "domain"));
                    let rect = mask.coverage().domain();
                    assert_eq!(
                        [rect.x0, rect.y0, rect.x1, rect.y1],
                        [
                            number(&domain[0]),
                            number(&domain[1]),
                            number(&domain[2]),
                            number(&domain[3])
                        ]
                    );
                    assert_eq!(
                        mask.coverage().stats().bands,
                        array(get(value, "bands")).len()
                    );
                    let left = if array(get(value, "bands")).is_empty() {
                        0
                    } else {
                        128
                    };
                    assert_eq!(
                        [
                            mask.coverage().coverage(0, 0),
                            mask.coverage().coverage(1, 0)
                        ],
                        [left, 0]
                    );
                }
            }
        }
        assert_eq!(write(&doc).unwrap(), original, "RAW {name}");
        assert_eq!(write_compact(&doc).unwrap(), original, "compact {name}");
        check_metadata_admission(&doc, &|| Ok(())).unwrap();
        check_compact_metadata_admission(&doc, &|| Ok(())).unwrap();
        let header = probe(&original).unwrap();
        header.require_supported().unwrap();
        assert_eq!(header.container_version, 5);
        assert_eq!(header.document_schema, "paintcraft.document/v5");
        let mut features = SUPPORTED_FEATURES
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        features.extend(
            [
                LAYER_FILTERS_FEATURE,
                LAYER_MASKS_FEATURE,
                LAYER_SPATIAL_FILTERS_FEATURE,
            ]
            .map(str::to_string),
        );
        if doc.source_metadata.is_some() {
            features.push(SOURCE_METADATA_FEATURE.into());
        }
        assert_eq!(header.required_features, features);
        drop(doc);
        assert_eq!(profile.usage().reserved, 0);
        assert_eq!(masks.usage().reserved, 0);
    }
    assert_eq!(accepted, 16);
}
#[test]
fn old_tag7_registries_unknown8_disabled_bad_ranges_and_noncanonical5_refuse() {
    let oracle = manifest();
    let mut refused = 0;
    for row in array(get(&oracle, "fixtures"))
        .iter()
        .filter(|row| get(row, "accepted").as_bool() == Some(false))
    {
        refused += 1;
        let name = get(row, "file").as_str().unwrap();
        let data = fixture(name);
        assert_eq!(
            hex(&Sha256::digest(&data)),
            get(row, "sha256").as_str().unwrap()
        );
        let profile = SharedBudget::new(2 << 20);
        let masks = SharedBudget::new(2 << 20);
        let error = open(&data, &profile, &masks, 2 << 20, Limits::default(), &|| {
            Ok(())
        })
        .unwrap_err();
        // Frozen historical version6 bytes have no catalog prelude. Now that
        // version6 has defined framing, this specimen is malformed rather than
        // an unknown version. Its bytes/provenance remain unchanged.
        let expected = if name == "future-v6.pcnd" {
            assert_eq!(data[4], 6);
            "codec.failed"
        } else {
            get(row, "code").as_str().unwrap()
        };
        assert_eq!(error.code.as_str(), expected, "{name}");
        assert_eq!(profile.usage().reserved, 0);
        assert_eq!(masks.usage().reserved, 0);
    }
    assert_eq!(refused, 18);
}
#[test]
fn stored_disabled_and_zero_box_select5_and_removal_restores_legacy_grammar() {
    for (name, version) in [
        ("box-r0.pcnd", 1),
        ("box-disabled-r0.pcnd", 1),
        ("box-disabled-r2.pcnd", 1),
        ("source-box.pcnd", 2),
        ("point-box-point.pcnd", 3),
        ("point-mask-disabled-box.pcnd", 4),
    ] {
        let mut doc = load(name);
        assert_eq!(write(&doc).unwrap()[4], 5);
        for layer in doc.layers.values_mut() {
            layer.filters = FilterStack::new(
                layer
                    .filters
                    .iter()
                    .filter(|f| !matches!(f.descriptor(), Descriptor::BoxBlurV1(_)))
                    .cloned()
                    .collect(),
            )
            .unwrap();
        }
        assert_eq!(write(&doc).unwrap()[4], version, "{name}");
    }
    let mut header = probe(&fixture("box-r0.pcnd")).unwrap();
    for required in [
        LAYER_FILTERS_FEATURE,
        LAYER_MASKS_FEATURE,
        LAYER_SPATIAL_FILTERS_FEATURE,
    ] {
        let original = header.required_features.clone();
        header.required_features.retain(|f| f != required);
        assert_eq!(
            header.require_supported().unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        header.required_features = original;
    }
}
#[test]
fn v5_mask_grammar_requires_explicit_shared_admission_and_aggregate_counts() {
    assert_eq!(
        read(&fixture("box-r0.pcnd")).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let zero = SharedBudget::new(0);
    let unmasked = open(
        &fixture("box-r0.pcnd"),
        &zero,
        &zero,
        0,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    drop(unmasked);
    assert_eq!(zero.usage().reserved, 0);
    for (name, mask_bytes, limits) in [
        ("box-half-mask.pcnd", 1, Limits::default()),
        (
            "two-box-shared-wire-masks.pcnd",
            2 << 20,
            Limits {
                bands: 1,
                runs: 1,
                ..Limits::default()
            },
        ),
    ] {
        let profile = SharedBudget::new(2 << 20);
        let masks = SharedBudget::new(2 << 20);
        assert_eq!(
            open(
                &fixture(name),
                &profile,
                &masks,
                mask_bytes,
                limits,
                &|| Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(masks.usage().reserved, 0);
    }
}
#[test]
fn pcnd5_cancelled_reads_release_profile_and_coverage_without_partial_document() {
    let data = fixture("source-box-mask.pcnd");
    let profile = SharedBudget::new(2 << 20);
    let masks = SharedBudget::new(2 << 20);
    let calls = Cell::new(0);
    let doc = open(&data, &profile, &masks, 2 << 20, Limits::default(), &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    drop(doc);
    let total = calls.get();
    assert!(total > 0);
    for stop in 0..total {
        calls.set(0);
        let check = || {
            let n = calls.get();
            calls.set(n + 1);
            if n >= stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            open(&data, &profile, &masks, 2 << 20, Limits::default(), &check)
                .unwrap_err()
                .code,
            ErrorCode::OperationCancelled,
            "stop{stop}"
        );
        assert_eq!(profile.usage().reserved, 0);
        assert_eq!(masks.usage().reserved, 0);
    }
}
#[test]
fn pcnd5_unmasked_presence_bytes_obey_the_same_exact_8mib_metadata_cap() {
    let mut doc = load("source-box.pcnd");
    let source = doc.source_metadata.take().unwrap();
    let descriptor = *source.descriptor();
    let metadata = 295usize; // independent wire:44 prelude+192attachment+1count+53layer/filter+4block+1end
    let profile_len = DOCUMENT_MAX as usize - metadata;
    let mut profile = source.profile().unwrap().to_vec();
    profile.resize(profile_len, 0);
    profile[..4].copy_from_slice(&(profile_len as u32).to_be_bytes());
    let budget = SharedBudget::new(DOCUMENT_MAX + 4096);
    doc.source_metadata = Some(
        paintcraft_document::source_metadata::SourceMetadata::capture(
            descriptor,
            Some(&profile),
            DOCUMENT_MAX,
            &budget,
            &|| Ok(()),
        )
        .unwrap(),
    );
    check_metadata_admission(&doc, &|| Ok(())).unwrap();
    check_compact_metadata_admission(&doc, &|| Ok(())).unwrap();
    let encoded = write(&doc).unwrap();
    assert_eq!(encoded.len(), DOCUMENT_MAX as usize + 8 + 32);
    assert_eq!(encoded[4], 5);
    doc.layers.values_mut().next().unwrap().name.push('x');
    assert_eq!(
        check_metadata_admission(&doc, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        check_compact_metadata_admission(&doc, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(write(&doc).unwrap_err().code, ErrorCode::ResourceLimit);
}
