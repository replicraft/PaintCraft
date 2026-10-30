//! Independent PCND3 wire specimens and preservation/admission controls.
use super::*;
use paintcraft_core::types::filters::Descriptor;
use paintcraft_document::{FilterInstance, FilterStack};
use std::{cell::Cell, path::Path};
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/pcnd-filter-fixtures")
            .join(name),
    )
    .unwrap()
}
fn cancelled() -> Error {
    Error::new(ErrorCode::OperationCancelled, "PCND3 cancellation")
}

#[test]
fn independent_raw_v1_v2_and_v3_roundtrip_exact_bytes_and_hidden_rgb() {
    let oracle = paintcraft_core::types::json::parse(&fixture("oracle.json")).unwrap();
    for row in oracle.get("fixtures").unwrap().as_array().unwrap() {
        let name = row.get("file").unwrap().as_str().unwrap();
        let bytes = fixture(name);
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&bytes)),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        if !row.get("accepted").unwrap().as_bool().unwrap() {
            continue;
        }
        let document = read(&bytes).unwrap();
        let layer = &document.layers[&document.roots[0]];
        assert_eq!(document.canvas, (2, 1));
        assert_eq!(layer.raster.get(0, 0), Rgba([17, 32, 255, 0]));
        assert_eq!(layer.raster.get(1, 0), Rgba([0, 127, 200, 255]));
        assert_eq!(write(&document).unwrap(), bytes, "independent raw {name}");
        assert_eq!(
            write_compact(&document).unwrap(),
            bytes,
            "selected compact {name}"
        );
        check_metadata_admission(&document, &|| Ok(())).unwrap();
        check_compact_metadata_admission(&document, &|| Ok(())).unwrap();
        let header = probe(&bytes).unwrap();
        header.require_supported().unwrap();
        assert_eq!(
            header
                .required_features
                .contains(&LAYER_FILTERS_FEATURE.into()),
            bytes[4] == 3
        );
        assert_eq!(
            header
                .required_features
                .contains(&SOURCE_METADATA_FEATURE.into()),
            document.source_metadata.is_some()
        );
        if name == "stack.pcnd" || name == "source-stack.pcnd" {
            assert_eq!(layer.filters.len(), 3);
            let first = &layer.filters.as_slice()[0];
            assert_eq!(first.id(), Uuid::nil());
            assert!(!first.enabled());
            let Descriptor::BrightnessContrastV1(parameters) = first.descriptor() else {
                panic!("expected BC")
            };
            assert_eq!((parameters.brightness(), parameters.contrast()), (-100, 99));
            assert_eq!(
                layer.filters.as_slice()[1].descriptor(),
                Descriptor::InvertRgbV1
            );
        } else if name == "disabled.pcnd" {
            assert_eq!(layer.filters.len(), 1);
            assert!(!layer.filters.as_slice()[0].enabled());
            assert_eq!(bytes[4], 3, "disabled settings must persist");
        } else {
            assert!(layer.filters.is_empty());
        }
        if let Some(metadata) = &document.source_metadata {
            let parent = read(include_bytes!(
                "../../tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
            ))
            .unwrap();
            assert_eq!(Some(metadata), parent.source_metadata.as_ref());
        }
    }
}

#[test]
fn unknown_disabled_algorithms_malformed_counts_flags_params_and_versions_refuse() {
    for (name, code) in [
        ("unknown-disabled.pcnd", ErrorCode::CapabilityUnsupported),
        ("unknown-enabled.pcnd", ErrorCode::CapabilityUnsupported),
        ("future-v5.pcnd", ErrorCode::CapabilityUnsupported),
        ("count257.pcnd", ErrorCode::ResourceLimit),
        ("empty-v3.pcnd", ErrorCode::CodecFailed),
        ("duplicate.pcnd", ErrorCode::CodecFailed),
        ("bad-enabled.pcnd", ErrorCode::CodecFailed),
        ("bad-presence.pcnd", ErrorCode::CodecFailed),
        ("bad-brightness.pcnd", ErrorCode::CodecFailed),
        ("bad-contrast.pcnd", ErrorCode::CodecFailed),
        ("noncanonical-count.pcnd", ErrorCode::CodecFailed),
    ] {
        assert_eq!(read(&fixture(name)).unwrap_err().code, code, "{name}");
    }
    let original = fixture("stack.pcnd");
    for end in [44, 70, original.len() - 33] {
        let body = &original[..end];
        let mut sealed = body.to_vec();
        sealed.extend_from_slice(&Sha256::digest(body));
        assert_eq!(read(&sealed).unwrap_err().code, ErrorCode::CodecFailed);
    }
    let mut header = probe(&original).unwrap();
    header
        .required_features
        .push("unknown-filter-policy.v2".into());
    assert_eq!(
        header.require_supported().unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}

#[test]
fn count_cap_and_metadata_admission_precede_decoded_tile_construction() {
    let mut document = read(&fixture("stack.pcnd")).unwrap();
    let instances = (0..256)
        .map(|n| FilterInstance::new(Uuid::from_u128(n), n % 2 == 0, Descriptor::InvertRgbV1))
        .collect();
    document.layers.values_mut().next().unwrap().filters = FilterStack::new(instances).unwrap();
    let encoded = write(&document).unwrap();
    assert_eq!(
        read(&encoded)
            .unwrap()
            .layers
            .values()
            .next()
            .unwrap()
            .filters
            .len(),
        256
    );
    let error = read_with_limits(&fixture("count257.pcnd"), 2, 0).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(error.message.contains("filter count"));
    let empty = write(&read(&fixture("legacy-v1.pcnd")).unwrap()).unwrap();
    assert_eq!(empty[4], 1);
}

#[test]
fn every_read_write_and_preflight_checkpoint_preserves_one_shot_cancellation() {
    let bytes = fixture("source-stack.pcnd");
    let document = read(&bytes).unwrap();
    let realm = SharedBudget::new(1 << 20);
    let read_checked = |check: &dyn Fn() -> Result<()>| {
        read_with_metadata_admission_checked(
            &bytes,
            2,
            65536,
            1,
            MetadataAdmission {
                bytes: 1 << 20,
                budget: &realm,
            },
            check,
        )
        .map(drop)
    };
    let raw = |check: &dyn Fn() -> Result<()>| binary::write_raw(&document, std::io::sink(), check);
    let compact = |check: &dyn Fn() -> Result<()>| {
        write_compact_to_checked(&document, std::io::sink(), check)
    };
    let preflight =
        |check: &dyn Fn() -> Result<()>| check_compact_metadata_admission(&document, check);
    for operation in [
        &read_checked as &dyn Fn(&dyn Fn() -> Result<()>) -> Result<()>,
        &raw,
        &compact,
        &preflight,
    ] {
        let visits = Cell::new(0);
        operation(&|| {
            visits.set(visits.get() + 1);
            Ok(())
        })
        .unwrap();
        for stop in 1..=visits.get() {
            let count = Cell::new(0);
            let error = operation(&|| {
                count.set(count.get() + 1);
                if count.get() == stop {
                    Err(cancelled())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
            assert_eq!(realm.usage().reserved, 0);
        }
    }
}

#[test]
fn v3_filter_metadata_is_counted_at_the_exact_shared_limit() {
    use paintcraft_document::source_metadata::{BYTES_MAX, SourceMetadata};
    let document = read(&fixture("source-stack.pcnd")).unwrap();
    let original = document.source_metadata.as_ref().unwrap();
    let raw = write(&document).unwrap();
    let metadata_without_profile = raw.len() - 32 - 8 - original.profile().unwrap().len();
    let profile_bytes = DOCUMENT_MAX as usize - metadata_without_profile;
    let mut profile = original.profile().unwrap().to_vec();
    profile.resize(profile_bytes, 0);
    profile[..4].copy_from_slice(&(profile_bytes as u32).to_be_bytes());
    let budget = SharedBudget::new(BYTES_MAX + 4096);
    let metadata = SourceMetadata::capture(
        *original.descriptor(),
        Some(&profile),
        BYTES_MAX,
        &budget,
        &|| Ok(()),
    )
    .unwrap();
    let mut candidate = document
        .with_source_metadata_checked(Some(metadata), &|| Ok(()))
        .unwrap();
    check_metadata_admission(&candidate, &|| Ok(())).unwrap();
    check_compact_metadata_admission(&candidate, &|| Ok(())).unwrap();
    assert_eq!(
        write(&candidate).unwrap().len(),
        DOCUMENT_MAX as usize + 8 + 32
    );
    assert_eq!(
        write_compact(&candidate).unwrap().len(),
        DOCUMENT_MAX as usize + 8 + 32
    );
    candidate.layers.values_mut().next().unwrap().name.push('x');
    assert_eq!(
        check_metadata_admission(&candidate, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        check_compact_metadata_admission(&candidate, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        write(&candidate).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    drop(candidate);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn mixed_empty_stacks_and_filter_removal_select_the_correct_container_version() {
    let mut document = read(&fixture("stack.pcnd")).unwrap();
    let mut layer = document.layers[&document.roots[0]].clone();
    layer.id = Uuid::from_u128(19);
    layer.filters = FilterStack::default();
    document.roots.push(layer.id);
    document.layers.insert(layer.id, layer);
    let bytes = write(&document).unwrap();
    assert_eq!(bytes[4], 3);
    assert!(read(&bytes).unwrap().same_semantics(&document));
    for layer in document.layers.values_mut() {
        layer.filters = FilterStack::default();
    }
    assert_eq!(write(&document).unwrap()[4], 1);
    let mut attached = read(&fixture("source-stack.pcnd")).unwrap();
    attached.layers.values_mut().next().unwrap().filters = FilterStack::default();
    assert_eq!(write(&attached).unwrap(), fixture("legacy-v2.pcnd"));
}
