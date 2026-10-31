//! Proposed Core catalog controls; not registered in the product.
use super::*;
#[test]
fn vibrance_strict_signed_intent_default_and_legacy_closure() {
    for n in [-100, -1, 0, 1, 100] {
        let value = Value::object([
            ("kind", Value::str("vibrance-rgb-v1")),
            ("amount_hundredths", Value::Int(n)),
        ]);
        let descriptor = Descriptor::from_catalog_value(&value).unwrap();
        assert_eq!(descriptor.to_value(), value);
        assert!(descriptor.is_catalog_only());
        assert!(!descriptor.is_spatial());
        let (bytes, length) = descriptor.semantic_parameter_bytes();
        assert_eq!(length, 2);
        assert_eq!(&bytes[..2], &(n as i16).to_be_bytes());
        assert_eq!(
            Descriptor::from_value(&value).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
    for n in [-101, 101, i64::MIN, i64::MAX] {
        let value = Value::object([
            ("kind", Value::str("vibrance-rgb-v1")),
            ("amount_hundredths", Value::Int(n)),
        ]);
        assert_eq!(
            Descriptor::from_catalog_value(&value).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
    for value in [
        Value::Null,
        Value::Bool(false),
        Value::str("0"),
        Value::Array(vec![]),
    ] {
        assert_eq!(
            Descriptor::from_catalog_value(&Value::object([
                ("kind", Value::str("vibrance-rgb-v1")),
                ("amount_hundredths", value)
            ]))
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
    }
    for value in [
        Value::object([("kind", Value::str("vibrance-rgb-v1"))]),
        Value::object([
            ("kind", Value::str("vibrance-rgb-v1")),
            ("amount_hundredths", Value::Int(0)),
            ("extra", Value::Int(0)),
        ]),
    ] {
        assert_eq!(
            Descriptor::from_catalog_value(&value).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
}
