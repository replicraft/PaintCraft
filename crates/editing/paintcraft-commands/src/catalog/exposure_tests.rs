//! Exposure joins only the explicit catalog grammar; older packets stay closed.
use super::{tests::packet, *};
use paintcraft_core::types::filters::{Descriptor, ExposureSrgbV1};
fn exposure(stops: Value) -> Value {
    Value::object([
        ("kind", Value::str("exposure-srgb-v1")),
        ("stops_hundredths", stops),
    ])
}
fn descriptor(stops: i16) -> Descriptor {
    Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(stops).unwrap())
}
#[test]
fn exposure_catalog_add_update_roundtrip_and_generated_minimal_packet() {
    for stops in [-800, -1, 0, 1, 800] {
        for op in [
            FilterOp::Add {
                layer: LayerRef::Id(Uuid::from_u128(2)),
                alias: "ev".into(),
                descriptor: descriptor(stops),
                enabled: false,
                to_index: Some(0),
            },
            FilterOp::Update {
                layer: LayerRef::Id(Uuid::from_u128(2)),
                filter: FilterRef::Id(Uuid::from_u128(3)),
                descriptor: descriptor(stops),
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
                exposure(Value::Int(i64::from(stops))),
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
            // Untrusted input must provide the inventory, even for zero EV.
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
            Batch::schema_for_operation_fields([(name, Some("exposure-srgb-v1"))]),
            "paintcraft.batch/v2"
        );
    }
}
#[test]
fn exposure_exact_integer_domains_and_closed_legacy_grammars() {
    for stops in [-800, 0, 800] {
        assert!(
            Batch::from_value(&packet(
                CATALOG_BATCH_SCHEMA,
                Some(&["exposure-srgb-v1"]),
                exposure(Value::Int(stops))
            ))
            .is_ok()
        );
    }
    for invalid in [
        Value::Int(-801),
        Value::Int(801),
        Value::Int(i64::MIN),
        Value::Null,
        Value::Bool(true),
        Value::str("100"),
    ] {
        assert_eq!(
            Batch::from_value(&packet(
                CATALOG_BATCH_SCHEMA,
                Some(&["exposure-srgb-v1"]),
                exposure(invalid)
            ))
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
    }
    for invalid in [
        Value::object([("kind", Value::str("exposure-srgb-v1"))]),
        Value::object([
            ("kind", Value::str("exposure-srgb-v1")),
            ("stops_hundredths", Value::Int(0)),
            ("stops", Value::Int(0)),
        ]),
    ] {
        assert_eq!(
            Batch::from_value(&packet(
                CATALOG_BATCH_SCHEMA,
                Some(&["exposure-srgb-v1"]),
                invalid
            ))
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
    }
    for version in 1..=5 {
        for stops in [-800, 0, 800] {
            assert_eq!(
                Batch::from_value(&packet(
                    &format!("paintcraft.batch/v{version}"),
                    None,
                    exposure(Value::Int(stops))
                ))
                .unwrap_err()
                .code,
                ErrorCode::InputSchema
            );
        }
    }
    let value = packet(
        CATALOG_BATCH_SCHEMA,
        Some(&["exposure-srgb-v1"]),
        exposure(Value::Int(0)),
    );
    let lexical = String::from_utf8(json::canonical(&value))
        .unwrap()
        .replace("\"stops_hundredths\":0", "\"stops_hundredths\":0.0");
    assert_ne!(lexical.as_bytes(), json::canonical(&value));
    assert_eq!(
        Batch::from_json(lexical.as_bytes()).unwrap_err().code,
        ErrorCode::InputSchema
    );
}
#[test]
fn exposure_inventory_union_is_complete_sorted_and_strict() {
    let fields = [
        ("filter.add", Some("levels-rgb-v1")),
        ("filter.update", Some("exposure-srgb-v1")),
        ("filter.add", Some("invert-rgb-v1")),
        ("filter.add", Some("exposure-srgb-v1")),
    ];
    assert_eq!(
        Batch::catalog_requirements_for_fields(fields).unwrap(),
        ["exposure-srgb-v1", "invert-rgb-v1", "levels-rgb-v1"]
    );
    for declared in [
        vec![],
        vec!["levels-rgb-v1"],
        vec!["exposure-srgb-v1", "levels-rgb-v1"],
        vec!["exposure-srgb-v1", "exposure-srgb-v1"],
        vec!["future-exposure-v1"],
    ] {
        assert_eq!(
            Batch::from_value(&packet(
                CATALOG_BATCH_SCHEMA,
                Some(&declared),
                exposure(Value::Int(0))
            ))
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
    }
}
