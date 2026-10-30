//! Additional descriptors use v4 without widening the frozen v2/v3 grammars.
use paintcraft_commands::{Batch, FilterOp, FilterRef, LayerRef, MaskOp, Op};
use paintcraft_core::types::{
    ErrorCode,
    filters::{BlackWhiteV1, Descriptor, PosterizeRgbV1, ThresholdLumaV1},
    json::{self, Value},
};
use uuid::Uuid;
fn packet(version: u8, operations: Vec<Value>) -> Value {
    Value::object([
        ("schema", Value::str(format!("paintcraft.batch/v{version}"))),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str("paintcraft.core/default-v1")),
        (
            "base",
            Value::object([
                ("state_id", Value::str(Uuid::from_u128(1).to_string())),
                ("publication_epoch", Value::str("0")),
            ]),
        ),
        ("operations", Value::Array(operations)),
    ])
}
fn add(descriptor: Descriptor) -> FilterOp {
    FilterOp::Add {
        layer: LayerRef::Alias("layer".into()),
        alias: "f".into(),
        descriptor,
        enabled: false,
        to_index: Some(256),
    }
}
fn update(descriptor: Descriptor) -> FilterOp {
    FilterOp::Update {
        layer: LayerRef::Id(Uuid::from_u128(2)),
        filter: FilterRef::Alias("f".into()),
        descriptor,
    }
}
#[test]
fn new_add_update_roundtrip_edges_disabled_indices_and_closed_legacy_profiles() {
    for descriptor in [
        Descriptor::BlackWhiteRgbV1(BlackWhiteV1::new(0).unwrap()),
        Descriptor::BlackWhiteRgbV1(BlackWhiteV1::new(100).unwrap()),
        Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(0).unwrap()),
        Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(255).unwrap()),
        Descriptor::PosterizeRgbV1(PosterizeRgbV1::new(2).unwrap()),
        Descriptor::PosterizeRgbV1(PosterizeRgbV1::new(32).unwrap()),
        Descriptor::SolarizeRgbV1,
    ] {
        for op in [add(descriptor), update(descriptor)] {
            assert_eq!(op.required_batch_version(), 4);
            let wire = op.to_value().unwrap();
            assert_eq!(
                Batch::schema_for_operations(std::slice::from_ref(&wire)),
                "paintcraft.batch/v4"
            );
            let parsed = Batch::from_value(&packet(4, vec![wire.clone()])).unwrap();
            assert_eq!(parsed.operations, [Op::Filter(op)]);
            for version in [1, 2, 3] {
                assert_eq!(
                    Batch::from_value(&packet(version, vec![wire.clone()]))
                        .unwrap_err()
                        .code,
                    ErrorCode::InputSchema
                );
            }
        }
    }
}
#[test]
fn packet_selectors_keep_legacy_mask_and_scalar_operations_at_lowest_profile() {
    let old = add(Descriptor::InvertRgbV1);
    let old_wire = old.to_value().unwrap();
    assert_eq!(old.required_batch_version(), 2);
    assert_eq!(
        Batch::schema_for_operations(std::slice::from_ref(&old_wire)),
        "paintcraft.batch/v2"
    );
    let mask = FilterOp::Mask {
        layer: LayerRef::Id(Uuid::from_u128(2)),
        filter: FilterRef::Id(Uuid::from_u128(3)),
        operation: MaskOp::Remove,
    };
    let mask_wire = mask.to_value().unwrap();
    assert_eq!(mask.required_batch_version(), 3);
    assert_eq!(
        Batch::schema_for_operations(&[old_wire.clone(), mask_wire.clone()]),
        "paintcraft.batch/v3"
    );
    let new = add(Descriptor::SolarizeRgbV1).to_value().unwrap();
    assert_eq!(
        Batch::schema_for_operations(&[mask_wire, new, old_wire]),
        "paintcraft.batch/v4"
    );
    let layer = LayerRef::Id(Uuid::from_u128(2));
    let filter = FilterRef::Id(Uuid::from_u128(3));
    for op in [
        FilterOp::SetEnabled {
            layer: layer.clone(),
            filter: filter.clone(),
            enabled: false,
        },
        FilterOp::Reorder {
            layer: layer.clone(),
            filter: filter.clone(),
            to_index: 0,
        },
        FilterOp::Duplicate {
            layer: layer.clone(),
            filter: filter.clone(),
            alias: "copy".into(),
            to_index: None,
        },
        FilterOp::Remove { layer, filter },
    ] {
        assert_eq!(op.required_batch_version(), 2);
        let value = op.to_value().unwrap();
        assert_eq!(
            Batch::schema_for_operations(std::slice::from_ref(&value)),
            "paintcraft.batch/v2"
        );
        Batch::from_value(&packet(2, vec![value])).unwrap();
    }
    assert_eq!(
        Batch::schema_for_operation_fields([("filter.update", Some("threshold-luma-v1"))]),
        "paintcraft.batch/v4"
    );
    assert_eq!(
        Batch::schema_for_operation_fields([("filter.duplicate", Some("threshold-luma-v1"))]),
        "paintcraft.batch/v2"
    );
}
#[test]
fn v4_still_rejects_wrong_types_extra_fields_missing_values_and_unknown_versions() {
    for text in [
        r#"{"kind":"black-white-rgb-v1","strength":-1}"#,
        r#"{"kind":"black-white-rgb-v1","strength":101}"#,
        r#"{"kind":"black-white-rgb-v1","strength":true}"#,
        r#"{"kind":"black-white-rgb-v1"}"#,
        r#"{"kind":"threshold-luma-v1","level":256}"#,
        r#"{"kind":"threshold-luma-v1","level":-1}"#,
        r#"{"kind":"threshold-luma-v1","level":128,"strength":100}"#,
        r#"{"kind":"posterize-rgb-v1","levels":1}"#,
        r#"{"kind":"posterize-rgb-v1","levels":33}"#,
        r#"{"kind":"posterize-rgb-v1","levels":null}"#,
        r#"{"kind":"solarize-rgb-v1","threshold":128}"#,
        r#"{"kind":"solarize-rgb-v2"}"#,
    ] {
        let mut wire = add(Descriptor::InvertRgbV1).to_value().unwrap();
        let Value::Object(fields) = &mut wire else {
            unreachable!()
        };
        fields.insert("descriptor".into(), json::parse(text.as_bytes()).unwrap());
        assert_eq!(
            Batch::from_value(&packet(4, vec![wire])).unwrap_err().code,
            ErrorCode::InputSchema,
            "{text}"
        );
    }
    // The integer-only wire reader rejects decimal notation before a Value exists.
    assert!(json::parse(br#"{"kind":"black-white-rgb-v1","strength":1.0}"#).is_err());
    let mut op = add(Descriptor::SolarizeRgbV1);
    if let FilterOp::Add {
        alias, to_index, ..
    } = &mut op
    {
        *alias = "bad alias".into();
        *to_index = Some(usize::MAX);
    }
    assert_eq!(op.to_value().unwrap_err().code, ErrorCode::InputSchema);
}
