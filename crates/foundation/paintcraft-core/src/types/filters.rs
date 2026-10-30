//! Versioned encoded-RGB filter descriptors. These do not change composition policy.
use super::{Error, ErrorCode, Result};

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
}
impl Descriptor {
    pub const fn id(self) -> &'static str {
        match self {
            Self::InvertRgbV1 => "invert-rgb-v1",
            Self::BrightnessContrastV1(_) => "brightness-contrast-v1",
        }
    }
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
