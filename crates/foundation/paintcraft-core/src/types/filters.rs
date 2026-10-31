//! Versioned encoded-RGB filter descriptors. These do not change composition policy.
use super::{Error, ErrorCode, Result};
mod catalog_value;
mod coverage;
mod curves_params;
mod exposure_params;
mod levels_params;
mod point_params;
mod spatial_params;
mod value;
pub use coverage::{A8Band, A8Input, A8InputBuilder, A8Run, MaskInputLimits};
pub use curves_params::CurvesRgbV1;
pub use exposure_params::ExposureSrgbV1;
pub use levels_params::LevelsRgbV1;
pub use point_params::{BlackWhiteV1, PosterizeRgbV1, ThresholdLumaV1};
pub use spatial_params::BoxBlurV1;

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
    BoxBlurV1(BoxBlurV1),
    LevelsRgbV1(LevelsRgbV1),
    ExposureSrgbV1(ExposureSrgbV1),
    CurvesRgbV1(CurvesRgbV1),
}
impl Descriptor {
    /// Spatial intentions require a finite source and neighborhood admission.
    /// Radius zero retains this classification; it grants no point-only API.
    pub const fn is_spatial(self) -> bool {
        match self {
            Self::BoxBlurV1(_) => true,
            Self::InvertRgbV1
            | Self::BrightnessContrastV1(_)
            | Self::BlackWhiteRgbV1(_)
            | Self::ThresholdLumaV1(_)
            | Self::PosterizeRgbV1(_)
            | Self::SolarizeRgbV1
            | Self::LevelsRgbV1(_)
            | Self::ExposureSrgbV1(_)
            | Self::CurvesRgbV1(_) => false,
        }
    }
    /// Requires the explicit catalog constructor/persistence boundary. The old
    /// seven-profile parser and container registries do not gain this capability.
    pub const fn is_catalog_only(self) -> bool {
        match self {
            Self::LevelsRgbV1(_) | Self::ExposureSrgbV1(_) | Self::CurvesRgbV1(_) => true,
            Self::InvertRgbV1
            | Self::BrightnessContrastV1(_)
            | Self::BlackWhiteRgbV1(_)
            | Self::ThresholdLumaV1(_)
            | Self::PosterizeRgbV1(_)
            | Self::SolarizeRgbV1
            | Self::BoxBlurV1(_) => false,
        }
    }
    pub const fn id(self) -> &'static str {
        match self {
            Self::InvertRgbV1 => "invert-rgb-v1",
            Self::BrightnessContrastV1(_) => "brightness-contrast-v1",
            Self::BlackWhiteRgbV1(_) => "black-white-rgb-v1",
            Self::ThresholdLumaV1(_) => "threshold-luma-v1",
            Self::PosterizeRgbV1(_) => "posterize-rgb-v1",
            Self::SolarizeRgbV1 => "solarize-rgb-v1",
            Self::BoxBlurV1(_) => "box-blur-v1",
            Self::LevelsRgbV1(_) => "levels-rgb-v1",
            Self::ExposureSrgbV1(_) => "exposure-srgb-v1",
            Self::CurvesRgbV1(_) => "curves-rgb-v1",
        }
    }
    /// Canonical semantic bytes for the finite registered numerical profiles.
    /// Only the returned used prefix is semantic; zero padding is excluded.
    /// This is independent of any transport codec, JSON or derived lookup table.
    pub fn semantic_parameter_bytes(self) -> ([u8; 6], usize) {
        let mut bytes = [0; 6];
        let length = match self {
            Self::InvertRgbV1 | Self::SolarizeRgbV1 => 0,
            Self::BrightnessContrastV1(p) => {
                bytes[..2].copy_from_slice(&p.brightness().to_be_bytes());
                bytes[2..4].copy_from_slice(&p.contrast().to_be_bytes());
                4
            }
            Self::BlackWhiteRgbV1(p) => {
                bytes[0] = p.strength();
                1
            }
            Self::ThresholdLumaV1(p) => {
                bytes[0] = p.level();
                1
            }
            Self::PosterizeRgbV1(p) => {
                bytes[0] = p.levels();
                1
            }
            Self::BoxBlurV1(p) => {
                bytes[0] = p.radius();
                1
            }
            Self::LevelsRgbV1(p) => {
                bytes[0] = p.input_black();
                bytes[1] = p.input_white();
                bytes[2..4].copy_from_slice(&p.gamma_hundredths().to_be_bytes());
                bytes[4] = p.output_black();
                bytes[5] = p.output_white();
                6
            }
            Self::ExposureSrgbV1(p) => {
                bytes[..2].copy_from_slice(&p.semantic_bytes());
                2
            }
            Self::CurvesRgbV1(p) => {
                bytes[..5].copy_from_slice(&p.semantic_bytes());
                5
            }
        };
        (bytes, length)
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
            Self::BoxBlurV1(p) => Value::object([
                ("kind", Value::str(self.id())),
                ("radius", Value::Int(i64::from(p.radius()))),
            ]),
            Self::LevelsRgbV1(p) => Value::object([
                ("kind", Value::str(self.id())),
                ("input_black", Value::Int(i64::from(p.input_black()))),
                ("input_white", Value::Int(i64::from(p.input_white()))),
                (
                    "gamma_hundredths",
                    Value::Int(i64::from(p.gamma_hundredths())),
                ),
                ("output_black", Value::Int(i64::from(p.output_black()))),
                ("output_white", Value::Int(i64::from(p.output_white()))),
            ]),
            Self::ExposureSrgbV1(p) => Value::object([
                ("kind", Value::str(self.id())),
                (
                    "stops_hundredths",
                    Value::Int(i64::from(p.stops_hundredths())),
                ),
            ]),
            Self::CurvesRgbV1(p) => Value::object([
                ("kind", Value::str(self.id())),
                (
                    "outputs",
                    Value::Array(
                        p.outputs()
                            .into_iter()
                            .map(|value| Value::Int(i64::from(value)))
                            .collect(),
                    ),
                ),
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
#[cfg(test)]
mod spatial_tests;
