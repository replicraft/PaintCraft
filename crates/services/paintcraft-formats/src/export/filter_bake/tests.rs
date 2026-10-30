use super::*;
use crate::{
    export::metadata_policy,
    registry::{Family, Operation},
};
use paintcraft_core::types::{
    Rgba,
    filters::{BrightnessContrastV1, Descriptor},
};
use paintcraft_document::{FilterInstance, FilterStack};
use std::cell::Cell;
const PNG: Target = Target::new(Family::Png, Operation::ExportFlat);
fn document(filtered: bool) -> Document {
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([17, 31, 47, 0])),
        7,
        paintcraft_document::POLICY,
    )
    .unwrap();
    if filtered {
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters = stack(true);
    }
    doc
}
fn stack(enabled: bool) -> FilterStack {
    FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(1),
        enabled,
        Descriptor::InvertRgbV1,
    )])
    .unwrap()
}
fn layered() -> Document {
    let mut doc = document(true);
    let mut second = doc.layers[&doc.roots[0]].clone();
    second.id = Uuid::from_u128(2);
    second.filters = Default::default();
    doc.roots.push(second.id);
    doc.layers.insert(second.id, second);
    doc
}
fn capture(doc: &Document) -> Policy {
    Policy::capture(doc, PNG, Intent::Bake, &|| Ok(())).unwrap()
}
#[test]
fn explicit_bake_requires_all_intentions_even_when_hidden_or_disabled() {
    for (enabled, visible) in [(true, true), (false, true), (true, false), (false, false)] {
        let mut doc = document(true);
        let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
        layer.filters = stack(enabled);
        layer.visible = visible;
        let before = doc.clone();
        assert_eq!(
            Policy::default()
                .validate(&doc, PNG, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            Policy::capture(&doc, PNG, Intent::Refuse, &|| Ok(()))
                .unwrap()
                .validate(&doc, PNG, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert!(capture(&doc).validate(&doc, PNG, &|| Ok(())).unwrap());
        assert!(doc.same_semantics(&before));
    }
    assert!(
        BAKED_REASON.parse::<Intent>().is_err(),
        "receipt names never mint authority"
    );
    assert!(metadata_policy::OMITTED_REASON.parse::<Intent>().is_err());
    assert_eq!("bake".parse::<Intent>().unwrap(), Intent::Bake);
    assert_eq!("refuse".parse::<Intent>().unwrap(), Intent::Refuse);
}
#[test]
fn empty_sources_emit_no_hypothetical_loss_but_captured_empty_witness_is_still_bound() {
    for doc in [
        document(false),
        paintcraft_document::new_document(2, 1, None, 0, paintcraft_document::POLICY).unwrap(),
    ] {
        assert!(!Policy::default().validate(&doc, PNG, &|| Ok(())).unwrap());
        let policy = capture(&doc);
        assert!(!policy.validate(&doc, PNG, &|| Ok(())).unwrap());
        let mut changed = doc.clone();
        changed.state_id = Uuid::from_u128(8);
        assert_eq!(
            policy.validate(&changed, PNG, &|| Ok(())).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
    }
    let doc = document(false);
    let policy = capture(&doc);
    let mut changed = doc.clone();
    changed.layers.get_mut(&changed.roots[0]).unwrap().filters = stack(false);
    assert_eq!(
        policy.validate(&changed, PNG, &|| Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
}
#[test]
fn same_uuid_candidates_reject_changed_stack_flags_order_geometry_and_target() {
    let doc = layered();
    let policy = capture(&doc);
    let first = doc.roots[0];
    for case in 0..13 {
        let mut changed = doc.clone();
        match case {
            0 => changed.document_id = Uuid::from_u128(70),
            1 => changed.state_id = Uuid::from_u128(71),
            2 => changed.layers.get_mut(&first).unwrap().filters = stack(false),
            3 => {
                changed.layers.get_mut(&first).unwrap().filters =
                    FilterStack::new(vec![FilterInstance::new(
                        Uuid::from_u128(1),
                        true,
                        Descriptor::BrightnessContrastV1(
                            BrightnessContrastV1::new(10, 50).unwrap(),
                        ),
                    )])
                    .unwrap()
            }
            4 => changed.layers.get_mut(&first).unwrap().visible = false,
            5 => changed.layers.get_mut(&first).unwrap().opacity = 127,
            6 => changed.layers.get_mut(&first).unwrap().origin = (1, -1),
            7 => changed.canvas = (3, 1),
            8 => changed.roots.swap(0, 1),
            9 => {
                changed.layers.get_mut(&first).unwrap().raster =
                    paintcraft_raster::RasterLayer::new(3, 1, Rgba([17, 31, 47, 0]))
            }
            10 => {
                changed.layers.get_mut(&first).unwrap().filters =
                    FilterStack::new(vec![FilterInstance::new(
                        Uuid::from_u128(70),
                        true,
                        Descriptor::InvertRgbV1,
                    )])
                    .unwrap()
            }
            11 => {
                let old = changed.roots[1];
                let mut layer = changed.layers.remove(&old).unwrap();
                layer.id = Uuid::from_u128(71);
                changed.roots[1] = layer.id;
                changed.layers.insert(layer.id, layer);
            }
            12 => {
                changed.layers.get_mut(&first).unwrap().raster =
                    paintcraft_raster::RasterLayer::new(2, 1, Rgba([18, 31, 47, 0]))
            }
            _ => unreachable!(),
        }
        assert_eq!(
            policy.validate(&changed, PNG, &|| Ok(())).unwrap_err().code,
            ErrorCode::ConflictStaleBase,
            "case{case}"
        );
    }
    for target in [
        Target::new(Family::Jpeg, Operation::ExportFlat),
        Target::new(Family::Png, Operation::SaveAsCopy),
    ] {
        assert_eq!(
            policy.validate(&doc, target, &|| Ok(())).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
    }
}
#[test]
fn preserving_pcnd_read_and_non_icon_asset_targets_never_mint_a_bake_witness() {
    for doc in [document(false), document(true)] {
        for target in [
            Target::new(Family::Pcnd, Operation::SaveAsCopy),
            Target::new(Family::Pcnd, Operation::ExportFlat),
            Target::new(Family::Png, Operation::SavePreserving),
            Target::new(Family::Png, Operation::Open),
            Target::new(Family::Png, Operation::ExportAsset),
        ] {
            assert_eq!(
                Policy::capture(&doc, target, Intent::Bake, &|| Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema
            );
            assert_eq!(
                capture(&doc)
                    .validate(&doc, target, &|| Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema
            );
        }
        for target in [
            Target::new(Family::Ico, Operation::ExportAsset),
            Target::new(Family::Cur, Operation::ExportAsset),
            Target::new(Family::Icns, Operation::ExportAsset),
        ] {
            let policy = Policy::capture(&doc, target, Intent::Bake, &|| Ok(())).unwrap();
            assert_eq!(
                policy.validate(&doc, target, &|| Ok(())).unwrap(),
                !doc.layers[&doc.roots[0]].filters.is_empty()
            );
        }
    }
}
#[test]
fn malformed_root_inventories_fail_typed_before_any_loss_authorization() {
    for case in 0..5 {
        let mut doc = layered();
        match case {
            0 => {
                doc.roots.pop();
            }
            1 => doc.roots[1] = doc.roots[0],
            2 => doc.roots[0] = Uuid::from_u128(999),
            3 => doc.layers.get_mut(&doc.roots[0]).unwrap().id = Uuid::from_u128(999),
            4 => doc.roots.resize(LAYER_COUNT_MAX + 1, doc.roots[0]),
            _ => unreachable!(),
        }
        let expected = if case == 4 {
            ErrorCode::ResourceLimit
        } else {
            ErrorCode::InputSchema
        };
        assert_eq!(
            Policy::capture(&doc, PNG, Intent::Bake, &|| Ok(()))
                .unwrap_err()
                .code,
            expected
        );
        assert_eq!(
            Policy::default()
                .validate(&doc, PNG, &|| Ok(()))
                .unwrap_err()
                .code,
            expected
        );
    }
}
#[test]
fn cancellation_at_every_capture_or_validation_checkpoint_keeps_the_document_intact() {
    let doc = layered();
    let before = doc.clone();
    let policy = capture(&doc);
    for validating in [false, true] {
        let calls = Cell::new(0);
        let count = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        if validating {
            policy.validate(&doc, PNG, &count).unwrap();
        } else {
            Policy::capture(&doc, PNG, Intent::Bake, &count).unwrap();
        }
        let checkpoints = calls.get();
        assert!(checkpoints >= doc.roots.len() + 2);
        for stop in 1..=checkpoints {
            let calls = Cell::new(0);
            let check = || {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancelled bake-policy inspection",
                    ))
                } else {
                    Ok(())
                }
            };
            let error = if validating {
                policy.validate(&doc, PNG, &check).unwrap_err()
            } else {
                Policy::capture(&doc, PNG, Intent::Bake, &check).unwrap_err()
            };
            assert_eq!(error.code, ErrorCode::OperationCancelled);
            assert!(doc.same_semantics(&before));
            assert_eq!(doc.state_id, before.state_id);
        }
    }
}
#[test]
fn explicit_metadata_omission_does_not_authorize_baking_filters() {
    use paintcraft_core::types::resources::SharedBudget;
    use paintcraft_document::source_metadata::{
        BYTES_MAX, OrientationPolicy, SourceColorModel, SourceDescriptor, SourceFormat,
        SourceMetadata,
    };
    let mut doc = document(true);
    doc.source_metadata = Some(
        SourceMetadata::capture(
            SourceDescriptor {
                format: SourceFormat::Png,
                source_model: SourceColorModel::Rgb,
                source_sha256: [3; 32],
                source_bytes: 64,
                selection: None,
                source_dimensions: doc.canvas,
                output_dimensions: doc.canvas,
                orientation: Some(1),
                orientation_policy: OrientationPolicy::Unmodified,
                resolution: None,
            },
            None,
            BYTES_MAX,
            &SharedBudget::new(BYTES_MAX),
            &|| Ok(()),
        )
        .unwrap(),
    );
    let omission =
        metadata_policy::Policy::capture(&doc, PNG, metadata_policy::Intent::Omit).unwrap();
    assert!(omission.validate(&doc, PNG).unwrap());
    assert_eq!(
        Policy::default()
            .validate(&doc, PNG, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(capture(&doc).validate(&doc, PNG, &|| Ok(())).unwrap());
    assert_eq!(
        metadata_policy::Policy::default()
            .validate(&doc, PNG)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}
