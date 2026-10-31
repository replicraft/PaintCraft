use super::*;
fn value(a: i64, seed: i64, mono: bool, distribution: &str) -> Value {
    Value::object([
        ("kind", Value::str("add-noise-rgb-v1")),
        ("amount_hundredths", Value::Int(a)),
        ("seed", Value::Int(seed)),
        ("monochrome", Value::Bool(mono)),
        ("distribution", Value::str(distribution)),
    ])
}
#[test]
fn noise_complete_six_byte_intent_roundtrips_with_both_modes_and_u16_seed() {
    for a in [0, 10, 100] {
        for seed in [0, 1, 65535] {
            for mono in [false, true] {
                for (name, tag) in [("uniform", 0), ("gaussian", 1)] {
                    let json = value(a, seed, mono, name);
                    let d = Descriptor::from_catalog_value(&json).unwrap();
                    assert_eq!(d.to_value(), json);
                    assert_eq!(d.id(), "add-noise-rgb-v1");
                    assert!(d.is_catalog_only());
                    assert!(!d.is_spatial());
                    let (bytes, len) = d.semantic_parameter_bytes();
                    assert_eq!(len, 6);
                    let s = (seed as u16).to_be_bytes();
                    assert_eq!(bytes, [0, a as u8, s[0], s[1], mono as u8, tag]);
                    assert_eq!(
                        Descriptor::from_value(&json).unwrap_err().code,
                        ErrorCode::InputSchema
                    );
                }
            }
        }
    }
    assert_eq!(
        Descriptor::checked("add-noise-rgb-v1", None, None)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_ne!(
        Descriptor::from_catalog_value(&value(0, 0, true, "uniform")).unwrap(),
        Descriptor::from_catalog_value(&value(0, 65535, false, "gaussian")).unwrap()
    );
}
#[test]
fn noise_types_required_fields_flags_and_domains_are_closed_without_coercion() {
    for (a, seed) in [
        (-1, 0),
        (101, 0),
        (0, -1),
        (0, 65536),
        (i64::MAX, 0),
        (0, i64::MAX),
    ] {
        assert_eq!(
            Descriptor::from_catalog_value(&value(a, seed, false, "gaussian"))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    for (field, bad) in [
        ("amount_hundredths", Value::Bool(false)),
        ("seed", Value::Bool(true)),
        ("seed", Value::str("65535")),
        ("monochrome", Value::Int(1)),
        ("distribution", Value::Int(0)),
        ("distribution", Value::str("Gaussian")),
        ("distribution", Value::str("normal")),
        ("distribution", Value::Null),
    ] {
        let Value::Object(mut fields) = value(10, 65535, true, "uniform") else {
            unreachable!()
        };
        fields.insert(field.into(), bad);
        assert_eq!(
            Descriptor::from_catalog_value(&Value::Object(fields))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    for field in ["amount_hundredths", "seed", "monochrome", "distribution"] {
        let Value::Object(mut fields) = value(10, 1, true, "uniform") else {
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
    let Value::Object(mut fields) = value(10, 1, true, "uniform") else {
        unreachable!()
    };
    fields.insert("extra".into(), Value::Null);
    assert_eq!(
        Descriptor::from_catalog_value(&Value::Object(fields))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert!(crate::types::json::parse(br#"{"kind":"add-noise-rgb-v1","amount_hundredths":10,"seed":1.0,"monochrome":true,"distribution":"uniform"}"#).is_err());
}
