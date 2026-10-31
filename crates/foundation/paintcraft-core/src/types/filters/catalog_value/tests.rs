use super::*;

fn catalog(p: [i16; 5]) -> Descriptor {
    Descriptor::LevelsRgbV1(LevelsRgbV1::new(p[0], p[1], p[2], p[3], p[4]).unwrap())
}

#[test]
fn explicit_catalog_roundtrips_full_intent_and_preserves_the_closed_legacy_parser() {
    for p in [
        [0, 255, 100, 0, 255],
        [0, 255, 10, 255, 0],
        [17, 201, 999, 240, 16],
        [64, 192, 200, 73, 73],
    ] {
        let descriptor = catalog(p);
        assert!(descriptor.is_catalog_only());
        assert!(!descriptor.is_spatial());
        assert_eq!(descriptor.id(), "levels-rgb-v1");
        assert_eq!(
            Descriptor::from_catalog_value(&descriptor.to_value()).unwrap(),
            descriptor
        );
        assert_eq!(
            Descriptor::from_value(&descriptor.to_value())
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        assert_eq!(
            Descriptor::checked(descriptor.id(), None, None)
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    for descriptor in [
        Descriptor::InvertRgbV1,
        Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(-19, 23).unwrap()),
        Descriptor::BlackWhiteRgbV1(BlackWhiteV1::default()),
        Descriptor::ThresholdLumaV1(ThresholdLumaV1::default()),
        Descriptor::PosterizeRgbV1(PosterizeRgbV1::default()),
        Descriptor::SolarizeRgbV1,
        Descriptor::BoxBlurV1(BoxBlurV1::new(32).unwrap()),
    ] {
        assert!(!descriptor.is_catalog_only());
        assert_eq!(
            Descriptor::from_value(&descriptor.to_value()).unwrap(),
            descriptor
        );
        assert_eq!(
            Descriptor::from_catalog_value(&descriptor.to_value()).unwrap(),
            descriptor
        );
    }
}

#[test]
fn catalog_keys_types_and_domains_are_closed_without_defaults_or_truncation() {
    let source = catalog([0, 255, 100, 0, 255]).to_value();
    let object = source.as_object().unwrap();
    for key in [
        "input_black",
        "input_white",
        "gamma_hundredths",
        "output_black",
        "output_white",
    ] {
        let mut missing = object.clone();
        missing.remove(key);
        assert_eq!(
            Descriptor::from_catalog_value(&Value::Object(missing))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        for invalid in [
            Value::Null,
            Value::Bool(true),
            Value::str("100"),
            Value::Int(i64::MAX),
            Value::Int(i64::MIN),
        ] {
            let mut invalid_object = object.clone();
            invalid_object.insert(key.into(), invalid);
            assert_eq!(
                Descriptor::from_catalog_value(&Value::Object(invalid_object))
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema
            );
        }
    }
    for (key, value) in [
        ("input_black", 255),
        ("input_white", 0),
        ("gamma_hundredths", 9),
        ("gamma_hundredths", 1000),
        ("output_black", 256),
        ("output_white", -1),
    ] {
        let mut bad = object.clone();
        bad.insert(key.into(), Value::Int(value));
        assert_eq!(
            Descriptor::from_catalog_value(&Value::Object(bad))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    let mut bad = object.clone();
    bad.insert("gamma".into(), Value::Int(1));
    assert_eq!(
        Descriptor::from_catalog_value(&Value::Object(bad))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    bad = object.clone();
    bad.insert("kind".into(), Value::str("levels-rgb-v2"));
    assert_eq!(
        Descriptor::from_catalog_value(&Value::Object(bad))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert!(
        crate::types::json::parse(br#"{"kind":"levels-rgb-v1","gamma_hundredths":100.0}"#).is_err()
    );
}

#[test]
fn semantic_bytes_pin_six_byte_gamma_order_and_exclude_unused_padding() {
    let descriptor = catalog([17, 201, 999, 240, 16]);
    assert_eq!(
        descriptor.semantic_parameter_bytes(),
        ([17, 201, 3, 231, 240, 16], 6)
    );
    let bc = Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(-19, 23).unwrap());
    assert_eq!(bc.semantic_parameter_bytes(), ([255, 237, 0, 23, 0, 0], 4));
    assert_eq!(
        Descriptor::InvertRgbV1.semantic_parameter_bytes(),
        ([0; 6], 0)
    );
    assert_eq!(
        Descriptor::BoxBlurV1(BoxBlurV1::new(32).unwrap()).semantic_parameter_bytes(),
        ([32, 0, 0, 0, 0, 0], 1)
    );
}
