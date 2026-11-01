//! Finite independently authored PCND7 wire and shared admission controls.
use super::*;
use paintcraft_core::types::resources::AllocationClass;
use paintcraft_raster::coverage_mask::Limits;
use std::cell::Cell;

fn oracle() -> paintcraft_core::types::json::Value {
    paintcraft_core::types::json::parse(include_bytes!(
        "../../tests/pcnd-layer-mask-fixtures/oracle.json"
    ))
    .unwrap()
}
fn decode_hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let digit = |b: u8| match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                _ => panic!("hex"),
            };
            (digit(pair[0]) << 4) | digit(pair[1])
        })
        .collect()
}
fn specimen(name: &str) -> Vec<u8> {
    let oracle = oracle();
    let row = oracle
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row.get("name").unwrap().as_str() == Some(name))
        .unwrap();
    decode_hex(row.get("hex").unwrap().as_str().unwrap())
}
fn open(
    bytes: &[u8],
    budget: &SharedBudget,
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Document> {
    read_with_admission_checked(
        bytes,
        2,
        131_072,
        2,
        ReadAdmission {
            metadata: MetadataAdmission {
                bytes: DOCUMENT_MAX,
                budget,
            },
            coverage: CoverageAdmission {
                bytes: 1 << 20,
                budget,
                limits,
            },
        },
        check,
    )
}
#[test]
fn independent_v7_masks_roundtrip_features_and_persistence_without_execution_grant() {
    let data = oracle();
    for row in data.get("fixtures").unwrap().as_array().unwrap() {
        if !row.get("accepted").unwrap().as_bool().unwrap() {
            continue;
        }
        let name = row.get("name").unwrap().as_str().unwrap();
        let bytes = specimen(name);
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&bytes)),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        let budget = SharedBudget::new(1 << 20);
        let doc = open(&bytes, &budget, Limits::default(), &|| Ok(())).unwrap();
        let layer = &doc.layers[&doc.roots[0]];
        let mask = layer.mask.as_ref().unwrap();
        assert_eq!(mask.enabled(), name != "disabled");
        assert_eq!(
            mask.coverage().domain(),
            paintcraft_core::types::Rect::new(-2, -1, 6, 3).unwrap()
        );
        assert_eq!(
            mask.coverage().coverage(-2, -1),
            if name == "zero" { 0 } else { 128 }
        );
        assert_eq!(layer.raster.get(0, 0), Rgba([17, 32, 255, 0]));
        assert_eq!(write(&doc).unwrap(), bytes, "{name}");
        assert_eq!(write_compact(&doc).unwrap(), bytes, "{name}");
        check_metadata_admission(&doc, &|| Ok(())).unwrap();
        check_compact_metadata_admission(&doc, &|| Ok(())).unwrap();
        let witness = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
        let header = witness.header().unwrap();
        header.require_supported().unwrap();
        assert_eq!(header.document_schema, "paintcraft.document/v7");
        assert!(
            header
                .required_features
                .iter()
                .any(|f| f == LAYER_RASTER_MASKS_FEATURE)
        );
        assert_eq!(
            witness.snapshot_tile_records_checked(&|| Ok(())).unwrap(),
            if name == "two-layers" { 2 } else { 1 }
        );
        assert_eq!(
            catalog_manifest(&bytes).unwrap().len(),
            usize::from(name == "mixed")
        );
        let scope = budget
            .reserve_capacity(8192)
            .unwrap()
            .scope(AllocationClass::CoverageMasks);
        let callbacks = Cell::new(0);
        assert_eq!(
            witness
                .resolve_requirements_checked(
                    RequirementsLimits {
                        layers: 1,
                        tiles: 1,
                        metadata_bytes: DOCUMENT_MAX as usize,
                        coverage: Limits::default(),
                        work: 1_000_000
                    },
                    &scope,
                    &|| {
                        callbacks.set(callbacks.get() + 1);
                        Ok(())
                    },
                    |_, _| panic!("v7 must not reach old execution registry")
                )
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(callbacks.get(), 0);
        drop(scope);
        drop(doc);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn independent_malformed_and_missing_shared_admission_refuse_and_release() {
    let data = oracle();
    for row in data.get("fixtures").unwrap().as_array().unwrap() {
        if row.get("accepted").unwrap().as_bool().unwrap() {
            continue;
        }
        let name = row.get("name").unwrap().as_str().unwrap();
        let budget = SharedBudget::new(1 << 20);
        let err = open(&specimen(name), &budget, Limits::default(), &|| Ok(())).unwrap_err();
        assert_eq!(
            err.code,
            if name == "unknown-disabled-codec" {
                ErrorCode::CapabilityUnsupported
            } else {
                ErrorCode::CodecFailed
            },
            "{name}"
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    assert_eq!(
        read(&specimen("bands")).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn both_mask_families_share_counts_work_bytes_cancellation_and_release() {
    let bytes = specimen("mixed");
    let budget = SharedBudget::new(1 << 20);
    for limits in [
        Limits {
            bands: 3,
            ..Limits::default()
        },
        Limits {
            runs: 5,
            ..Limits::default()
        },
        Limits {
            work: 1,
            ..Limits::default()
        },
    ] {
        assert_eq!(
            open(&bytes, &budget, limits, &|| Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    let calls = Cell::new(0);
    let doc = open(&bytes, &budget, Limits::default(), &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    let facts = coverage_facts(&doc, &|| Ok(())).unwrap();
    assert_eq!((facts.masks, facts.bands, facts.runs), (2, 4, 6));
    let retained = budget.usage().reserved;
    assert!(retained > 0);
    let filler = budget.reserve((1 << 20) - retained).unwrap();
    assert_eq!(
        open(&bytes, &budget, Limits::default(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    drop(filler);
    drop(doc);
    assert_eq!(budget.usage().reserved, 0);
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        assert_eq!(
            open(&bytes, &budget, Limits::default(), &|| {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "layer-mask cancel",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled,
            "{stop}"
        );
        assert_eq!(budget.usage().reserved, 0, "{stop}");
    }
}
#[test]
fn removing_last_layer_mask_restores_exact_old_minimum_and_none_callback_path() {
    let budget = SharedBudget::new(1 << 20);
    let legacy = include_bytes!("../../tests/pcnd-filter-fixtures/legacy-v1.pcnd");
    let original = read(legacy).unwrap();
    let a = Cell::new(0);
    let mut before = Vec::new();
    write_compact_to_checked(&original, &mut before, &|| {
        a.set(a.get() + 1);
        Ok(())
    })
    .unwrap();
    let mut changed = open(&specimen("bands"), &budget, Limits::default(), &|| Ok(())).unwrap();
    changed.layers.get_mut(&changed.roots[0]).unwrap().mask = None;
    let b = Cell::new(0);
    let mut after = Vec::new();
    write_compact_to_checked(&changed, &mut after, &|| {
        b.set(b.get() + 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(after, before);
    assert_eq!(write(&changed).unwrap(), legacy);
    assert_eq!(a.get(), b.get());
}
#[test]
fn old_filter_only_bake_authority_and_layered_writers_refuse_all_some_intent() {
    use crate::export::{
        filter_bake::{Intent, Policy},
        metadata_policy::Target,
    };
    use crate::registry::{Family, Operation};
    let budget = SharedBudget::new(1 << 20);
    for name in ["bands", "disabled", "zero"] {
        let mut doc = open(&specimen(name), &budget, Limits::default(), &|| Ok(())).unwrap();
        let mask = doc
            .layers
            .get_mut(&doc.roots[0])
            .unwrap()
            .mask
            .take()
            .unwrap();
        let target = Target::new(Family::Png, Operation::ExportFlat);
        let old = Policy::capture(&doc, target, Intent::Bake, &|| Ok(())).unwrap();
        doc.layers.get_mut(&doc.roots[0]).unwrap().mask = Some(mask);
        assert_eq!(
            old.validate(&doc, target, &|| Ok(())).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            Policy::capture(&doc, target, Intent::Bake, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            crate::filter_policy::require_unfiltered_layers(&doc)
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
}

#[test]
fn seven_requires_distinct_own_mask_feature_and_not_filter_mask_alias() {
    let mut header = probe(&specimen("zero")).unwrap();
    assert_ne!(LAYER_RASTER_MASKS_FEATURE, LAYER_MASKS_FEATURE);
    header
        .required_features
        .retain(|f| f != LAYER_RASTER_MASKS_FEATURE);
    assert_eq!(
        header.require_supported().unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    header
        .required_features
        .push(LAYER_RASTER_MASKS_FEATURE.into());
    header.require_supported().unwrap();
    header
        .required_features
        .retain(|f| f != LAYER_CATALOG_FEATURE);
    assert_eq!(
        header.require_supported().unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}

#[test]
fn adding_then_removing_some_preserves_each_existing_minimum_version_and_callbacks() {
    use paintcraft_core::types::filters::{BoxBlurV1, Descriptor, VibranceRgbV1};
    use paintcraft_document::{FilterInstance, FilterMask, FilterStack};
    let budget = SharedBudget::new(1 << 20);
    let holder = open(
        &specimen("disabled"),
        &budget,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    let own = holder.layers[&holder.roots[0]]
        .mask
        .as_ref()
        .unwrap()
        .clone();
    for version in 1..=6 {
        let mut doc = read(include_bytes!(
            "../../tests/pcnd-filter-fixtures/legacy-v1.pcnd"
        ))
        .unwrap();
        if version == 2 {
            // The independent metadata-only fixture has no layers. Retain its
            // actual admitted attachment on the existing root-bearing source.
            doc.source_metadata = read(include_bytes!(
                "../../tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
            ))
            .unwrap()
            .source_metadata;
            assert!(doc.source_metadata.is_some());
        }
        if version >= 3 {
            let descriptor = match version {
                5 => Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap()),
                6 => Descriptor::VibranceRgbV1(VibranceRgbV1::new(0).unwrap()),
                _ => Descriptor::InvertRgbV1,
            };
            let mut filter = FilterInstance::new(Uuid::from_u128(9), false, descriptor);
            if version == 4 {
                filter = filter.with_mask(Some(FilterMask::new(false, own.coverage().clone())))
            }
            doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
                FilterStack::new(vec![filter]).unwrap();
        }
        let calls = Cell::new(0);
        let mut before = Vec::new();
        write_compact_to_checked(&doc, &mut before, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(before[4], version);
        doc.layers.get_mut(&doc.roots[0]).unwrap().mask = Some(own.clone());
        assert_eq!(write(&doc).unwrap()[4], 7);
        doc.layers.get_mut(&doc.roots[0]).unwrap().mask = None;
        let after_calls = Cell::new(0);
        let mut after = Vec::new();
        write_compact_to_checked(&doc, &mut after, &|| {
            after_calls.set(after_calls.get() + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(after, before, "version{version}");
        assert_eq!(calls.get(), after_calls.get());
    }
}
