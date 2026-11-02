use super::*;
fn operation(mode: Value) -> Value {
    Value::object([
        ("op", Value::str("layer.set_blend_mode")),
        (
            "layer_ref",
            Value::object([("id", Value::str(Uuid::from_u128(19).to_string()))]),
        ),
        ("blend_mode", mode),
    ])
}
fn packet(schema: &str, op: Value) -> Value {
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
        ("operations", Value::Array(vec![op])),
    ])
}
#[test]
fn three_modes_have_one_explicit_new_grammar_and_old_grammars_remain_closed() {
    for (name, mode) in [
        ("normal", LayerBlendMode::Normal),
        ("multiply", LayerBlendMode::Multiply),
        ("screen", LayerBlendMode::Screen),
    ] {
        let op = operation(Value::str(name));
        assert_eq!(
            Batch::schema_for_operations(std::slice::from_ref(&op)),
            LAYER_BLEND_BATCH_SCHEMA
        );
        let batch =
            Batch::from_generated_value(packet(LAYER_BLEND_BATCH_SCHEMA, op.clone())).unwrap();
        assert_eq!(batch.version, 8);
        assert!(
            matches!(&batch.operations[0],Op::LayerSetBlendMode{mode:actual,..} if *actual==mode)
        );
        for schema in [
            "paintcraft.batch/v1",
            "paintcraft.batch/v2",
            "paintcraft.batch/v3",
            "paintcraft.batch/v4",
            "paintcraft.batch/v5",
            CATALOG_BATCH_SCHEMA,
            LAYER_APPEARANCE_BATCH_SCHEMA,
        ] {
            assert_eq!(
                Batch::from_generated_value(packet(schema, op.clone()))
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema
            );
        }
    }
}
#[test]
fn domains_exact_fields_and_declared_inventory_are_strict() {
    for mode in [
        Value::str("Multiply"),
        Value::str("overlay"),
        Value::Int(1),
        Value::Null,
    ] {
        assert!(
            Batch::from_generated_value(packet(LAYER_BLEND_BATCH_SCHEMA, operation(mode))).is_err()
        );
    }
    let mut op = operation(Value::str("multiply"));
    let Value::Object(ref mut fields) = op else {
        unreachable!()
    };
    fields.insert("opacity".into(), Value::Int(255));
    assert!(Batch::from_generated_value(packet(LAYER_BLEND_BATCH_SCHEMA, op)).is_err());
    let mut value = packet(LAYER_BLEND_BATCH_SCHEMA, operation(Value::str("normal")));
    let Value::Object(ref mut fields) = value else {
        unreachable!()
    };
    fields.insert("required_filters".into(), Value::Array(vec![]));
    fields.insert(
        "required_layer_features".into(),
        Value::Array(vec![Value::str("layer-blend-modes.v1")]),
    );
    assert_eq!(Batch::from_value(&value).unwrap().version, 8);
    for requirements in [
        vec![],
        vec![Value::str("layer-masks.v1")],
        vec![Value::str("layer-blend-modes.v1"); 2],
    ] {
        let mut bad = value.clone();
        let Value::Object(ref mut fields) = bad else {
            unreachable!()
        };
        fields.insert("required_layer_features".into(), Value::Array(requirements));
        assert!(Batch::from_value(&bad).is_err());
    }
}

#[test]
fn mixed_layer_features_are_sorted_and_old_property_fields_stay_closed() {
    let mask = LayerMaskOp::Remove {
        layer: LayerRef::Id(Uuid::from_u128(19)),
    }
    .to_value()
    .unwrap();
    let blend = operation(Value::str("screen"));
    let mut value = packet(LAYER_BLEND_BATCH_SCHEMA, blend);
    let Value::Object(ref mut fields) = value else {
        unreachable!()
    };
    fields.insert(
        "operations".into(),
        Value::Array(vec![operation(Value::str("screen")), mask]),
    );
    fields.insert("required_filters".into(), Value::Array(vec![]));
    fields.insert(
        "required_layer_features".into(),
        Value::Array(vec![
            Value::str("layer-blend-modes.v1"),
            Value::str("layer-masks.v1"),
        ]),
    );
    assert_eq!(Batch::from_value(&value).unwrap().operations.len(), 2);
    let Value::Object(ref mut fields) = value else {
        unreachable!()
    };
    fields.insert(
        "required_layer_features".into(),
        Value::Array(vec![
            Value::str("layer-masks.v1"),
            Value::str("layer-blend-modes.v1"),
        ]),
    );
    assert!(Batch::from_value(&value).is_err());
    let mut op = operation(Value::str("multiply"));
    let Value::Object(ref mut fields) = op else {
        unreachable!()
    };
    fields.insert("op".into(), Value::str("layer.set_properties"));
    assert!(Batch::from_generated_value(packet(LAYER_BLEND_BATCH_SCHEMA, op)).is_err());
}
