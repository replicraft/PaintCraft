//! Strict closed descriptor objects; UI defaults never imply missing wire data.
use super::*;
use crate::types::json::Value;
fn bad() -> Error {
    Error::new(
        ErrorCode::InputSchema,
        "Invalid filter descriptor fields or parameter type",
    )
}
impl Descriptor {
    pub fn from_value(value: &Value) -> Result<Self> {
        let object = value.as_object().ok_or_else(bad)?;
        let kind = object.get("kind").and_then(Value::as_str).ok_or_else(bad)?;
        let fields: &[&str] = match kind {
            "invert-rgb-v1" | "solarize-rgb-v1" => &["kind"],
            "brightness-contrast-v1" => &["kind", "brightness", "contrast"],
            "black-white-rgb-v1" => &["kind", "strength"],
            "threshold-luma-v1" => &["kind", "level"],
            "posterize-rgb-v1" => &["kind", "levels"],
            "box-blur-v1" => &["kind", "radius"],
            _ => {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "Unknown filter algorithm/version",
                ));
            }
        };
        if object.len() != fields.len() || object.keys().any(|k| !fields.contains(&k.as_str())) {
            return Err(bad());
        }
        let parameter = |name: &str| -> Result<i16> {
            let value = object.get(name).and_then(Value::as_i64).ok_or_else(bad)?;
            i16::try_from(value).map_err(|_| bad())
        };
        Ok(match kind {
            "invert-rgb-v1" => Self::InvertRgbV1,
            "solarize-rgb-v1" => Self::SolarizeRgbV1,
            "brightness-contrast-v1" => Self::BrightnessContrastV1(BrightnessContrastV1::new(
                parameter("brightness")?,
                parameter("contrast")?,
            )?),
            "black-white-rgb-v1" => {
                Self::BlackWhiteRgbV1(BlackWhiteV1::new(parameter("strength")?)?)
            }
            "threshold-luma-v1" => {
                Self::ThresholdLumaV1(ThresholdLumaV1::new(parameter("level")?)?)
            }
            "posterize-rgb-v1" => Self::PosterizeRgbV1(PosterizeRgbV1::new(parameter("levels")?)?),
            "box-blur-v1" => Self::BoxBlurV1(BoxBlurV1::new(parameter("radius")?)?),
            _ => unreachable!("kind and exact field set validated above"),
        })
    }
}
