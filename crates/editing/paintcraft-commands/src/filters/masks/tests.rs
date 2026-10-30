use super::*;
fn mask() -> Value {
    Value::object([
        (
            "domain",
            Value::object([
                ("x", Value::Int(-3)),
                ("y", Value::Int(-2)),
                ("width", Value::Int(8)),
                ("height", Value::Int(6)),
            ]),
        ),
        (
            "bands",
            Value::Array(vec![Value::object([
                ("y0", Value::Int(-2)),
                ("y1", Value::Int(1)),
                (
                    "runs",
                    Value::Array(vec![
                        Value::object([
                            ("x0", Value::Int(-2)),
                            ("x1", Value::Int(0)),
                            ("coverage", Value::Int(0)),
                        ]),
                        Value::object([
                            ("x0", Value::Int(1)),
                            ("x1", Value::Int(3)),
                            ("coverage", Value::Int(128)),
                        ]),
                    ]),
                ),
            ])]),
        ),
    ])
}
fn operation(name: &str, extra: Vec<(&str, Value)>) -> Value {
    let Value::Object(mut o) = Value::object([
        ("op", Value::str(name)),
        ("layer_ref", Value::object([("ref", Value::str("ink"))])),
        (
            "filter_ref",
            Value::object([("id", Value::str("00000000-0000-4000-8000-000000000002"))]),
        ),
    ]) else {
        unreachable!()
    };
    for (k, v) in extra {
        o.insert(k.into(), v);
    }
    Value::Object(o)
}
fn batch(version: u8, operation: Value) -> Value {
    Value::object([
        ("schema", Value::str(format!("paintcraft.batch/v{version}"))),
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
fn four_new_shapes_require_v3_and_roundtrip_checked_numeric_rows() {
    let budget = SharedBudget::new(8192);
    for op in [
        operation(
            "filter.mask.set",
            vec![("mask", mask()), ("enabled", Value::Bool(false))],
        ),
        operation(
            "filter.mask.from_selection",
            vec![("enabled", Value::Bool(false))],
        ),
        operation(
            "filter.mask.set_enabled",
            vec![("enabled", Value::Bool(true))],
        ),
        operation("filter.mask.remove", vec![]),
    ] {
        for version in [1, 2] {
            assert_eq!(
                Batch::from_value_with_coverage(&batch(version, op.clone()), &budget, &|| Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema
            );
            assert_eq!(budget.usage().reserved, 0);
        }
        let value = batch(3, op);
        let parsed = Batch::from_value_with_coverage(&value, &budget, &|| Ok(())).unwrap();
        let Op::Filter(filter) = &parsed.operations[0] else {
            panic!()
        };
        assert_eq!(filter.required_batch_version(), 3);
        let roundtrip = filter.to_value().unwrap();
        let other =
            Batch::from_value_with_coverage(&batch(3, roundtrip), &budget, &|| Ok(())).unwrap();
        assert_eq!(parsed.operations, other.operations);
    }
    assert_eq!(budget.usage().reserved, 0);
    let parsed = Batch::from_value_with_coverage(
        &batch(3, operation("filter.mask.set", vec![("mask", mask())])),
        &budget,
        &|| Ok(()),
    )
    .unwrap();
    let Op::Filter(FilterOp::Mask {
        operation: MaskOp::Set { coverage, enabled },
        ..
    }) = &parsed.operations[0]
    else {
        panic!()
    };
    assert!(*enabled);
    assert_eq!(coverage.domain().x0, -3);
    assert_eq!(coverage.bands().next().unwrap().runs[1].coverage, 128);
    let retained = coverage.clone();
    let charge = budget.usage().reserved;
    drop(parsed);
    assert_eq!(budget.usage().reserved, charge);
    drop(retained);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn strict_fields_numeric_types_and_order_never_widen_mask_grammar() {
    let valid = operation("filter.mask.set", vec![("mask", mask())]);
    let raw = String::from_utf8(json::canonical(&batch(3, valid))).unwrap();
    for (old, new) in [
        ("\"coverage\":128", "\"coverage\":256"),
        ("\"coverage\":128", "\"coverage\":128.5"),
        ("\"coverage\":128", "\"coverage\":\"128\""),
        ("\"x0\":1", "\"x0\":-1"),
        ("\"y1\":1", "\"y1\":-2"),
        ("\"height\":6", "\"height\":0"),
        ("\"mask\":", "\"extra\":true,\"mask\":"),
    ] {
        assert!(raw.contains(old));
        assert_eq!(
            Batch::from_json(raw.replace(old, new).as_bytes())
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    assert_eq!(
        Batch::from_value(&batch(
            3,
            operation("filter.mask.from_selection", vec![("mask", mask())])
        ))
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        Batch::from_value(&batch(
            3,
            operation("filter.mask.set_enabled", vec![("enabled", Value::Int(1))])
        ))
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
}
#[test]
fn numeric_input_admission_cancellation_and_failure_drop_owned_arrays() {
    let op = batch(3, operation("filter.mask.set", vec![("mask", mask())]));
    let denied = SharedBudget::new(1);
    let error = Batch::from_value_with_coverage(&op, &denied, &|| Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(
        error.allocation_denial().unwrap().class(),
        paintcraft_core::types::resources::AllocationClass::CoverageMasks
    );
    assert_eq!(denied.usage().reserved, 0);
    let budget = SharedBudget::new(8192);
    let calls = std::cell::Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() == 5 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "stop numeric input",
            ))
        } else {
            Ok(())
        }
    };
    assert_eq!(
        Batch::from_value_with_coverage(&op, &budget, &check)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(budget.usage().reserved, 0);
}
