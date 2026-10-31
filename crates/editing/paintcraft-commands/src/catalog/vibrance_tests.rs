//! Vibrance joins only the explicit catalog grammar; older packets stay closed.
use super::{tests::packet, *};
use paintcraft_core::types::filters::{Descriptor, VibranceRgbV1};
fn vibrance(amount: Value) -> Value {
    Value::object([
        ("kind", Value::str("vibrance-rgb-v1")),
        ("amount_hundredths", amount),
    ])
}
fn descriptor(amount: i16) -> Descriptor {
    Descriptor::VibranceRgbV1(VibranceRgbV1::new(amount).unwrap())
}
#[test]
fn vibrance_catalog_add_update_roundtrip_and_generated_minimal_packet() {
    for amount in [-100, -1, 0, 1, 100] {
        for op in [
            FilterOp::Add {
                layer: LayerRef::Id(Uuid::from_u128(2)),
                alias: "vib".into(),
                descriptor: descriptor(amount),
                enabled: false,
                to_index: Some(0),
            },
            FilterOp::Update {
                layer: LayerRef::Id(Uuid::from_u128(2)),
                filter: FilterRef::Id(Uuid::from_u128(3)),
                descriptor: descriptor(amount),
            },
        ] {
            let operation = op.to_value().unwrap();
            assert_eq!(op.required_batch_version(), 6);
            assert_eq!(
                Batch::schema_for_operations(std::slice::from_ref(&operation)),
                CATALOG_BATCH_SCHEMA
            );
            let mut value = packet(
                CATALOG_BATCH_SCHEMA,
                None,
                vibrance(Value::Int(i64::from(amount))),
            );
            let Value::Object(ref mut fields) = value else {
                unreachable!()
            };
            fields.insert("operations".into(), Value::Array(vec![operation]));
            let parsed = Batch::from_generated_value(value.clone()).unwrap();
            assert_eq!(parsed.version, 6);
            let Op::Filter(actual) = &parsed.operations[0] else {
                panic!("filter operation")
            };
            assert_eq!(actual, &op);
            // Untrusted input must provide the inventory, even for zero amount.
            assert_eq!(
                Batch::from_value(&value).unwrap_err().code,
                ErrorCode::InputSchema
            );
        }
    }
    for name in [
        "filter.set_enabled",
        "filter.duplicate",
        "filter.remove",
        "filter.reorder",
    ] {
        assert_eq!(
            Batch::schema_for_operation_fields([(name, Some("vibrance-rgb-v1"))]),
            "paintcraft.batch/v2"
        );
    }
}
#[test]
fn vibrance_exact_integer_domains_and_closed_legacy_grammars() {
    for amount in [-100, 0, 100] {
        assert!(
            Batch::from_value(&packet(
                CATALOG_BATCH_SCHEMA,
                Some(&["vibrance-rgb-v1"]),
                vibrance(Value::Int(amount))
            ))
            .is_ok()
        );
    }
    for invalid in [
        Value::Int(-101),
        Value::Int(101),
        Value::Int(i64::MIN),
        Value::Null,
        Value::Bool(true),
        Value::str("100"),
    ] {
        assert_eq!(
            Batch::from_value(&packet(
                CATALOG_BATCH_SCHEMA,
                Some(&["vibrance-rgb-v1"]),
                vibrance(invalid)
            ))
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
    }
    for invalid in [
        Value::object([("kind", Value::str("vibrance-rgb-v1"))]),
        Value::object([
            ("kind", Value::str("vibrance-rgb-v1")),
            ("amount_hundredths", Value::Int(0)),
            ("amount", Value::Int(0)),
        ]),
    ] {
        assert_eq!(
            Batch::from_value(&packet(
                CATALOG_BATCH_SCHEMA,
                Some(&["vibrance-rgb-v1"]),
                invalid
            ))
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
    }
    for version in 1..=5 {
        for amount in [-100, 0, 100] {
            assert_eq!(
                Batch::from_value(&packet(
                    &format!("paintcraft.batch/v{version}"),
                    None,
                    vibrance(Value::Int(amount))
                ))
                .unwrap_err()
                .code,
                ErrorCode::InputSchema
            );
        }
    }
    let value = packet(
        CATALOG_BATCH_SCHEMA,
        Some(&["vibrance-rgb-v1"]),
        vibrance(Value::Int(0)),
    );
    let lexical = String::from_utf8(json::canonical(&value))
        .unwrap()
        .replace("\"amount_hundredths\":0", "\"amount_hundredths\":0.0");
    assert_ne!(lexical.as_bytes(), json::canonical(&value));
    assert_eq!(
        Batch::from_json(lexical.as_bytes()).unwrap_err().code,
        ErrorCode::InputSchema
    );
}
#[test]
fn vibrance_inventory_union_is_complete_sorted_and_strict() {
    let fields = [
        ("filter.add", Some("levels-rgb-v1")),
        ("filter.update", Some("vibrance-rgb-v1")),
        ("filter.add", Some("invert-rgb-v1")),
        ("filter.add", Some("vibrance-rgb-v1")),
    ];
    assert_eq!(
        Batch::catalog_requirements_for_fields(fields).unwrap(),
        ["invert-rgb-v1", "levels-rgb-v1", "vibrance-rgb-v1"]
    );
    for declared in [
        vec![],
        vec!["levels-rgb-v1"],
        vec!["vibrance-rgb-v1", "levels-rgb-v1"],
        vec!["vibrance-rgb-v1", "vibrance-rgb-v1"],
        vec!["future-vibrance-v1"],
    ] {
        assert_eq!(
            Batch::from_value(&packet(
                CATALOG_BATCH_SCHEMA,
                Some(&declared),
                vibrance(Value::Int(0))
            ))
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
    }
}
