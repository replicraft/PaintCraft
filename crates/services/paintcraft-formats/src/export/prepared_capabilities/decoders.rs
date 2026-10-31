//! Strict finite transport decoders belonging to the priced execution registry.
use super::*;
use paintcraft_core::types::filters::{
    BlackWhiteV1, BoxBlurV1, BrightnessContrastV1, CurvesRgbV1, ExposureSrgbV1, LevelsRgbV1,
    PosterizeRgbV1, ThresholdLumaV1,
};
fn bad() -> Error {
    Error::new(
        ErrorCode::CodecFailed,
        "Prepared registry parameter framing",
    )
}
pub(super) fn invert(bytes: &[u8]) -> Result<Descriptor> {
    if !bytes.is_empty() {
        return Err(bad());
    }
    Ok(Descriptor::InvertRgbV1)
}
pub(super) fn solarize(bytes: &[u8]) -> Result<Descriptor> {
    if !bytes.is_empty() {
        return Err(bad());
    }
    Ok(Descriptor::SolarizeRgbV1)
}
pub(super) fn brightness(bytes: &[u8]) -> Result<Descriptor> {
    let [a, b, c, d] = bytes else {
        return Err(bad());
    };
    Ok(Descriptor::BrightnessContrastV1(
        BrightnessContrastV1::new(i16::from_be_bytes([*a, *b]), i16::from_be_bytes([*c, *d]))
            .map_err(|_| bad())?,
    ))
}
fn one(bytes: &[u8]) -> Result<i16> {
    let [n] = bytes else { return Err(bad()) };
    Ok(i16::from(*n))
}
pub(super) fn black_white(bytes: &[u8]) -> Result<Descriptor> {
    Ok(Descriptor::BlackWhiteRgbV1(
        BlackWhiteV1::new(one(bytes)?).map_err(|_| bad())?,
    ))
}
pub(super) fn threshold(bytes: &[u8]) -> Result<Descriptor> {
    Ok(Descriptor::ThresholdLumaV1(
        ThresholdLumaV1::new(one(bytes)?).map_err(|_| bad())?,
    ))
}
pub(super) fn posterize(bytes: &[u8]) -> Result<Descriptor> {
    Ok(Descriptor::PosterizeRgbV1(
        PosterizeRgbV1::new(one(bytes)?).map_err(|_| bad())?,
    ))
}
pub(super) fn box_blur(bytes: &[u8]) -> Result<Descriptor> {
    Ok(Descriptor::BoxBlurV1(
        BoxBlurV1::new(one(bytes)?).map_err(|_| bad())?,
    ))
}
pub(super) fn levels(bytes: &[u8]) -> Result<Descriptor> {
    let [a, b, c, d, e, f] = bytes else {
        return Err(bad());
    };
    Ok(Descriptor::LevelsRgbV1(
        LevelsRgbV1::new(
            i16::from(*a),
            i16::from(*b),
            i16::try_from(u16::from_be_bytes([*c, *d])).map_err(|_| bad())?,
            i16::from(*e),
            i16::from(*f),
        )
        .map_err(|_| bad())?,
    ))
}
pub(super) fn exposure(bytes: &[u8]) -> Result<Descriptor> {
    let [a, b] = bytes else { return Err(bad()) };
    Ok(Descriptor::ExposureSrgbV1(
        ExposureSrgbV1::new(i16::from_be_bytes([*a, *b])).map_err(|_| bad())?,
    ))
}

pub(super) fn curves(bytes: &[u8]) -> Result<Descriptor> {
    let [a, b, c, d, e] = bytes else {
        return Err(bad());
    };
    Ok(Descriptor::CurvesRgbV1(CurvesRgbV1::new([
        *a, *b, *c, *d, *e,
    ])))
}

pub(super) fn vibrance(bytes: &[u8]) -> Result<Descriptor> {
    let [a, b] = bytes else { return Err(bad()) };
    Ok(Descriptor::VibranceRgbV1(
        paintcraft_core::types::filters::VibranceRgbV1::new(i16::from_be_bytes([*a, *b]))
            .map_err(|_| bad())?,
    ))
}
