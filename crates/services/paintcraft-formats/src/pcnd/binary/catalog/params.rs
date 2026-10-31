//! Fixed typed parameters; transport names are scoped by numerical profile.
use super::*;
use paintcraft_core::types::filters::{
    BlackWhiteV1, BoxBlurV1, BrightnessContrastV1, CurvesRgbV1, Descriptor, ExposureSrgbV1,
    LevelsRgbV1, PosterizeRgbV1, ThresholdLumaV1, VibranceRgbV1,
};
pub(super) fn require(profile: &str, encoding: &str) -> Result<()> {
    if encoding != "params-v1"
        || !matches!(
            profile,
            "invert-rgb-v1"
                | "brightness-contrast-v1"
                | "black-white-rgb-v1"
                | "threshold-luma-v1"
                | "posterize-rgb-v1"
                | "solarize-rgb-v1"
                | "box-blur-v1"
                | "levels-rgb-v1"
                | "exposure-srgb-v1"
                | "curves-rgb-v1"
                | "vibrance-rgb-v1"
        )
    {
        return Err(unsupported("unknown PCND catalog algorithm/codec"));
    }
    Ok(())
}
pub(super) fn decode(profile: &str, bytes: &[u8]) -> Result<Descriptor> {
    let invalid = || codec("invalid PCND catalog parameters");
    Ok(match (profile, bytes) {
        ("invert-rgb-v1", []) => Descriptor::InvertRgbV1,
        ("solarize-rgb-v1", []) => Descriptor::SolarizeRgbV1,
        ("brightness-contrast-v1", [a, b, c, d]) => Descriptor::BrightnessContrastV1(
            BrightnessContrastV1::new(i16::from_be_bytes([*a, *b]), i16::from_be_bytes([*c, *d]))
                .map_err(|_| invalid())?,
        ),
        ("black-white-rgb-v1", [n]) => {
            Descriptor::BlackWhiteRgbV1(BlackWhiteV1::new(i16::from(*n)).map_err(|_| invalid())?)
        }
        ("threshold-luma-v1", [n]) => {
            Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(i16::from(*n)).map_err(|_| invalid())?)
        }
        ("posterize-rgb-v1", [n]) => {
            Descriptor::PosterizeRgbV1(PosterizeRgbV1::new(i16::from(*n)).map_err(|_| invalid())?)
        }
        ("box-blur-v1", [n]) => {
            Descriptor::BoxBlurV1(BoxBlurV1::new(i16::from(*n)).map_err(|_| invalid())?)
        }
        ("levels-rgb-v1", [a, b, c, d, e, f]) => Descriptor::LevelsRgbV1(
            LevelsRgbV1::new(
                i16::from(*a),
                i16::from(*b),
                i16::try_from(u16::from_be_bytes([*c, *d])).map_err(|_| invalid())?,
                i16::from(*e),
                i16::from(*f),
            )
            .map_err(|_| invalid())?,
        ),
        ("exposure-srgb-v1", [a, b]) => Descriptor::ExposureSrgbV1(
            ExposureSrgbV1::new(i16::from_be_bytes([*a, *b])).map_err(|_| invalid())?,
        ),
        ("curves-rgb-v1", [a, b, c, d, e]) => {
            Descriptor::CurvesRgbV1(CurvesRgbV1::new([*a, *b, *c, *d, *e]))
        }
        ("vibrance-rgb-v1", [a, b]) => Descriptor::VibranceRgbV1(
            VibranceRgbV1::new(i16::from_be_bytes([*a, *b])).map_err(|_| invalid())?,
        ),
        _ => return Err(invalid()),
    })
}
pub(super) fn encode(descriptor: Descriptor) -> ([u8; 6], usize) {
    descriptor.semantic_parameter_bytes()
}
