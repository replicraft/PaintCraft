use super::*;
use paintcraft_core::types::styles::{ColorOverlayRgbV1, EffectBlendMode};
fn descriptor() -> Descriptor {
    Descriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
        [0, 127, 255],
        0,
        EffectBlendMode::Multiply,
    ))
}
fn packet(schema: &str, operations: Vec<Value>) -> Value {
    Value::object([
        ("schema", Value::str(schema)),
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
fn operations() -> Vec<StyleOp> {
    let layer = LayerRef::Id(Uuid::from_u128(2));
    let style = StyleRef::Id(Uuid::from_u128(3));
    vec![
        StyleOp::Add {
            layer: layer.clone(),
            alias: "overlay".into(),
            descriptor: descriptor(),
            enabled: false,
            to_index: None,
        },
        StyleOp::Update {
            layer: layer.clone(),
            style: style.clone(),
            descriptor: descriptor(),
        },
        StyleOp::SetEnabled {
            layer: layer.clone(),
            style: style.clone(),
            enabled: false,
        },
        StyleOp::Move {
            layer: layer.clone(),
            style: style.clone(),
            to_index: 0,
        },
        StyleOp::Duplicate {
            layer: layer.clone(),
            style: style.clone(),
            alias: "copy".into(),
            to_index: Some(0),
        },
        StyleOp::Remove { layer, style },
    ]
}
#[test]
fn all_six_closed_operations_typed_roundtrip_new_profile_old_profiles_refuse() {
    for operation in operations() {
        let value = operation.to_value().unwrap();
        assert_eq!(operation.required_batch_version(), 10);
        assert_eq!(
            Batch::schema_for_operations(std::slice::from_ref(&value)),
            LAYER_STYLES_BATCH_SCHEMA
        );
        let b = Batch::from_generated_value(packet(LAYER_STYLES_BATCH_SCHEMA, vec![value.clone()]))
            .unwrap();
        assert_eq!(b.version, 10);
        assert_eq!(b.operations, vec![Op::Style(operation)]);
        for old in [
            "paintcraft.batch/v1",
            "paintcraft.batch/v2",
            "paintcraft.batch/v3",
            "paintcraft.batch/v4",
            "paintcraft.batch/v5",
            CATALOG_BATCH_SCHEMA,
            LAYER_APPEARANCE_BATCH_SCHEMA,
            LAYER_BLEND_BATCH_SCHEMA,
            STRETCH_SKEW_BATCH_SCHEMA,
        ] {
            assert_eq!(
                Batch::from_generated_value(packet(old, vec![value.clone()]))
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema
            );
        }
    }
}
#[test]
fn exact_sorted_style_feature_union_not_numeric_filter_catalog() {
    let style = operations().remove(0).to_value().unwrap();
    let filter = Value::object([
        ("op", Value::str("filter.add")),
        (
            "layer_ref",
            Value::object([("id", Value::str(Uuid::from_u128(2).to_string()))]),
        ),
        ("ref", Value::str("filter")),
        (
            "descriptor",
            Value::object([("kind", Value::str("invert-rgb-v1"))]),
        ),
    ]);
    let blend = Value::object([
        ("op", Value::str("layer.set_blend_mode")),
        (
            "layer_ref",
            Value::object([("id", Value::str(Uuid::from_u128(2).to_string()))]),
        ),
        ("blend_mode", Value::str("screen")),
    ]);
    let mut packet = packet(LAYER_STYLES_BATCH_SCHEMA, vec![style, filter, blend]);
    let Value::Object(ref mut fields) = packet else {
        panic!()
    };
    fields.insert(
        "required_filters".into(),
        Value::Array(vec![Value::str("invert-rgb-v1")]),
    );
    fields.insert(
        "required_layer_features".into(),
        Value::Array(vec![
            Value::str("layer-blend-modes.v1"),
            Value::str("layer-styles.v1"),
        ]),
    );
    assert!(Batch::from_value(&packet).is_ok());
    for wrong in [
        vec![],
        vec![Value::str("layer-styles.v1")],
        vec![
            Value::str("layer-styles.v1"),
            Value::str("layer-blend-modes.v1"),
        ],
        vec![
            Value::str("layer-blend-modes.v1"),
            Value::str("layer-styles.v1"),
            Value::str("layer-styles.v1"),
        ],
    ] {
        let Value::Object(mut fields) = packet.clone() else {
            panic!()
        };
        fields.insert("required_layer_features".into(), Value::Array(wrong));
        assert!(Batch::from_value(&Value::Object(fields)).is_err());
    }
}
#[test]
fn malformed_fields_names_references_and_style_only_descriptor_domain_refuse() {
    let value = operations().remove(0).to_value().unwrap();
    for (key, bad) in [
        ("unknown", Value::Bool(true)),
        ("enabled", Value::Int(1)),
        ("to_index", Value::Int(257)),
        ("ref", Value::str("")),
        ("op", Value::str("layer.style.reorder")),
        (
            "descriptor",
            Value::object([("kind", Value::str("invert-rgb-v1"))]),
        ),
    ] {
        let Value::Object(mut fields) = value.clone() else {
            panic!()
        };
        fields.insert(key.into(), bad);
        assert!(parse(&Value::Object(fields)).is_err());
    }
    assert!(
        StyleOp::Remove {
            layer: LayerRef::Alias("unstable".into()),
            style: StyleRef::Id(Uuid::from_u128(3))
        }
        .stable_uuid_refs()
        .is_none()
    );
}
