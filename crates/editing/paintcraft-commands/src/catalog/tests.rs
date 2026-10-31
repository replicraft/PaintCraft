use super::*;

fn levels(gamma: i64) -> Value {
    Value::object([
        ("kind", Value::str("levels-rgb-v1")),
        ("input_black", Value::Int(17)),
        ("input_white", Value::Int(201)),
        ("gamma_hundredths", Value::Int(gamma)),
        ("output_black", Value::Int(240)),
        ("output_white", Value::Int(16)),
    ])
}
fn packet(schema: &str, requirements: Option<&[&str]>, descriptor: Value) -> Value {
    let mut fields = vec![
        ("schema", Value::str(schema)),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str("paintcraft.core/default-v1")),
        (
            "base",
            Value::object([
                (
                    "state_id",
                    Value::str("11111111-1111-4111-8111-111111111111"),
                ),
                ("publication_epoch", Value::str("0")),
            ]),
        ),
        (
            "operations",
            Value::Array(vec![Value::object([
                ("op", Value::str("filter.add")),
                (
                    "layer_ref",
                    Value::object([("id", Value::str("22222222-2222-4222-8222-222222222222"))]),
                ),
                ("ref", Value::str("levels")),
                ("descriptor", descriptor),
            ])]),
        ),
    ];
    if let Some(requirements) = requirements {
        fields.push((
            "required_filters",
            Value::Array(requirements.iter().map(|id| Value::str(*id)).collect()),
        ));
    }
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

#[test]
fn catalog_requires_exact_inventory_and_retains_all_levels_parameters() {
    let value = packet(CATALOG_BATCH_SCHEMA, Some(&["levels-rgb-v1"]), levels(999));
    let batch = Batch::from_value(&value).unwrap();
    assert_eq!(batch.version, 6);
    let Op::Filter(FilterOp::Add { descriptor, .. }) = &batch.operations[0] else {
        panic!("filter add");
    };
    assert_eq!(descriptor.to_value(), levels(999));
    for requirements in [
        vec![],
        vec!["invert-rgb-v1"],
        vec!["levels-rgb-v1", "levels-rgb-v1"],
        vec!["invert-rgb-v1", "levels-rgb-v1"],
        vec!["future-v1"],
    ] {
        assert_eq!(
            Batch::from_value(&packet(
                CATALOG_BATCH_SCHEMA,
                Some(&requirements),
                levels(999)
            ))
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
    }
    assert!(Batch::from_value(&packet(CATALOG_BATCH_SCHEMA, None, levels(999))).is_err());
    assert!(
        Batch::from_value(&packet(
            CATALOG_BATCH_SCHEMA,
            Some(&["levels-rgb-v1"]),
            levels(1000)
        ))
        .is_err()
    );
}

#[test]
fn generated_adapter_fills_requirements_but_public_legacy_grammars_stay_closed() {
    let generated = packet(CATALOG_BATCH_SCHEMA, None, levels(333));
    assert_eq!(Batch::from_generated_value(generated).unwrap().version, 6);
    assert!(
        Batch::from_generated_value(packet(
            CATALOG_BATCH_SCHEMA,
            Some(&["levels-rgb-v1"]),
            levels(333)
        ))
        .is_err()
    );
    for schema in [
        "paintcraft.batch/v1",
        "paintcraft.batch/v2",
        "paintcraft.batch/v3",
        "paintcraft.batch/v4",
        "paintcraft.batch/v5",
    ] {
        assert!(Batch::from_value(&packet(schema, None, levels(333))).is_err());
        let invert = Value::object([("kind", Value::str("invert-rgb-v1"))]);
        assert!(Batch::from_value(&packet(schema, Some(&[]), invert)).is_err());
    }
}

#[test]
fn requirement_projection_is_sorted_unique_complete_and_does_not_infer_scalar_targets() {
    let fields = [
        ("filter.add", Some("levels-rgb-v1")),
        ("filter.update", Some("box-blur-v1")),
        ("filter.add", Some("levels-rgb-v1")),
        ("filter.duplicate", Some("invert-rgb-v1")),
    ];
    assert_eq!(
        Batch::catalog_requirements_for_fields(fields).unwrap(),
        ["box-blur-v1", "levels-rgb-v1"]
    );
    assert_eq!(
        Batch::schema_for_operation_fields(fields),
        CATALOG_BATCH_SCHEMA
    );
    assert_eq!(
        Batch::schema_for_operation_fields([("filter.duplicate", Some("levels-rgb-v1"))]),
        "paintcraft.batch/v2"
    );
    assert!(Batch::catalog_requirements_for_fields([("filter.add", Some("unknown-v1"))]).is_err());
    assert!(Batch::catalog_requirements_for_fields([("filter.add", None)]).is_err());
}
