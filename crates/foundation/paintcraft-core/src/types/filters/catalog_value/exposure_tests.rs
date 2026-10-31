use super::*;

#[test]
fn exposure_catalog_retains_signed_intent_and_never_enters_legacy_constructors() {
    for ev in [-800, -799, -1, 0, 1, 799, 800] {
        let p = ExposureSrgbV1::new(ev).unwrap();
        let descriptor = Descriptor::ExposureSrgbV1(p);
        assert!(descriptor.is_catalog_only());
        assert!(!descriptor.is_spatial());
        assert_eq!(descriptor.id(), "exposure-srgb-v1");
        let (bytes, used) = descriptor.semantic_parameter_bytes();
        assert_eq!(used, 2);
        assert_eq!(&bytes[..used], &ev.to_be_bytes());
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
}

#[test]
fn exposure_catalog_refuses_missing_extra_float_and_out_of_range_fields() {
    for text in [
        r#"{"kind":"exposure-srgb-v1"}"#,
        r#"{"kind":"exposure-srgb-v1","stops_hundredths":0,"gamma":1}"#,
        r#"{"kind":"exposure-srgb-v1","stops_hundredths":"100"}"#,
        r#"{"kind":"exposure-srgb-v1","stops_hundredths":true}"#,
        r#"{"kind":"exposure-srgb-v1","stops_hundredths":null}"#,
        r#"{"kind":"exposure-srgb-v1","stops_hundredths":-801}"#,
        r#"{"kind":"exposure-srgb-v1","stops_hundredths":801}"#,
        r#"{"kind":"exposure-srgb-v1","stops_hundredths":65536}"#,
        r#"{"kind":"exposure-srgb-v2","stops_hundredths":0}"#,
    ] {
        let value = crate::types::json::parse(text.as_bytes()).unwrap();
        assert_eq!(
            Descriptor::from_catalog_value(&value).unwrap_err().code,
            ErrorCode::InputSchema,
            "{text}"
        );
    }
    // Lexical floats are refused by the wire parser before descriptor dispatch.
    assert!(
        crate::types::json::parse(br#"{"kind":"exposure-srgb-v1","stops_hundredths":1.0}"#)
            .is_err()
    );
}
