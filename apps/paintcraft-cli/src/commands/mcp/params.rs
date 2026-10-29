use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Create {
    pub width: u32,
    pub height: u32,
    /// #RRGGBBAA; defaults to opaque white. Use #00000000 for transparency.
    pub background: Option<String>,
    #[serde(default)]
    pub discard_changes: bool,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Open {
    pub path: String,
    #[serde(default)]
    pub discard_changes: bool,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Import {
    pub path: String,
    /// Explicit zero-based DCX/TIFF page. Required for DCX or TIFF page conversion.
    pub page: Option<u16>,
    /// Explicit zero-based ICO/CUR/ICNS entry. Required for icon imports; conflicts with page.
    pub entry: Option<u16>,
    /// SVG/SVGZ only: explicit dimensions, optional DPI/fitting and font files.
    pub svg: Option<super::super::svg_options::Selection>,
    /// Explicitly acknowledge source information that RGBA8 cannot preserve.
    #[serde(default)]
    pub allow_losses: Vec<String>,
    #[serde(default)]
    pub discard_changes: bool,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Save {
    pub path: String,
    #[serde(default)]
    pub overwrite: bool,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SourceMetadata {
    #[default]
    Refuse,
    Omit,
}
impl From<SourceMetadata> for paintcraft_formats::export::metadata_policy::Intent {
    fn from(value: SourceMetadata) -> Self {
        match value {
            SourceMetadata::Refuse => Self::Refuse,
            SourceMetadata::Omit => Self::Omit,
        }
    }
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Export {
    /// Explicitly "omit" retained source metadata from this copy, or "refuse" (default).
    pub source_metadata: Option<SourceMetadata>,
    /// JPEG only: six RGB hexadecimal digits for the alpha backdrop; default white.
    pub jpeg_matte: Option<String>,
    /// Extension selects the image codec, for example .png or .jpg.
    pub path: String,
    /// PCX only. RGB24 is the default; indexed-256 requires consent for any changed colors.
    pub pcx_encoding: Option<super::super::pcx_options::Encoding>,
    /// DDS only: explicit texture encoding, transfer, mip generation and BC effort.
    pub dds: Option<super::super::dds_options::Options>,
    /// MSP only: v1 raw or v2 row-RLE. Omit for the unchanged default MSP1 path.
    pub msp_version: Option<super::super::msp_options::Version>,
    #[serde(default)]
    pub overwrite: bool,
    /// Explicitly acknowledged losses, such as flatten or lossy-jpeg.
    #[serde(default)]
    pub allow_losses: Vec<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    /// Engine operations from get_batch_schema. The server supplies the current base ticket.
    pub operations: Vec<serde_json::Value>,
}
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub x: i64,
    pub y: i64,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Stroke {
    pub points: Vec<Point>,
    pub color: String,
    pub size: u32,
    /// Engine brush name; defaults to smooth-round. See get_batch_schema for all brushes.
    pub brush: Option<String>,
    /// Defaults to the top layer.
    pub layer_id: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Shape {
    /// rect, ellipse, line, or a preset from get_batch_schema.
    pub kind: String,
    pub from: Point,
    pub to: Point,
    pub fill: Option<String>,
    pub stroke_color: Option<String>,
    pub stroke_size: Option<u32>,
    pub layer_id: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Navigate {
    /// false undoes; true redoes.
    #[serde(default)]
    pub redo: bool,
}
