use super::*;
fn layer() -> LayerRef {
    LayerRef::Id(Uuid::from_u128(17))
}
#[test]
fn four_typed_operations_roundtrip_and_preserve_explicit_enabled() {
    for operation in [
        LayerMaskOp::FromSelection {
            layer: layer(),
            enabled: false,
        },
        LayerMaskOp::SetEnabled {
            layer: layer(),
            enabled: true,
        },
        LayerMaskOp::Invert { layer: layer() },
        LayerMaskOp::Remove { layer: layer() },
    ] {
        assert_eq!(operation.required_batch_version(), 7);
        assert_eq!(operation.stable_uuid_ref(), Some(Uuid::from_u128(17)));
        let value = operation.to_value().unwrap();
        assert_eq!(parse(&value).unwrap(), Op::LayerMask(operation));
    }
    let alias = LayerMaskOp::Remove {
        layer: LayerRef::Alias("layer".into()),
    };
    assert_eq!(alias.stable_uuid_ref(), None);
    assert_eq!(
        parse(&alias.to_value().unwrap()).unwrap(),
        Op::LayerMask(alias)
    );
    assert!(
        LayerMaskOp::Remove {
            layer: LayerRef::Alias("".into())
        }
        .to_value()
        .is_err()
    );
}
#[test]
fn exact_fields_bool_and_layer_reference_are_required_without_coercion() {
    for name in ["from_selection", "set_enabled"] {
        for enabled in [
            None,
            Some(Value::Int(1)),
            Some(Value::str("true")),
            Some(Value::Null),
        ] {
            let mut v = LayerMaskOp::FromSelection {
                layer: layer(),
                enabled: true,
            }
            .to_value()
            .unwrap();
            let Value::Object(ref mut fields) = v else {
                unreachable!()
            };
            fields.insert("op".into(), Value::str(format!("layer.mask.{name}")));
            if let Some(enabled) = enabled {
                fields.insert("enabled".into(), enabled);
            } else {
                fields.remove("enabled");
            }
            assert_eq!(parse(&v).unwrap_err().code, ErrorCode::InputSchema);
        }
    }
    for name in ["invert", "remove", "set", "paint", "apply", "link"] {
        let v = Value::object([
            ("op", Value::str(format!("layer.mask.{name}"))),
            (
                "layer_ref",
                Value::object([("id", Value::str(Uuid::from_u128(17).to_string()))]),
            ),
            ("enabled", Value::Bool(true)),
        ]);
        assert_eq!(parse(&v).unwrap_err().code, ErrorCode::InputSchema);
    }
}
