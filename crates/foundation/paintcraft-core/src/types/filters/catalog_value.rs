//! Explicit catalog construction. The original seven-profile grammar is closed.
use super::*;
use crate::types::json::Value;

impl Descriptor {
    /// Strict finite registered catalog. Legacy callers retain `from_value` and its
    /// unchanged seven kinds; using this method is an explicit capability choice.
    pub fn from_catalog_value(value: &Value) -> Result<Self> {
        let Some(object) = value.as_object() else {
            return Self::from_value(value);
        };
        let kind = object.get("kind").and_then(Value::as_str);
        let fields: &[&str] = match kind {
            Some("levels-rgb-v1") => &[
                "kind",
                "input_black",
                "input_white",
                "gamma_hundredths",
                "output_black",
                "output_white",
            ],
            Some("exposure-srgb-v1") => &["kind", "stops_hundredths"],
            Some("curves-rgb-v1") => &["kind", "outputs"],
            Some("vibrance-rgb-v1") => &["kind", "amount_hundredths"],
            Some("hue-saturation-rgb-v1") => &[
                "kind",
                "hue_degrees",
                "saturation_hundredths",
                "lightness_hundredths",
            ],
            _ => return Self::from_value(value),
        };
        let bad = || {
            Error::new(
                ErrorCode::InputSchema,
                "Invalid filter descriptor fields or parameter type",
            )
        };
        if object.len() != fields.len() || object.keys().any(|key| !fields.contains(&key.as_str()))
        {
            return Err(bad());
        }
        let parameter = |key| {
            object
                .get(key)
                .and_then(Value::as_i64)
                .and_then(|v| i16::try_from(v).ok())
                .ok_or_else(bad)
        };
        Ok(match kind {
            Some("hue-saturation-rgb-v1") => Self::HueSaturationRgbV1(HueSaturationRgbV1::new(
                parameter("hue_degrees")?,
                parameter("saturation_hundredths")?,
                parameter("lightness_hundredths")?,
            )?),
            Some("vibrance-rgb-v1") => {
                Self::VibranceRgbV1(VibranceRgbV1::new(parameter("amount_hundredths")?)?)
            }
            Some("curves-rgb-v1") => {
                let values = object
                    .get("outputs")
                    .and_then(Value::as_array)
                    .filter(|values| values.len() == 5)
                    .ok_or_else(bad)?;
                let mut outputs = [0; 5];
                for (output, value) in outputs.iter_mut().zip(values) {
                    *output = value
                        .as_i64()
                        .and_then(|value| u8::try_from(value).ok())
                        .ok_or_else(bad)?;
                }
                Self::CurvesRgbV1(CurvesRgbV1::new(outputs))
            }
            Some("exposure-srgb-v1") => {
                Self::ExposureSrgbV1(ExposureSrgbV1::new(parameter("stops_hundredths")?)?)
            }
            _ => Self::LevelsRgbV1(LevelsRgbV1::new(
                parameter("input_black")?,
                parameter("input_white")?,
                parameter("gamma_hundredths")?,
                parameter("output_black")?,
                parameter("output_white")?,
            )?),
        })
    }
}

#[cfg(test)]
#[path = "catalog_value/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "catalog_value/exposure_tests.rs"]
mod exposure_tests;

#[cfg(test)]
#[path = "catalog_value/curves_tests.rs"]
mod curves_tests;

#[cfg(test)]
#[path = "catalog_value/vibrance_tests.rs"]
mod vibrance_tests;

#[cfg(test)]
#[path = "catalog_value/hue_saturation_tests.rs"]
mod hue_saturation_tests;
