//! Independent A8-band wire controls, including aggregate resident ownership.
use super::*;
use paintcraft_raster::coverage_mask::Limits;
use std::{cell::Cell, path::Path};
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/pcnd-mask-fixtures")
            .join(name),
    )
    .unwrap()
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
        65536,
        1,
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
fn independent_bands_zero_disabled_mixed_and_repeated_roundtrip_exact() {
    let oracle = paintcraft_core::types::json::parse(&fixture("oracle.json")).unwrap();
    for row in oracle.get("fixtures").unwrap().as_array().unwrap() {
        if !row.get("accepted").unwrap().as_bool().unwrap() {
            continue;
        }
        let name = row.get("file").unwrap().as_str().unwrap();
        let bytes = fixture(name);
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&bytes)),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        let budget = SharedBudget::new(1 << 20);
        let document = open(&bytes, &budget, Limits::default(), &|| Ok(())).unwrap();
        let layer = &document.layers[&document.roots[0]];
        let plane = name == "opaque-half.pcnd";
        assert_eq!(
            layer.raster.get(0, 0),
            if plane {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([17, 32, 255, 0])
            }
        );
        assert_eq!(
            layer.raster.get(1, 0),
            if plane {
                Rgba([0, 255, 0, 255])
            } else {
                Rgba([0, 127, 200, 255])
            }
        );
        for filter in layer.filters.iter() {
            let Some(mask) = filter.mask() else { continue };
            assert_eq!(mask.enabled(), name != "disabled.pcnd");
            assert_eq!(
                mask.coverage().domain(),
                if plane {
                    paintcraft_core::types::Rect::new(0, 0, 2, 1).unwrap()
                } else {
                    paintcraft_core::types::Rect::new(-2, -1, 6, 3).unwrap()
                }
            );
            if plane {
                assert_eq!(mask.coverage().coverage(0, 0), 128);
                assert_eq!(mask.coverage().coverage(1, 0), 0);
                continue;
            }
            for (x, y, value) in [
                (-2, -1, 128),
                (0, -1, 0),
                (1, -1, 255),
                (0, 1, 64),
                (4, 1, 0),
            ] {
                assert_eq!(
                    mask.coverage().coverage(x, y),
                    if name == "zero.pcnd" { 0 } else { value },
                    "{name} {x},{y}"
                );
            }
        }
        assert_eq!(write(&document).unwrap(), bytes, "{name}");
        assert_eq!(write_compact(&document).unwrap(), bytes, "{name}");
        check_metadata_admission(&document, &|| Ok(())).unwrap();
        check_compact_metadata_admission(&document, &|| Ok(())).unwrap();
        let header = probe(&bytes).unwrap();
        header.require_supported().unwrap();
        assert_eq!(header.document_schema, "paintcraft.document/v4");
        assert!(
            header
                .required_features
                .contains(&LAYER_MASKS_FEATURE.into())
        );
        assert!(
            header
                .required_features
                .contains(&LAYER_FILTERS_FEATURE.into())
        );
        if name != "zero.pcnd" {
            assert!(budget.usage().reserved > 0);
        }
        drop(document);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn malformed_canonical_bands_and_unknown_disabled_codec_refuse_without_charge() {
    let oracle = paintcraft_core::types::json::parse(&fixture("oracle.json")).unwrap();
    for row in oracle.get("fixtures").unwrap().as_array().unwrap() {
        if row.get("accepted").unwrap().as_bool().unwrap() {
            continue;
        }
        let name = row.get("file").unwrap().as_str().unwrap();
        let budget = SharedBudget::new(1 << 20);
        let result = open(&fixture(name), &budget, Limits::default(), &|| Ok(()));
        let expected = if name == "unknown-disabled.pcnd" {
            ErrorCode::CapabilityUnsupported
        } else {
            ErrorCode::CodecFailed
        };
        assert_eq!(result.unwrap_err().code, expected, "{name}");
        assert_eq!(budget.usage().reserved, 0, "{name}");
    }
}
#[test]
fn explicit_admission_counts_are_aggregate_and_live_documents_share_realm() {
    let bytes = fixture("repeated.pcnd");
    let budget = SharedBudget::new(1 << 20);
    for limits in [
        Limits {
            bands: 5,
            ..Limits::default()
        },
        Limits {
            runs: 8,
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
    assert_eq!(
        read(&bytes).unwrap_err().code,
        ErrorCode::CapabilityUnsupported,
        "legacy caller cannot allocate unowned masks"
    );
    let doc = open(&bytes, &budget, Limits::default(), &|| Ok(())).unwrap();
    let retained = budget.usage().reserved;
    assert!(retained > 0);
    let _other = budget.reserve((1 << 20) - retained).unwrap();
    assert_eq!(
        open(&bytes, &budget, Limits::default(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    drop(doc);
    assert_eq!(budget.usage().reserved, (1 << 20) - retained);
}
#[test]
fn cancelled_mask_read_drops_all_partial_retained_and_scratch_charges() {
    let bytes = fixture("repeated.pcnd");
    let calls = Cell::new(0);
    let budget = SharedBudget::new(1 << 20);
    drop(
        open(&bytes, &budget, Limits::default(), &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap(),
    );
    for stop in 1..=calls.get() {
        let count = Cell::new(0);
        let error = open(&bytes, &budget, Limits::default(), &|| {
            count.set(count.get() + 1);
            if count.get() == stop {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "mask cancellation",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(
            error.code,
            ErrorCode::OperationCancelled,
            "checkpoint{stop}"
        );
        assert_eq!(budget.usage().reserved, 0, "checkpoint{stop}");
    }
}

#[test]
fn quote_prices_repeated_serialized_masks_and_denies_before_allocation() {
    let bytes = fixture("bands.pcnd");
    let budget = SharedBudget::new(1 << 20);
    let mut doc = open(&bytes, &budget, Limits::default(), &|| Ok(())).unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    let original = layer.filters.as_slice()[0].clone();
    layer.filters = paintcraft_document::FilterStack::new(
        (0..3)
            .map(|id| original.with_id(Uuid::from_u128(id)))
            .collect(),
    )
    .unwrap();
    let facts = coverage_facts(&doc, &|| Ok(())).unwrap();
    assert_eq!((facts.masks, facts.bands, facts.runs), (3, 6, 9));
    assert!(
        facts.decode_bytes > budget.usage().reserved * 3,
        "parse scratch and per-instance copies must be priced"
    );
    let empty = SharedBudget::new(1 << 20);
    assert_eq!(
        read_with_admission_checked(
            &bytes,
            2,
            65536,
            1,
            ReadAdmission {
                metadata: MetadataAdmission {
                    bytes: 0,
                    budget: &empty
                },
                coverage: CoverageAdmission {
                    bytes: 0,
                    budget: &empty,
                    limits: Limits::default()
                },
            },
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        empty.activity(),
        0,
        "byte admission precedes all mask allocations"
    );
    layer_reset(&mut doc);
    assert_eq!(
        write(&doc).unwrap()[4],
        3,
        "removing final stored mask selects unchanged PCND3"
    );
    fn layer_reset(doc: &mut Document) {
        let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
        layer.filters = paintcraft_document::FilterStack::new(
            layer.filters.iter().map(|f| f.with_mask(None)).collect(),
        )
        .unwrap();
    }
}

#[test]
fn aggregate_shared_mask_counts_refuse_before_output_even_when_wire_fits() {
    use paintcraft_core::types::{Rect, filters::Descriptor};
    use paintcraft_document::{FilterInstance, FilterMask, FilterStack};
    use paintcraft_raster::coverage_mask::{A8Mask, Band, Run};
    let budget = SharedBudget::new(64 << 20);
    let count = Limits::default().runs / 2 + 1;
    let runs: Vec<_> = (0..count)
        .map(|x| Run {
            x0: x as i64,
            x1: x as i64 + 1,
            coverage: if x % 2 == 0 { 1 } else { 2 },
        })
        .collect();
    let mask = A8Mask::from_bands(
        Rect::new(0, 0, count as i64, 1).unwrap(),
        &[Band {
            y0: 0,
            y1: 1,
            runs: &runs,
        }],
        &budget,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(mask.stats().runs <= Limits::default().runs);
    let mut doc = read(include_bytes!(
        "../../tests/pcnd-filter-fixtures/legacy-v1.pcnd"
    ))
    .unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.filters = FilterStack::new(
        (0..2)
            .map(|id| {
                FilterInstance::new(Uuid::from_u128(id), true, Descriptor::InvertRgbV1)
                    .with_mask(Some(FilterMask::new(true, mask.clone())))
            })
            .collect(),
    )
    .unwrap();
    let wire_bound =
        super::binary::mask_metadata_bound(layer.filters.as_slice()[0].mask()).unwrap() * 2;
    let facts = coverage_facts(&doc, &|| Ok(())).unwrap();
    assert_eq!(facts.runs, 2 * count as u64);
    assert!(
        wire_bound < DOCUMENT_MAX,
        "wire metadata bound itself admits both masks"
    );
    let mut output = Vec::new();
    assert_eq!(
        write_to(&doc, &mut output).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(
        output.is_empty(),
        "aggregate count must precede first output byte"
    );
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
    // Actual persistent admission rejects the second shared instance atomically.
    let layer_id = doc.roots[0];
    let first = doc.layers[&layer_id].filters.as_slice()[0].clone();
    doc.layers.get_mut(&layer_id).unwrap().filters = FilterStack::new(vec![first.clone()]).unwrap();
    check_compact_metadata_admission(&doc, &|| Ok(())).unwrap();
    let mut session = paintcraft_editor::session::Session::new(doc)
        .with_persistent_validator(Some(check_compact_metadata_admission));
    let before = session.snapshot();
    let ticket = session.ticket();
    let retained = budget.usage().reserved;
    let operation = paintcraft_commands::FilterOp::Duplicate {
        layer: paintcraft_commands::LayerRef::Id(layer_id),
        filter: paintcraft_commands::FilterRef::Id(first.id()),
        alias: "copy".into(),
        to_index: None,
    }
    .to_value()
    .unwrap();
    use paintcraft_core::types::json::Value;
    let batch = paintcraft_commands::Batch::from_value(&Value::object([
        ("schema", Value::str("paintcraft.batch/v2")),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str(paintcraft_document::POLICY)),
        (
            "base",
            Value::object([
                (
                    "state_id",
                    Value::str(session.document().state_id.to_string()),
                ),
                (
                    "publication_epoch",
                    Value::str(ticket.publication_epoch.to_string()),
                ),
            ]),
        ),
        ("operations", Value::Array(vec![operation])),
    ]))
    .unwrap();
    assert_eq!(
        session.apply(&batch).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(std::sync::Arc::ptr_eq(&before, &session.snapshot()));
    assert_eq!(session.ticket(), ticket);
    assert_eq!(session.document().layers[&layer_id].filters.len(), 1);
    assert_eq!(
        budget.usage().reserved,
        retained,
        "failed publication retains only original shared payload"
    );
}
