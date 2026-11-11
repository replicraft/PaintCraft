//! Closed Color Overlay descriptor, separate from filter catalog grammar.
use super::*;
use crate::types::json::Value;
impl StyleDescriptor {
    pub fn to_value(self) -> Value {
        let Self::ColorOverlayRgbV1(policy) = self;
        Value::object([
            ("kind", Value::str(ColorOverlayRgbV1::PROFILE)),
            (
                "color",
                Value::Array(
                    policy
                        .color()
                        .into_iter()
                        .map(|n| Value::Int(i64::from(n)))
                        .collect(),
                ),
            ),
            ("opacity_u8", Value::Int(i64::from(policy.opacity()))),
            (
                "effect_blend_mode",
                Value::str(match policy.blend() {
                    EffectBlendMode::Normal => "normal",
                    EffectBlendMode::Multiply => "multiply",
                    EffectBlendMode::Screen => "screen",
                }),
            ),
        ])
    }
    pub fn from_value(value: &Value) -> Result<Self> {
        let bad = || Error::new(ErrorCode::InputSchema, "Invalid Color Overlay descriptor");
        let fields = value.as_object().ok_or_else(bad)?;
        let keys = ["kind", "color", "opacity_u8", "effect_blend_mode"];
        if fields.len() != keys.len() || keys.iter().any(|key| !fields.contains_key(*key)) {
            return Err(bad());
        }
        if fields["kind"].as_str() != Some(ColorOverlayRgbV1::PROFILE) {
            return Err(bad());
        }
        let channel = |v: &Value| {
            v.as_i64()
                .filter(|n| (0..=255).contains(n))
                .map(|n| n as u8)
                .ok_or_else(bad)
        };
        let color = fields["color"]
            .as_array()
            .filter(|v| v.len() == 3)
            .ok_or_else(bad)?;
        let color = [
            channel(&color[0])?,
            channel(&color[1])?,
            channel(&color[2])?,
        ];
        let opacity = channel(&fields["opacity_u8"])?;
        let blend = match fields["effect_blend_mode"].as_str() {
            Some("normal") => EffectBlendMode::Normal,
            Some("multiply") => EffectBlendMode::Multiply,
            Some("screen") => EffectBlendMode::Screen,
            _ => return Err(bad()),
        };
        Ok(Self::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
            color, opacity, blend,
        )))
    }
}
