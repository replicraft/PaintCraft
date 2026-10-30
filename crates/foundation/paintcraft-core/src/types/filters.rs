//! Versioned encoded-RGB filter descriptors. These do not change composition policy.
use super::{Error, ErrorCode, Result};
mod coverage;
mod point_params;
mod value;
pub use coverage::{A8Band, A8Input, A8InputBuilder, A8Run, MaskInputLimits};
pub use point_params::{BlackWhiteV1, PosterizeRgbV1, ThresholdLumaV1};

/// Brightness is a signed hundredth of the full channel range; contrast is a
/// signed hundredth in the rational gain (100+c)/(100-c).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BrightnessContrastV1 {
    brightness: i16,
    contrast: i16,
}
impl BrightnessContrastV1 {
    pub fn new(brightness: i16, contrast: i16) -> Result<Self> {
        if !(-100..=100).contains(&brightness) || !(-99..=99).contains(&contrast) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Brightness must be -100..100 and contrast -99..99",
            ));
        }
        Ok(Self {
            brightness,
            contrast,
        })
    }
    pub const fn brightness(self) -> i16 {
        self.brightness
    }
    pub const fn contrast(self) -> i16 {
        self.contrast
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Descriptor {
    InvertRgbV1,
    BrightnessContrastV1(BrightnessContrastV1),
    BlackWhiteRgbV1(BlackWhiteV1),
    ThresholdLumaV1(ThresholdLumaV1),
    PosterizeRgbV1(PosterizeRgbV1),
    SolarizeRgbV1,
}
impl Descriptor {
    pub const fn id(self) -> &'static str {
        match self {
            Self::InvertRgbV1 => "invert-rgb-v1",
            Self::BrightnessContrastV1(_) => "brightness-contrast-v1",
            Self::BlackWhiteRgbV1(_) => "black-white-rgb-v1",
            Self::ThresholdLumaV1(_) => "threshold-luma-v1",
            Self::PosterizeRgbV1(_) => "posterize-rgb-v1",
            Self::SolarizeRgbV1 => "solarize-rgb-v1",
        }
    }
    /// Stable inspection/command representation; versions are part of kind.
    pub fn to_value(self) -> super::json::Value {
        use super::json::Value;
        match self {
            Self::InvertRgbV1 | Self::SolarizeRgbV1 => {
                Value::object([("kind", Value::str(self.id()))])
            }
            Self::BrightnessContrastV1(p) => Value::object([
                ("kind", Value::str(self.id())),
                ("brightness", Value::Int(i64::from(p.brightness()))),
                ("contrast", Value::Int(i64::from(p.contrast()))),
            ]),
            Self::BlackWhiteRgbV1(p) => Value::object([
                ("kind", Value::str(self.id())),
                ("strength", Value::Int(i64::from(p.strength()))),
            ]),
            Self::ThresholdLumaV1(p) => Value::object([
                ("kind", Value::str(self.id())),
                ("level", Value::Int(i64::from(p.level()))),
            ]),
            Self::PosterizeRgbV1(p) => Value::object([
                ("kind", Value::str(self.id())),
                ("levels", Value::Int(i64::from(p.levels()))),
            ]),
        }
    }
    /// Legacy two-profile constructor: its optional fields describe only Invert
    /// and Brightness/Contrast. New profiles require strict `from_value`, not
    /// defaults inferred from these unrelated optional parameters.
    /// Unknown algorithms/versions are never approximated by a known filter.
    pub fn checked(id: &str, brightness: Option<i16>, contrast: Option<i16>) -> Result<Self> {
        match (id, brightness, contrast) {
            ("invert-rgb-v1", None, None) => Ok(Self::InvertRgbV1),
            ("brightness-contrast-v1", Some(b), Some(c)) => {
                BrightnessContrastV1::new(b, c).map(Self::BrightnessContrastV1)
            }
            ("invert-rgb-v1" | "brightness-contrast-v1", _, _) => Err(Error::new(
                ErrorCode::InputSchema,
                "Invalid filter parameters",
            )),
            _ => Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Unknown filter algorithm/version",
            )),
        }
    }
}

#[cfg(test)]
mod point_tests;
