use super::*;
use crate::types::json::{self, Value};

#[test]
fn box_descriptor_is_strict_spatial_even_at_zero_and_roundtrips_all_radii() {
    for radius in 0..=32 {
        let descriptor = Descriptor::BoxBlurV1(BoxBlurV1::new(radius).unwrap());
        assert!(descriptor.is_spatial());
        assert_eq!(descriptor.id(), "box-blur-v1");
        let expected = Value::object([
            ("kind", Value::str("box-blur-v1")),
            ("radius", Value::Int(i64::from(radius))),
        ]);
        assert_eq!(descriptor.to_value(), expected);
        assert_eq!(Descriptor::from_value(&expected).unwrap(), descriptor);
    }
    for descriptor in [
        Descriptor::InvertRgbV1,
        Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(-100, 99).unwrap()),
        Descriptor::BlackWhiteRgbV1(BlackWhiteV1::new(100).unwrap()),
        Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(128).unwrap()),
        Descriptor::PosterizeRgbV1(PosterizeRgbV1::new(6).unwrap()),
        Descriptor::SolarizeRgbV1,
    ] {
        assert!(!descriptor.is_spatial());
        assert_eq!(
            Descriptor::from_value(&descriptor.to_value()).unwrap(),
            descriptor
        );
    }
}

#[test]
fn box_wire_defaults_never_replace_required_values_or_accept_wrong_types() {
    for text in [
        r#"{"kind":"box-blur-v1"}"#,
        r#"{"kind":"box-blur-v1","radius":-1}"#,
        r#"{"kind":"box-blur-v1","radius":33}"#,
        r#"{"kind":"box-blur-v1","radius":9223372036854775807}"#,
        r#"{"kind":"box-blur-v1","radius":true}"#,
        r#"{"kind":"box-blur-v1","radius":null}"#,
        r#"{"kind":"box-blur-v1","radius":"2"}"#,
        r#"{"kind":"box-blur-v1","radius":2,"strength":100}"#,
        r#"{"kind":"box-blur-v2","radius":2}"#,
    ] {
        assert_eq!(
            Descriptor::from_value(&json::parse(text.as_bytes()).unwrap())
                .unwrap_err()
                .code,
            ErrorCode::InputSchema,
            "{text}"
        );
    }
    // Integer-only wire parsing refuses decimal notation before creating Value.
    assert!(json::parse(br#"{"kind":"box-blur-v1","radius":2.0}"#).is_err());
    assert_eq!(BoxBlurV1::default().radius(), 2);
    // The legacy unrelated two-parameter constructor does not infer Box radius.
    assert_eq!(
        Descriptor::checked("box-blur-v1", None, None)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}
