use super::*;
fn value(h: i64, s: i64, l: i64) -> Value {
    Value::object([
        ("kind", Value::str("hue-saturation-rgb-v1")),
        ("hue_degrees", Value::Int(h)),
        ("saturation_hundredths", Value::Int(s)),
        ("lightness_hundredths", Value::Int(l)),
    ])
}
#[test]
fn hsl_complete_signed_intent_identity_and_distinct_equal_effect_hues_remain_catalog_only() {
    for [h, s, l] in [
        [0, 0, 0],
        [-180, -100, 100],
        [180, 100, -100],
        [-1, 1, -1],
        [137, 73, -19],
    ] {
        let json = value(h, s, l);
        let descriptor = Descriptor::from_catalog_value(&json).unwrap();
        assert_eq!(descriptor.to_value(), json);
        assert!(descriptor.is_catalog_only());
        assert!(!descriptor.is_spatial());
        let expected: Vec<u8> = [h, s, l]
            .into_iter()
            .flat_map(|n| (n as i16).to_be_bytes())
            .collect();
        let (bytes, len) = descriptor.semantic_parameter_bytes();
        assert_eq!(len, 6);
        assert_eq!(&bytes[..len], expected.as_slice());
        assert_eq!(
            Descriptor::from_value(&json).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
    assert_eq!(
        Descriptor::checked("hue-saturation-rgb-v1", None, None)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_ne!(
        Descriptor::from_catalog_value(&value(-180, 0, 0)).unwrap(),
        Descriptor::from_catalog_value(&value(180, 0, 0)).unwrap()
    );
}
#[test]
fn hsl_strict_complete_domains_types_and_fields_refuse_without_coercion() {
    for [h, s, l] in [
        [-181, 0, 0],
        [181, 0, 0],
        [0, -101, 0],
        [0, 101, 0],
        [0, 0, -101],
        [0, 0, 101],
        [i64::MIN, 0, 0],
        [0, i64::MAX, 0],
    ] {
        assert_eq!(
            Descriptor::from_catalog_value(&value(h, s, l))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    for field in [
        "hue_degrees",
        "saturation_hundredths",
        "lightness_hundredths",
    ] {
        for invalid in [
            Value::Null,
            Value::Bool(false),
            Value::str("0"),
            Value::Array(vec![]),
        ] {
            let Value::Object(mut fields) = value(0, 0, 0) else {
                unreachable!()
            };
            fields.insert(field.into(), invalid);
            assert_eq!(
                Descriptor::from_catalog_value(&Value::Object(fields))
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema
            );
        }
        let Value::Object(mut fields) = value(0, 0, 0) else {
            unreachable!()
        };
        fields.remove(field);
        assert_eq!(
            Descriptor::from_catalog_value(&Value::Object(fields))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    let Value::Object(mut fields) = value(0, 0, 0) else {
        unreachable!()
    };
    fields.insert("extra".into(), Value::Int(0));
    assert_eq!(
        Descriptor::from_catalog_value(&Value::Object(fields))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
}
