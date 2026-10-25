//! Shared CLI/MCP spelling for explicit PCX encoding policies.
#[derive(
    Clone, Copy, Debug, Default, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Encoding {
    #[default]
    Rgb24,
    IndexedExact,
    #[value(name = "indexed-256")]
    #[serde(rename = "indexed-256")]
    Indexed256,
}
impl From<Encoding> for paintcraft_formats::pcx::Mode {
    fn from(value: Encoding) -> Self {
        match value {
            Encoding::Rgb24 => Self::Rgb24,
            Encoding::IndexedExact => Self::IndexedExact,
            Encoding::Indexed256 => Self::Quantized256,
        }
    }
}
