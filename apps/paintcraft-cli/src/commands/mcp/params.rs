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
    /// Explicit zero-based DCX page. Required for DCX; omit for other formats.
    pub page: Option<u16>,
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
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Export {
    /// Extension selects the image codec, for example .png or .jpg.
    pub path: String,
    /// PCX only. RGB24 is the default; indexed-256 requires consent for any changed colors.
    pub pcx_encoding: Option<super::super::pcx_options::Encoding>,
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
