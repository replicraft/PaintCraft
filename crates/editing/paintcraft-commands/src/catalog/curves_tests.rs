//! Curves is admitted only by the explicit, inventory-bound catalog grammar.
use super::{tests::packet, *};
use paintcraft_core::types::filters::{CurvesRgbV1, Descriptor};

fn curves(values: impl IntoIterator<Item = Value>) -> Value {
    Value::object([
        ("kind", Value::str("curves-rgb-v1")),
        ("outputs", Value::Array(values.into_iter().collect())),
    ])
}
fn values(outputs: [u8; 5]) -> Value {
    curves(outputs.map(|v| Value::Int(i64::from(v))))
}

#[test]
fn complete_curves_add_update_preserve_free_outputs_refs_and_generated_inventory() {
    let profiles = [
        [0, 64, 128, 192, 255],
        [255, 191, 127, 63, 0],
        [37; 5],
        [23, 9, 240, 73, 201],
        [255, 0, 255, 0, 255],
    ];
    for outputs in profiles {
        for layer in [
            LayerRef::Id(Uuid::from_u128(2)),
            LayerRef::Alias("layer".into()),
        ] {
            let descriptor = Descriptor::CurvesRgbV1(CurvesRgbV1::new(outputs));
            for op in [
                FilterOp::Add {
                    layer: layer.clone(),
                    alias: "curve".into(),
                    descriptor,
                    enabled: false,
                    to_index: Some(0),
                },
                FilterOp::Update {
                    layer: layer.clone(),
                    filter: FilterRef::Alias("prior".into()),
                    descriptor,
                },
            ] {
                let operation = op.to_value().unwrap();
                assert_eq!(op.required_batch_version(), 6);
                assert_eq!(
                    Batch::schema_for_operations(std::slice::from_ref(&operation)),
                    CATALOG_BATCH_SCHEMA
                );
                let mut generated = packet(CATALOG_BATCH_SCHEMA, None, values(outputs));
                let Value::Object(ref mut fields) = generated else {
                    unreachable!()
                };
                fields.insert("operations".into(), Value::Array(vec![operation]));
                assert_eq!(
                    Batch::from_value(&generated).unwrap_err().code,
                    ErrorCode::InputSchema
                );
                let parsed = Batch::from_generated_value(generated).unwrap();
                assert_eq!(parsed.version, 6);
                let Op::Filter(actual) = &parsed.operations[0] else {
                    panic!("filter operation")
                };
                assert_eq!(actual, &op);
            }
        }
        let public = packet(
            CATALOG_BATCH_SCHEMA,
            Some(&["curves-rgb-v1"]),
            values(outputs),
        );
        assert!(Batch::from_value(&public).is_ok());
        for version in 1..=5 {
            assert_eq!(
                Batch::from_value(&packet(
                    &format!("paintcraft.batch/v{version}"),
                    None,
                    values(outputs)
                ))
                .unwrap_err()
                .code,
                ErrorCode::InputSchema
            );
        }
    }
    for op in [
        "filter.set_enabled",
        "filter.duplicate",
        "filter.remove",
        "filter.reorder",
    ] {
        assert_eq!(
            Batch::schema_for_operation_fields([(op, Some("curves-rgb-v1"))]),
            "paintcraft.batch/v2"
        );
    }
}

#[test]
fn curves_rejects_invalid_array_members_shapes_unknowns_and_fractional_tokens() {
    let good = [0, 64, 128, 192, 255].map(Value::Int);
    for position in 0..5 {
        for bad_value in [
            Value::Int(-1),
            Value::Int(256),
            Value::Int(i64::MIN),
            Value::Int(i64::MAX),
            Value::Bool(true),
            Value::Null,
            Value::str("64"),
        ] {
            let mut fields = good.clone();
            fields[position] = bad_value;
            assert_eq!(
                Batch::from_value(&packet(
                    CATALOG_BATCH_SCHEMA,
                    Some(&["curves-rgb-v1"]),
                    curves(fields)
                ))
                .unwrap_err()
                .code,
                ErrorCode::InputSchema
            );
        }
    }
    for invalid in [
        curves([]),
        curves(vec![Value::Int(0); 4]),
        curves(vec![Value::Int(0); 6]),
        Value::object([("kind", Value::str("curves-rgb-v1"))]),
        Value::object([
            ("kind", Value::str("curves-rgb-v1")),
            ("outputs", Value::Null),
        ]),
        Value::object([
            ("kind", Value::str("curves-rgb-v1")),
            ("outputs", Value::str("identity")),
        ]),
        Value::object([
            ("kind", Value::str("curves-rgb-v1")),
            ("outputs", Value::Array(good.to_vec())),
            ("extra", Value::Int(0)),
        ]),
    ] {
        assert_eq!(
            Batch::from_value(&packet(
                CATALOG_BATCH_SCHEMA,
                Some(&["curves-rgb-v1"]),
                invalid
            ))
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
    }
    let packet = packet(CATALOG_BATCH_SCHEMA, Some(&["curves-rgb-v1"]), curves(good));
    let canonical = String::from_utf8(json::canonical(&packet)).unwrap();
    for token in ["0.0", "0e0", "true", "null", "\"0\""] {
        let lexical = canonical.replace("\"outputs\":[0,64", &format!("\"outputs\":[{token},64"));
        assert_ne!(lexical, canonical);
        assert_eq!(
            Batch::from_json(lexical.as_bytes()).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
}

#[test]
fn curves_inventory_deduplicates_all_payloads_and_rejects_missing_or_unused_requirements() {
    let fields = [
        ("filter.add", Some("exposure-srgb-v1")),
        ("filter.update", Some("curves-rgb-v1")),
        ("filter.add", Some("levels-rgb-v1")),
        ("filter.update", Some("curves-rgb-v1")),
        ("filter.duplicate", Some("invert-rgb-v1")),
    ];
    assert_eq!(
        Batch::catalog_requirements_for_fields(fields).unwrap(),
        ["curves-rgb-v1", "exposure-srgb-v1", "levels-rgb-v1"]
    );
    for required in [
        vec![],
        vec!["levels-rgb-v1"],
        vec!["curves-rgb-v1", "levels-rgb-v1"],
        vec!["curves-rgb-v1", "curves-rgb-v1"],
        vec!["future-curves-v1"],
    ] {
        assert_eq!(
            Batch::from_value(&packet(
                CATALOG_BATCH_SCHEMA,
                Some(&required),
                values([0, 64, 128, 192, 255])
            ))
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
    }
    let mut mixed = packet(
        CATALOG_BATCH_SCHEMA,
        Some(&["curves-rgb-v1", "invert-rgb-v1"]),
        values([0, 64, 128, 192, 255]),
    );
    let Value::Object(ref mut fields) = mixed else {
        unreachable!()
    };
    let Value::Array(mut operations) = fields["operations"].clone() else {
        unreachable!()
    };
    let mut second = operations[0].clone();
    let Value::Object(ref mut operation) = second else {
        unreachable!()
    };
    operation.insert("descriptor".into(), Descriptor::InvertRgbV1.to_value());
    operation.insert("ref".into(), Value::str("invert"));
    operations.push(second);
    fields.insert("operations".into(), Value::Array(operations));
    assert!(Batch::from_value(&mixed).is_ok());
    fields_if_object(&mut mixed).insert(
        "required_filters".into(),
        Value::Array(vec![
            Value::str("invert-rgb-v1"),
            Value::str("curves-rgb-v1"),
        ]),
    );
    assert_eq!(
        Batch::from_value(&mixed).unwrap_err().code,
        ErrorCode::InputSchema
    );
}
fn fields_if_object(value: &mut Value) -> &mut std::collections::BTreeMap<String, Value> {
    let Value::Object(fields) = value else {
        unreachable!()
    };
    fields
}
