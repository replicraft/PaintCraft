//! Shared explicit DDS spellings for CLI flags and MCP nested options.
use super::{Error, ErrorCode, ImageFormat, Result};
use paintcraft_formats::dds as backend;
#[derive(Clone, Copy, Debug, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Encoding {
    Rgba8,
    Bc1,
    Bc2,
    Bc3,
}
#[derive(Clone, Copy, Debug, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Transfer {
    Srgb,
    Linear8,
}
#[derive(Clone, Copy, Debug, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Mips {
    None,
    Full,
}
#[derive(Clone, Copy, Debug, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Filter {
    Nearest,
    BoxStored,
}
#[derive(Clone, Copy, Debug, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Effort {
    Normal,
    High,
}
#[derive(Clone, Copy, Debug, Default, clap::Args, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[group(id = "DdsTextureOptions")]
pub struct Options {
    /// DDS only: fixed RGBA8 or lossy BC1/2/3. BC1 refuses nonopaque composites.
    #[arg(long = "dds-encoding", value_enum)]
    pub encoding: Option<Encoding>,
    /// DDS only: native sRGB or real quantized linear8 conversion.
    #[arg(long = "dds-transfer", value_enum)]
    pub transfer: Option<Transfer>,
    /// DDS only: one level or generate the full 2D mip chain; conflicts with levels.
    #[arg(long = "dds-mips", value_enum)]
    pub mips: Option<Mips>,
    /// DDS only: total levels 1..17 including level zero, bounded by geometry; conflicts with mips.
    #[arg(long = "dds-levels", value_parser=clap::value_parser!(u8).range(1..=17), conflicts_with="mips")]
    #[schemars(range(min = 1, max = 17))]
    pub levels: Option<u8>,
    /// DDS mip filter: stored-sample Box is alpha weighted, not sRGB linear-light filtering.
    #[arg(long = "dds-filter", value_enum)]
    pub filter: Option<Filter>,
    /// DDS BC compression effort. RGBA8 has no compression-effort control.
    #[arg(long = "dds-effort", value_enum)]
    pub effort: Option<Effort>,
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
impl Options {
    pub fn requested(&self) -> bool {
        self.encoding.is_some()
            || self.transfer.is_some()
            || self.mips.is_some()
            || self.levels.is_some()
            || self.filter.is_some()
            || self.effort.is_some()
    }
    pub fn require_format(&self, format: ImageFormat) -> Result<()> {
        if format != ImageFormat::Dds {
            return Err(invalid("DDS options require DDS output"));
        }
        Ok(())
    }
    pub fn resolve(&self, width: u32, height: u32) -> Result<backend::TextureOptions> {
        if self.levels.is_some() && self.mips.is_some() {
            return Err(invalid("DDS mips and levels are mutually exclusive"));
        }
        let encoding = match self.encoding.unwrap_or(Encoding::Rgba8) {
            Encoding::Rgba8 => backend::Encoding::Rgba8,
            Encoding::Bc1 => backend::Encoding::Bc1,
            Encoding::Bc2 => backend::Encoding::Bc2,
            Encoding::Bc3 => backend::Encoding::Bc3,
        };
        if encoding == backend::Encoding::Rgba8 && matches!(self.effort, Some(Effort::High)) {
            return Err(invalid("DDS RGBA8 does not support BC compression effort"));
        }
        let maximum = 32 - width.max(height).leading_zeros();
        let mips = if let Some(levels) = self.levels {
            if !(1..=17).contains(&levels) || u32::from(levels) > maximum {
                return Err(invalid("DDS level count exceeds the document geometry"));
            }
            backend::MipCount::Levels(levels)
        } else if matches!(self.mips, Some(Mips::Full)) {
            backend::MipCount::Full
        } else {
            backend::MipCount::Levels(1)
        };
        let generates = match mips {
            backend::MipCount::Full => maximum > 1,
            backend::MipCount::Levels(n) => n > 1,
        };
        if !generates && matches!(self.filter, Some(Filter::BoxStored)) {
            return Err(invalid(
                "DDS Box mip filtering requires generated lower levels",
            ));
        }
        Ok(backend::TextureOptions {
            encoding,
            transfer: if matches!(self.transfer, Some(Transfer::Linear8)) {
                backend::Transfer::Linear
            } else {
                backend::Transfer::Srgb
            },
            alpha: if encoding == backend::Encoding::Bc1 {
                backend::SourceAlpha::Opaque
            } else {
                backend::SourceAlpha::Straight
            },
            mips,
            filter: if matches!(self.filter, Some(Filter::BoxStored)) {
                backend::MipFilter::BoxStored
            } else {
                backend::MipFilter::Nearest
            },
            quality: if matches!(self.effort, Some(Effort::High)) {
                backend::TextureQuality::High
            } else {
                backend::TextureQuality::Normal
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schema_and_runtime_levels_agree_with_clap_bounds() {
        let schema = serde_json::to_value(schemars::schema_for!(Options)).unwrap();
        let field = &schema["properties"]["levels"];
        assert_eq!(field["minimum"], 1);
        assert_eq!(field["maximum"], 17);
        for levels in [0, 18, 255] {
            let options: Options =
                serde_json::from_value(serde_json::json!({"levels": levels})).unwrap();
            // Geometry alone would allow 18 levels here; the shared DTO must
            // still reject values outside its explicit MCP/Clap 1..17 policy.
            assert_eq!(
                options.resolve(131_072, 1).unwrap_err().code,
                ErrorCode::InputSchema
            );
        }
        assert!(
            Options {
                levels: Some(17),
                ..Options::default()
            }
            .resolve(65_536, 1)
            .is_ok()
        );
    }
    #[test]
    fn exact_mapping_defaults_and_inconsistent_options() {
        let defaults = Options::default();
        assert!(!defaults.requested());
        let value = defaults.resolve(7, 3).unwrap();
        assert_eq!(value.encoding, backend::Encoding::Rgba8);
        assert_eq!(value.transfer, backend::Transfer::Srgb);
        assert_eq!(value.mips, backend::MipCount::Levels(1));
        let explicit: Options = serde_json::from_value(serde_json::json!({
            "encoding":"bc3", "transfer":"linear8", "mips":"full",
            "filter":"box-stored", "effort":"high"}))
        .unwrap();
        let value = explicit.resolve(7, 3).unwrap();
        assert_eq!(value.encoding, backend::Encoding::Bc3);
        assert_eq!(value.transfer, backend::Transfer::Linear);
        assert_eq!(value.quality, backend::TextureQuality::High);
        assert_eq!(value.filter, backend::MipFilter::BoxStored);
        for bad in [
            serde_json::json!({"effort":"high"}),
            serde_json::json!({"levels":0}),
            serde_json::json!({"levels":4}),
            serde_json::json!({"levels":2,"mips":"full"}),
            serde_json::json!({"filter":"box-stored"}),
        ] {
            let opts: Options = serde_json::from_value(bad).unwrap();
            assert_eq!(opts.resolve(7, 3).unwrap_err().code, ErrorCode::InputSchema);
        }
        assert!(serde_json::from_value::<Options>(serde_json::json!({"quality":5})).is_err());
        assert!(explicit.require_format(ImageFormat::Png).is_err());
        assert!(explicit.require_format(ImageFormat::Dds).is_ok());
    }
}
