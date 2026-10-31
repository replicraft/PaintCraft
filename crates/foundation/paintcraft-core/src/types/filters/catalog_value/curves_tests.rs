use super::*;
use crate::types::json;
#[test]
fn curves_catalog_roundtrips_free_outputs_and_complete_five_byte_intent_only_explicitly() {
    for outputs in [
        [0, 64, 128, 192, 255],
        [0; 5],
        [255; 5],
        [255, 0, 255, 0, 255],
        [23, 9, 240, 73, 201],
    ] {
        let descriptor = Descriptor::CurvesRgbV1(CurvesRgbV1::new(outputs));
        let expected = Value::object([
            ("kind", Value::str("curves-rgb-v1")),
            (
                "outputs",
                Value::Array(
                    outputs
                        .into_iter()
                        .map(|value| Value::Int(i64::from(value)))
                        .collect(),
                ),
            ),
        ]);
        assert_eq!(descriptor.id(), "curves-rgb-v1");
        assert!(descriptor.is_catalog_only());
        assert!(!descriptor.is_spatial());
        assert_eq!(descriptor.to_value(), expected);
        assert_eq!(
            Descriptor::from_catalog_value(&expected).unwrap(),
            descriptor
        );
        let (bytes, length) = descriptor.semantic_parameter_bytes();
        assert_eq!(length, 5);
        assert_eq!(&bytes[..length], &outputs);
        assert_eq!(bytes[5], 0);
        assert_eq!(
            Descriptor::from_value(&expected).unwrap_err().code,
            ErrorCode::InputSchema
        );
        assert_eq!(
            Descriptor::checked(descriptor.id(), None, None)
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    let value = Descriptor::CurvesRgbV1(CurvesRgbV1::default()).to_value();
    assert_eq!(
        json::canonical(&value),
        br#"{"kind":"curves-rgb-v1","outputs":[0,64,128,192,255]}"#
    );
}
#[test]
fn curves_catalog_is_closed_in_shape_fields_integer_type_and_byte_domain() {
    for text in [
        r#"{"kind":"curves-rgb-v1"}"#,
        r#"{"kind":"curves-rgb-v1","points":[0,64,128,192,255]}"#,
        r#"{"kind":"curves-rgb-v1","outputs":[0,64,128,192,255],"points":[0,64,128,192,255]}"#,
        r#"{"kind":"curves-rgb-v1","outputs":[]}"#,
        r#"{"kind":"curves-rgb-v1","outputs":[0,64,128,255]}"#,
        r#"{"kind":"curves-rgb-v1","outputs":[0,64,128,192,255,1]}"#,
        r#"{"kind":"curves-rgb-v1","outputs":null}"#,
        r#"{"kind":"curves-rgb-v1","outputs":"identity"}"#,
        r#"{"kind":"curves-rgb-v1","outputs":{"0":0}}"#,
        r#"{"kind":"curves-rgb-v1","outputs":[true,64,128,192,255]}"#,
        r#"{"kind":"curves-rgb-v1","outputs":[null,64,128,192,255]}"#,
        r#"{"kind":"curves-rgb-v1","outputs":["0",64,128,192,255]}"#,
        r#"{"kind":"curves-rgb-v1","outputs":[-1,64,128,192,255]}"#,
        r#"{"kind":"curves-rgb-v1","outputs":[0,64,128,192,256]}"#,
        r#"{"kind":"curves-rgb-v1","outputs":[9223372036854775807,64,128,192,255]}"#,
        r#"{"kind":"curves-rgb-v2","outputs":[0,64,128,192,255]}"#,
    ] {
        let value = json::parse(text.as_bytes()).unwrap();
        assert_eq!(
            Descriptor::from_catalog_value(&value).unwrap_err().code,
            ErrorCode::InputSchema,
            "{text}"
        );
    }
    for text in [
        br#"{"kind":"curves-rgb-v1","outputs":[0.0,64,128,192,255]}"#.as_slice(),
        br#"{"kind":"curves-rgb-v1","outputs":[0e0,64,128,192,255]}"#.as_slice(),
    ] {
        assert!(json::parse(text).is_err());
    }
    // The descriptor inherits the existing global integer lexical boundary.
    let scalar = json::parse(b"-0");
    let object = json::parse(br#"{"kind":"curves-rgb-v1","outputs":[-0,64,128,192,255]}"#);
    assert_eq!(scalar.is_ok(), object.is_ok());
    if scalar.is_ok() {
        assert_eq!(
            Descriptor::from_catalog_value(&object.unwrap()).unwrap(),
            Descriptor::CurvesRgbV1(CurvesRgbV1::default())
        );
    }
}
#[test]
fn old_nine_profile_ids_semantic_bytes_and_json_stay_exact() {
    let rows:&[(Descriptor,&str,&[u8],&[u8])]=&[
        (Descriptor::InvertRgbV1,"invert-rgb-v1",&[],br#"{"kind":"invert-rgb-v1"}"#),
        (Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(-17,23).unwrap()),"brightness-contrast-v1",&[255,239,0,23],br#"{"brightness":-17,"contrast":23,"kind":"brightness-contrast-v1"}"#),
        (Descriptor::BlackWhiteRgbV1(BlackWhiteV1::default()),"black-white-rgb-v1",&[100],br#"{"kind":"black-white-rgb-v1","strength":100}"#),
        (Descriptor::ThresholdLumaV1(ThresholdLumaV1::default()),"threshold-luma-v1",&[128],br#"{"kind":"threshold-luma-v1","level":128}"#),
        (Descriptor::PosterizeRgbV1(PosterizeRgbV1::default()),"posterize-rgb-v1",&[6],br#"{"kind":"posterize-rgb-v1","levels":6}"#),
        (Descriptor::SolarizeRgbV1,"solarize-rgb-v1",&[],br#"{"kind":"solarize-rgb-v1"}"#),
        (Descriptor::BoxBlurV1(BoxBlurV1::new(32).unwrap()),"box-blur-v1",&[32],br#"{"kind":"box-blur-v1","radius":32}"#),
        (Descriptor::LevelsRgbV1(LevelsRgbV1::new(17,201,999,240,16).unwrap()),"levels-rgb-v1",&[17,201,3,231,240,16],br#"{"gamma_hundredths":999,"input_black":17,"input_white":201,"kind":"levels-rgb-v1","output_black":240,"output_white":16}"#),
        (Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(-800).unwrap()),"exposure-srgb-v1",&[252,224],br#"{"kind":"exposure-srgb-v1","stops_hundredths":-800}"#),
    ];
    for (descriptor, id, expected_bytes, expected_json) in rows {
        assert_eq!(descriptor.id(), *id);
        let (bytes, length) = descriptor.semantic_parameter_bytes();
        assert_eq!(&bytes[..length], *expected_bytes);
        assert_eq!(json::canonical(&descriptor.to_value()), *expected_json);
        assert_eq!(
            Descriptor::from_catalog_value(&descriptor.to_value()).unwrap(),
            *descriptor
        );
    }
}
