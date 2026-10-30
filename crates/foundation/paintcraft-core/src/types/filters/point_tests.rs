use super::*;
use crate::types::json::{self, Value};
#[test]
fn point_domains_defaults_and_closed_descriptors_are_exact() {
    assert_eq!(BlackWhiteV1::default().strength(), 100);
    assert_eq!(ThresholdLumaV1::default().level(), 128);
    assert_eq!(PosterizeRgbV1::default().levels(), 6);
    for value in [i16::MIN, -1, 101, i16::MAX] {
        assert!(BlackWhiteV1::new(value).is_err());
    }
    for value in [i16::MIN, -1, 256, i16::MAX] {
        assert!(ThresholdLumaV1::new(value).is_err());
    }
    for value in [i16::MIN, 0, 1, 33, i16::MAX] {
        assert!(PosterizeRgbV1::new(value).is_err());
    }
    for descriptor in [
        Descriptor::InvertRgbV1,
        Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(-100, 99).unwrap()),
        Descriptor::BlackWhiteRgbV1(BlackWhiteV1::new(0).unwrap()),
        Descriptor::BlackWhiteRgbV1(BlackWhiteV1::new(100).unwrap()),
        Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(0).unwrap()),
        Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(255).unwrap()),
        Descriptor::PosterizeRgbV1(PosterizeRgbV1::new(2).unwrap()),
        Descriptor::PosterizeRgbV1(PosterizeRgbV1::new(32).unwrap()),
        Descriptor::SolarizeRgbV1,
    ] {
        let value = descriptor.to_value();
        assert_eq!(Descriptor::from_value(&value).unwrap(), descriptor);
        let mut fields = value.as_object().unwrap().clone();
        fields.insert("extra".into(), Value::Null);
        assert_eq!(
            Descriptor::from_value(&Value::Object(fields))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
}
#[test]
fn missing_wrong_type_and_unknown_versions_never_infer_parameters() {
    for text in [
        r#"{"kind":"black-white-rgb-v1"}"#,
        r#"{"kind":"black-white-rgb-v1","strength":true}"#,
        r#"{"kind":"black-white-rgb-v1","strength":101}"#,
        r#"{"kind":"black-white-rgb-v1","strength":"100"}"#,
        r#"{"kind":"threshold-luma-v1","level":null}"#,
        r#"{"kind":"threshold-luma-v1","level":-1}"#,
        r#"{"kind":"threshold-luma-v1","level":256}"#,
        r#"{"kind":"posterize-rgb-v1","levels":1}"#,
        r#"{"kind":"posterize-rgb-v1","levels":33}"#,
        r#"{"kind":"posterize-rgb-v1","levels":9223372036854775807}"#,
        r#"{"kind":"solarize-rgb-v1","level":128}"#,
        r#"{"kind":"solarize-rgb-v2"}"#,
        r#"{"kind":"invert-rgb-v1","strength":0}"#,
        r#"{"kind":"brightness-contrast-v1","brightness":0}"#,
        r#"{"kind":true}"#,
        r#"[]"#,
    ] {
        assert_eq!(
            Descriptor::from_value(&json::parse(text.as_bytes()).unwrap())
                .unwrap_err()
                .code,
            ErrorCode::InputSchema,
            "{text}"
        );
    }
    for text in [
        r#"{"kind":"threshold-luma-v1","level":128.0}"#,
        r#"{"kind":"threshold-luma-v1","level":NaN}"#,
    ] {
        assert!(json::parse(text.as_bytes()).is_err());
    }
    // checked is intentionally the historical two-parameter interface.
    assert_eq!(
        Descriptor::checked("solarize-rgb-v1", None, None)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        Descriptor::checked("invert-rgb-v1", Some(0), None)
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
}
