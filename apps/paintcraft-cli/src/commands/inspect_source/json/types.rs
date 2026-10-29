//! One typed output definition drives both JSON serialization and schema.
use schemars::JsonSchema;
use serde::Serialize;
#[derive(Debug, Serialize, JsonSchema)]
pub(super) enum Version {
    #[serde(rename = "paintcraft.source-inspection/v1")]
    V1,
}
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Digest {
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Icc {
    pub payload: Digest,
    pub color_space_signature: [u8; 4],
    pub connection_space_signature: [u8; 4],
    pub tag_count: u32,
    pub interpretation: &'static str,
}
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Resolution {
    pub x: u32,
    pub y: u32,
    pub unit: &'static str,
}
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Palette {
    pub rgb: Vec<[u8; 3]>,
    pub alpha: Vec<u8>,
    pub indices_retained: bool,
}
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Png {
    pub width: u32,
    pub height: u32,
    pub component_depth: u8,
    pub components: u8,
    pub color_type: u8,
    pub sample_type: &'static str,
    pub sample_byte_order: &'static str,
    pub interlaced: bool,
    pub packed_row_bytes: Option<u64>,
    pub palette: Palette,
    pub transparency_samples: Vec<u8>,
    pub icc: Option<Icc>,
    pub resolution: Option<Resolution>,
    pub gamma_scaled: Option<u32>,
    pub chromaticities_scaled: Option<[u32; 8]>,
    pub srgb_intent: Option<u8>,
    pub cicp: Option<[u8; 4]>,
    pub mastering_display: Option<[u8; 24]>,
    pub content_light: Option<[u8; 8]>,
    pub color_declaration_precedence: &'static str,
    pub exif_historical_payload: Option<Digest>,
    pub exif_orientation: Option<u8>,
    pub significant_bits: Vec<u8>,
    pub omitted_ancillary_count: u64,
    pub omitted_ancillary_bytes: u64,
    pub omitted_ancillary_sha256: String,
    pub other_metadata: bool,
}
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Bmp {
    pub width: u32,
    pub height: u32,
    pub top_down: bool,
    pub bits_per_pixel: u16,
    pub component_depth: Option<u16>,
    pub component_depths: Vec<u8>,
    pub components: u8,
    pub alpha_declaration: &'static str,
    pub sample_type: &'static str,
    pub sample_byte_order: &'static str,
    pub dib_bytes: u32,
    pub compression: u32,
    pub row_stride: Option<u64>,
    pub declared_image_bytes: u32,
    pub pixel_data_bytes: u64,
    pub rle_stream_bytes: Option<u64>,
    pub pixel_offset: u32,
    pub masks: [u32; 4],
    pub palette: Palette,
    pub palette_reserved: Vec<u8>,
    pub palette_reserved_is_alpha: bool,
    pub declared_colors_used: u32,
    pub important_colors: u32,
    pub file_reserved: [u16; 2],
    pub resolution_raw: [i32; 2],
    pub resolution: Option<Resolution>,
    pub color_space: Option<u32>,
    pub endpoints_raw: Option<[i32; 9]>,
    pub gamma_scaled: Option<[u32; 3]>,
    pub rendering_intent: Option<u32>,
    pub icc: Option<Icc>,
    pub undeclared_bytes: u64,
}
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Component {
    pub id: u8,
    pub horizontal: u8,
    pub vertical: u8,
    pub quantization_table: u8,
}
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Density {
    pub units: u8,
    pub x: u16,
    pub y: u16,
}
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Metadata {
    pub bytes: u64,
    pub icc_bytes: u64,
    pub icc_segments: u8,
    pub icc_sha256: Option<String>,
    pub icc_interpretation: &'static str,
    pub exif_bytes: u64,
    pub xmp_bytes: u64,
    pub iptc_bytes: u64,
    pub iptc_segments: u64,
    pub comment_bytes: u64,
    pub comment_segments: u64,
    pub other_bytes: u64,
    pub other_segments: u64,
    pub jfif_thumbnail_bytes: u64,
    pub trailing_bytes: u64,
}
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Jpeg {
    pub width: u32,
    pub height: u32,
    pub component_depth: u8,
    pub components: Vec<Component>,
    pub model: &'static str,
    pub model_origin: &'static str,
    pub frame: &'static str,
    pub orientation: u8,
    pub adobe_transform: Option<u8>,
    pub density: Option<Density>,
    pub metadata: Metadata,
}
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "format", rename_all = "lowercase")]
pub(super) enum Facts {
    Png { facts: Box<Png> },
    Bmp { facts: Box<Bmp> },
    Jpeg { facts: Box<Jpeg> },
}
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Limits {
    pub input_bytes: u64,
    pub pixels: u64,
    pub metadata_bytes: u64,
    pub metadata_scope: &'static str,
}
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct SourceAttachmentPolicy {
    /// Proposed exact original bytes, never an applied working profile.
    pub icc_profile_bytes: bool,
    /// Proposed original pHYs values, never active print settings.
    pub resolution_values: bool,
    pub color_management_applied: bool,
    pub active_print_settings: bool,
    pub requires_attachment_admission: bool,
    pub requires_final_import_validation: bool,
}
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Proposal {
    pub width: u32,
    pub height: u32,
    pub output_model: &'static str,
    pub orientation_policy: &'static str,
    pub identity_profile: Option<&'static str>,
    pub source_attachment_policy: Option<SourceAttachmentPolicy>,
    pub depth_policy: &'static str,
    pub losses: Vec<String>,
    pub losses_are_header_derived: bool,
    pub requires_pixel_validation: bool,
}
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Report {
    pub schema: Version,
    pub source: Digest,
    pub inspection_only: bool,
    pub pixel_decode_performed: bool,
    pub authorizes_adoption: bool,
    pub metadata_retained_in_document: bool,
    pub color_management_performed: bool,
    pub limits: Limits,
    #[serde(flatten)]
    pub facts: Facts,
    pub proposed_conversion: Proposal,
}
