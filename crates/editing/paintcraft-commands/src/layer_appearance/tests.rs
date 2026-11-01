use super::*;
fn packet(schema: &str, operations: Vec<Value>) -> Value {
    Value::object([
        ("schema", Value::str(schema)),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str("paintcraft.core/default-v1")),
        (
            "base",
            Value::object([
                ("state_id", Value::str(Uuid::from_u128(17).to_string())),
                ("publication_epoch", Value::str("0")),
            ]),
        ),
        ("operations", Value::Array(operations)),
    ])
}
fn mask() -> Value {
    LayerMaskOp::Remove {
        layer: LayerRef::Id(Uuid::from_u128(19)),
    }
    .to_value()
    .unwrap()
}
#[test]
fn generated_new_envelope_fills_both_exact_inventories_and_old_envelopes_stay_closed() {
    let op = mask();
    assert_eq!(
        Batch::schema_for_operations(std::slice::from_ref(&op)),
        LAYER_APPEARANCE_BATCH_SCHEMA
    );
    let value = packet(LAYER_APPEARANCE_BATCH_SCHEMA, vec![op.clone()]);
    assert_eq!(
        Batch::from_generated_value(value.clone()).unwrap().version,
        7
    );
    assert!(Batch::from_value(&value).is_err());
    for schema in [
        "paintcraft.batch/v1",
        "paintcraft.batch/v2",
        "paintcraft.batch/v3",
        "paintcraft.batch/v4",
        "paintcraft.batch/v5",
        CATALOG_BATCH_SCHEMA,
    ] {
        let mut value = packet(schema, vec![op.clone()]);
        if schema == CATALOG_BATCH_SCHEMA {
            let Value::Object(ref mut f) = value else {
                unreachable!()
            };
            f.insert("required_filters".into(), Value::Array(vec![]));
        }
        assert_eq!(
            Batch::from_value(&value).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
}
#[test]
fn explicit_requirements_reject_missing_unused_duplicate_unknown_and_wrong_types() {
    let mut value = packet(LAYER_APPEARANCE_BATCH_SCHEMA, vec![mask()]);
    let Value::Object(ref mut f) = value else {
        unreachable!()
    };
    f.insert("required_filters".into(), Value::Array(vec![]));
    f.insert(
        "required_layer_features".into(),
        Value::Array(vec![Value::str("layer-masks.v1")]),
    );
    assert_eq!(Batch::from_value(&value).unwrap().version, 7);
    assert!(Batch::from_generated_value(value.clone()).is_err());
    for requirement in [
        Value::Array(vec![]),
        Value::Array(vec![
            Value::str("layer-masks.v1"),
            Value::str("layer-masks.v1"),
        ]),
        Value::Array(vec![Value::str("future.v1")]),
        Value::str("layer-masks.v1"),
    ] {
        let mut bad_value = value.clone();
        let Value::Object(ref mut f) = bad_value else {
            unreachable!()
        };
        f.insert("required_layer_features".into(), requirement);
        assert_eq!(
            Batch::from_value(&bad_value).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
    let mut unused = value.clone();
    let Value::Object(ref mut f) = unused else {
        unreachable!()
    };
    f.insert(
        "required_filters".into(),
        Value::Array(vec![Value::str("invert-rgb-v1")]),
    );
    assert!(Batch::from_value(&unused).is_err());
}
#[test]
fn new_envelope_admits_mixed_catalog_and_selection_with_exact_filter_union() {
    let layer = LayerRef::Id(Uuid::from_u128(19));
    let filter = FilterOp::Add {
        layer,
        alias: "f".into(),
        descriptor: paintcraft_core::types::filters::Descriptor::LevelsRgbV1(Default::default()),
        enabled: false,
        to_index: None,
    }
    .to_value()
    .unwrap();
    let mut value = packet(
        LAYER_APPEARANCE_BATCH_SCHEMA,
        vec![filter.clone(), mask(), filter],
    );
    let Value::Object(ref mut f) = value else {
        unreachable!()
    };
    f.insert(
        "required_filters".into(),
        Value::Array(vec![Value::str("levels-rgb-v1")]),
    );
    f.insert(
        "required_layer_features".into(),
        Value::Array(vec![Value::str("layer-masks.v1")]),
    );
    assert_eq!(Batch::from_value(&value).unwrap().operations.len(), 3);
    let ordinary = Value::object([("op", Value::str("selection.clear"))]);
    assert_eq!(
        Batch::schema_for_operations(std::slice::from_ref(&ordinary)),
        "paintcraft.batch/v2"
    );
    assert_eq!(
        Batch::from_generated_value(packet(LAYER_APPEARANCE_BATCH_SCHEMA, vec![ordinary]))
            .unwrap()
            .version,
        7
    );
}
