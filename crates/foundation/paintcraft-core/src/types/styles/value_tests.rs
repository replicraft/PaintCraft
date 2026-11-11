use super::*;
use crate::types::json::Value;
#[test]
fn descriptor_roundtrip_closed_fields_exact_bytes_and_foreign_modes_refuse() {
    for mode in [
        EffectBlendMode::Normal,
        EffectBlendMode::Multiply,
        EffectBlendMode::Screen,
    ] {
        for opacity in [0, 1, 128, 255] {
            let d = StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
                [0, 127, 255],
                opacity,
                mode,
            ));
            let v = d.to_value();
            assert_eq!(StyleDescriptor::from_value(&v).unwrap(), d);
            for (key, bad) in [
                ("color", Value::Array(vec![Value::Int(256); 3])),
                ("opacity_u8", Value::Int(-1)),
                ("effect_blend_mode", Value::str("overlay")),
                ("kind", Value::str("invert-rgb-v1")),
                ("unknown", Value::Bool(true)),
            ] {
                let Value::Object(mut fields) = v.clone() else {
                    panic!()
                };
                fields.insert(key.into(), bad);
                assert_eq!(
                    StyleDescriptor::from_value(&Value::Object(fields))
                        .unwrap_err()
                        .code,
                    ErrorCode::InputSchema
                );
            }
        }
    }
}
