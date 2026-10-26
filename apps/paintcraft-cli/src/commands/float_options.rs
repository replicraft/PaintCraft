//! One finite CLI/MCP settings DTO; no format route, decoder or implicit policy.
use paintcraft_core::types::{Error, ErrorCode, Result};
#[cfg(test)]
use paintcraft_formats::float_import::wire;
use paintcraft_formats::precision;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, Deserialize, Serialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Curve {
    Clip,
    ReinhardPerChannel,
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, Deserialize, Serialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Negative {
    Reject,
    ClipToBlack,
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, Deserialize, Serialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum AlphaOutput {
    Preserve,
    Flatten,
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, Deserialize, Serialize, JsonSchema,
)]
pub enum SourceSpace {
    #[serde(rename = "linear-rec709-d65")]
    #[value(name = "linear-rec709-d65")]
    Rec709D65,
    #[serde(rename = "linear-radiance-nominal-ee")]
    #[value(name = "linear-radiance-nominal-ee")]
    RadianceEqualEnergy,
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, Deserialize, Serialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum AlphaAssignment {
    Premultiplied,
    Straight,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::Args, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    /// Explicit linear clipping or Reinhard per-channel curve; no inferred tone map.
    #[arg(long = "float-curve", value_enum)]
    pub curve: Curve,
    /// Exposure in thousandths of an EV, from -16000 through 16000.
    #[arg(long = "float-exposure-milli-ev", allow_hyphen_values = true,
          value_parser = clap::value_parser!(i32).range(-16000..=16000))]
    #[schemars(range(min = -16000, max = 16000))]
    pub exposure_milli_ev: i32,
    #[arg(long = "float-negative", value_enum)]
    pub negative: Negative,
    /// Preserve alpha or explicitly flatten in linear light over the given backdrop.
    #[arg(long = "float-alpha", value_enum)]
    pub alpha_output: AlphaOutput,
    /// Exactly R,G,B in 0..255; required only for flatten, refused for preserve.
    #[arg(long = "float-background", value_parser = parse_rgb,
          required_if_eq("alpha_output", "flatten"))]
    pub srgb_background: Option<[u8; 3]>,
    /// Explicit override/assignment; omitted means use admitted source evidence.
    #[arg(long = "float-source-space", value_enum)]
    pub source_assignment: Option<SourceSpace>,
    /// Explicit override, with actual source-dependent consent; never inferred here.
    #[arg(long = "float-alpha-assignment", value_enum)]
    pub alpha_assignment: Option<AlphaAssignment>,
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
fn parse_rgb(value: &str) -> std::result::Result<[u8; 3], String> {
    let mut fields = value.split(',');
    let mut rgb = [0; 3];
    for channel in &mut rgb {
        *channel = fields
            .next()
            .ok_or("backdrop requires exactly R,G,B")?
            .parse::<u8>()
            .map_err(|_| "backdrop components must be integer 0..255")?;
    }
    if fields.next().is_some() {
        return Err("backdrop requires exactly R,G,B".into());
    }
    Ok(rgb)
}
impl Selection {
    pub fn settings(self) -> Result<precision::Settings> {
        if !(-16000..=16000).contains(&self.exposure_milli_ev) {
            return Err(invalid("Float exposure must be -16000..16000 milli-EV"));
        }
        let alpha_output = match (self.alpha_output, self.srgb_background) {
            (AlphaOutput::Preserve, None) => precision::AlphaOutput::Preserve,
            (AlphaOutput::Flatten, Some(srgb_background)) => {
                precision::AlphaOutput::Flatten { srgb_background }
            }
            (AlphaOutput::Preserve, Some(_)) => {
                return Err(invalid("Preserve alpha refuses an inactive backdrop"));
            }
            (AlphaOutput::Flatten, None) => {
                return Err(invalid("Flatten alpha requires an explicit sRGB backdrop"));
            }
        };
        Ok(precision::Settings {
            curve: match self.curve {
                Curve::Clip => precision::Curve::Clip,
                Curve::ReinhardPerChannel => precision::Curve::ReinhardPerChannel,
            },
            exposure_milli_ev: self.exposure_milli_ev,
            negative: match self.negative {
                Negative::Reject => precision::Negative::Reject,
                Negative::ClipToBlack => precision::Negative::ClipToBlack,
            },
            alpha_output,
            source_assignment: self.source_assignment.map(|v| match v {
                SourceSpace::Rec709D65 => precision::LinearSpace::Rec709D65,
                SourceSpace::RadianceEqualEnergy => precision::LinearSpace::RadianceEqualEnergy,
            }),
            alpha_assignment: self.alpha_assignment.map(|v| match v {
                AlphaAssignment::Premultiplied => precision::AlphaAssignment::Premultiplied,
                AlphaAssignment::Straight => precision::AlphaAssignment::Straight,
            }),
        })
    }
    #[cfg(test)]
    pub fn wire(self) -> Result<[u8; wire::SETTINGS_BYTES]> {
        wire::encode_settings(self.settings()?)
    }
}
#[cfg(test)]
mod tests;
