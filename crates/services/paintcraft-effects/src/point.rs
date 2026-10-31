//! One full candidate from the immutable incoming tuple, before any A8 blend.
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba, filters::Descriptor};
pub(super) fn require_point(filter: Descriptor) -> Result<()> {
    if filter.is_spatial() {
        return Err(spatial_error());
    }
    if filter.is_catalog_only() {
        return Err(prepared_error(filter));
    }
    Ok(())
}
fn prepared_error(filter: Descriptor) -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        match filter {
            Descriptor::LevelsRgbV1(_) => "Levels filter requires its admitted prepared program",
            Descriptor::ExposureSrgbV1(_) => {
                "Exposure filter requires its admitted prepared program"
            }
            _ => "Catalog filter requires an admitted prepared program",
        },
    )
}
fn spatial_error() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Spatial filter requires admitted finite-source evaluation",
    )
}
fn round(n: u64, d: u64) -> u8 {
    ((n + d / 2) / d) as u8
}
fn luma(rgb: [u8; 3]) -> u64 {
    u64::from(rgb[0]) * 2126 + u64::from(rgb[1]) * 7152 + u64::from(rgb[2]) * 722
}
pub(super) fn evaluate(filter: Descriptor, pixel: Rgba) -> Result<Rgba> {
    let [r, g, b, alpha] = pixel.0;
    let rgb = [r, g, b];
    let candidate = match filter {
        Descriptor::InvertRgbV1 => rgb.map(|v| 255 - v),
        Descriptor::BrightnessContrastV1(p) => rgb.map(|v| {
            let (b, c, v) = (
                i64::from(p.brightness()),
                i64::from(p.contrast()),
                i64::from(v),
            );
            let denominator = 200 * (100 - c);
            let numerator =
                (2 * v - 255) * 100 * (100 + c) + 255 * 100 * (100 - c) + 510 * b * (100 - c);
            ((numerator.clamp(0, 255 * denominator) + denominator / 2) / denominator) as u8
        }),
        Descriptor::BlackWhiteRgbV1(p) => {
            let y = luma(rgb);
            let strength = u64::from(p.strength());
            rgb.map(|v| {
                round(
                    (100 - strength) * 10000 * u64::from(v) + strength * y,
                    1_000_000,
                )
            })
        }
        Descriptor::ThresholdLumaV1(p) => {
            [if luma(rgb) >= 10000 * u64::from(p.level()) {
                255
            } else {
                0
            }; 3]
        }
        Descriptor::PosterizeRgbV1(p) => {
            let k = u64::from(p.levels() - 1);
            rgb.map(|v| {
                let q = (u64::from(v) * k + 127) / 255;
                round(255 * q, k)
            })
        }
        Descriptor::SolarizeRgbV1 => rgb.map(|v| if v >= 128 { 255 - v } else { v }),
        Descriptor::BoxBlurV1(_) => return Err(spatial_error()),
        Descriptor::LevelsRgbV1(_) | Descriptor::ExposureSrgbV1(_) => {
            return Err(prepared_error(filter));
        }
    };
    Ok(Rgba([candidate[0], candidate[1], candidate[2], alpha]))
}
