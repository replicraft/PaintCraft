//! Additive grammar owns exact closed coefficient semantics; legacy stays closed.
use paintcraft_commands::{Batch, Op, ROTATION_BATCH_SCHEMA};
use paintcraft_core::types::{ErrorCode, json::Value};
fn op(coefficients: (i64, i64)) -> Value {
    Value::object([
        ("op", Value::str("canvas.rotate")),
        (
            "coefficients",
            Value::Array(vec![Value::Int(coefficients.0), Value::Int(coefficients.1)]),
        ),
    ])
}
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
#[test]
fn rotation_only_new_profile_and_real_required_envelope() {
    let operation = op((644245094, 858993459));
    assert_eq!(
        Batch::schema_for_operations(std::slice::from_ref(&operation)),
        ROTATION_BATCH_SCHEMA
    );
    assert_eq!(
        Batch::from_value(&packet(ROTATION_BATCH_SCHEMA, operation.clone()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    let b = Batch::from_generated_value(packet(ROTATION_BATCH_SCHEMA, operation.clone())).unwrap();
    assert_eq!(b.version, 11);
    assert!(matches!(
        b.operations[0],
        Op::CanvasRotate {
            coefficients: (644245094, 858993459)
        }
    ));
    for schema in [
        "paintcraft.batch/v1",
        "paintcraft.batch/v2",
        "paintcraft.batch/v3",
        "paintcraft.batch/v4",
        "paintcraft.batch/v5",
        "paintcraft.batch/catalog-v1",
        "paintcraft.batch/layer-appearance-v1",
        "paintcraft.batch/layer-blend-v1",
        "paintcraft.batch/stretch-skew-v1",
        "paintcraft.batch/layer-styles-v1",
    ] {
        assert!(
            Batch::from_generated_value(packet(schema, operation.clone())).is_err(),
            "{schema}"
        );
    }
}
#[test]
fn malformed_norm_fields_and_coefficients_refuse_without_fallback() {
    for pair in [
        (0, 0),
        (1 << 30, 1 << 30),
        ((1 << 30) + 1, 0),
        (i64::MAX, 0),
    ] {
        assert_eq!(
            Batch::from_generated_value(packet(ROTATION_BATCH_SCHEMA, op(pair)))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    let mut operation = op((0, 1 << 30));
    let Value::Object(ref mut o) = operation else {
        unreachable!()
    };
    o.insert("degrees".into(), Value::Int(90));
    assert!(Batch::from_generated_value(packet(ROTATION_BATCH_SCHEMA, operation)).is_err());
}
