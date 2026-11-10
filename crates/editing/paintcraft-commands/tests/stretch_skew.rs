//! Explicit additive profile; historical profiles must stay closed.
use paintcraft_commands::{Batch, Op, STRETCH_SKEW_BATCH_SCHEMA};
use paintcraft_core::types::{ErrorCode, json::Value};
fn packet(schema: &str, operation: Value) -> Value {
    Value::object([
        ("schema", Value::str(schema)),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str("paintcraft.core/default-v1")),
        (
            "base",
            Value::object([
                (
                    "state_id",
                    Value::str("00000000-0000-4000-8000-000000000001"),
                ),
                ("publication_epoch", Value::str("0")),
            ]),
        ),
        ("operations", Value::Array(vec![operation])),
    ])
}
fn op() -> Value {
    Value::object([
        ("op", Value::str("canvas.stretch_skew")),
        ("width", Value::Int(4)),
        ("height", Value::Int(2)),
        (
            "shear",
            Value::Array(vec![Value::Int(65536), Value::Int(-65536)]),
        ),
    ])
}
#[test]
fn new_profile_requires_actual_catalog_and_feature_envelope_and_old_profiles_refuse() {
    let operation = op();
    assert_eq!(
        Batch::schema_for_operations(std::slice::from_ref(&operation)),
        STRETCH_SKEW_BATCH_SCHEMA
    );
    assert_eq!(
        Batch::from_value(&packet(STRETCH_SKEW_BATCH_SCHEMA, operation.clone()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    let b =
        Batch::from_generated_value(packet(STRETCH_SKEW_BATCH_SCHEMA, operation.clone())).unwrap();
    assert_eq!(b.version, 9);
    assert!(matches!(
        b.operations[0],
        Op::CanvasStretchSkew {
            width: 4,
            height: 2,
            shear: (65536, -65536)
        }
    ));
    for old in [
        "paintcraft.batch/v1",
        "paintcraft.batch/v2",
        "paintcraft.batch/v3",
        "paintcraft.batch/v4",
        "paintcraft.batch/v5",
        "paintcraft.batch/catalog-v1",
        "paintcraft.batch/layer-appearance-v1",
        "paintcraft.batch/layer-appearance-v2",
    ] {
        assert_eq!(
            Batch::from_generated_value(packet(old, operation.clone()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
}
#[test]
fn unknown_fields_fractional_or_out_of_range_numbers_never_reach_engine() {
    for (key, value) in [
        ("extra", Value::Bool(true)),
        ("width", Value::Int(0)),
        ("height", Value::Int(1048577)),
        (
            "shear",
            Value::Array(vec![Value::Int(3754556), Value::Int(0)]),
        ),
        ("shear", Value::Array(vec![Value::Int(0)])),
    ] {
        let mut operation = op();
        let Value::Object(ref mut fields) = operation else {
            unreachable!()
        };
        fields.insert(key.into(), value);
        assert_eq!(
            Batch::from_generated_value(packet(STRETCH_SKEW_BATCH_SCHEMA, operation))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
}
