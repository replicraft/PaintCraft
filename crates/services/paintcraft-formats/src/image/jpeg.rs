//! Bounded JPEG source semantics and exact display-orientation normalization.
use paintcraft_core::types::{Error, ErrorCode, Result};
use sha2::{Digest, Sha256};

mod decode;
mod exif;
mod inspect;
mod orientation;
pub use decode::{Decoded, IDENTITY_PROFILE, decode, decode_with_limits};
pub use inspect::{inspect, inspect_with_limits};
pub use orientation::materialize_orientation;

pub const NORMALIZATION_POLICY: &str = "jpeg-orientation-materialized-v1";
pub const LOSS_METADATA: &str = "jpeg-metadata-ignored";
pub const LOSS_CMYK: &str = "jpeg-cmyk-to-rgb";
pub const LOSS_MODEL_ASSUMED: &str = "jpeg-color-model-assumed";
const INPUT_MAX: u64 = 1 << 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Gray,
    Rgb,
    YCbCr,
    Cmyk { adobe_inverted: bool },
    Ycck,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelOrigin {
    Jfif,
    Adobe,
    ComponentIds,
    ComponentCountConvention,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    Baseline,
    ExtendedSequential,
    Progressive,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Component {
    pub id: u8,
    pub horizontal: u8,
    pub vertical: u8,
    pub quantization_table: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Density {
    pub units: u8,
    pub x: u16,
    pub y: u16,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    pub bytes: u64,
    pub icc_bytes: u64,
    pub icc_segments: u8,
    pub icc_sha256: [u8; 32],
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
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub input_bytes: u64,
    pub pixels: u64,
    pub metadata_bytes: u64,
}
impl Limits {
    pub fn for_pixels(pixels: u64) -> Self {
        Self {
            input_bytes: INPUT_MAX,
            pixels: pixels.min(super::DECODE_PIXELS_MAX),
            metadata_bytes: INPUT_MAX,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    width: u32,
    height: u32,
    components: [Component; 4],
    component_count: u8,
    model: Model,
    model_origin: ModelOrigin,
    frame: Frame,
    orientation: u8,
    adobe_transform: Option<u8>,
    density: Option<Density>,
    metadata: Metadata,
    source_bytes: u64,
    source_sha256: [u8; 32],
}
impl Source {
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn oriented_dimensions(&self) -> (u32, u32) {
        if self.orientation >= 5 {
            (self.height, self.width)
        } else {
            self.dimensions()
        }
    }
    pub fn precision(&self) -> u8 {
        8
    }
    pub fn components(&self) -> &[Component] {
        &self.components[..usize::from(self.component_count)]
    }
    pub fn model(&self) -> Model {
        self.model
    }
    pub fn model_origin(&self) -> ModelOrigin {
        self.model_origin
    }
    pub fn frame(&self) -> Frame {
        self.frame
    }
    pub fn orientation(&self) -> u8 {
        self.orientation
    }
    pub fn adobe_transform(&self) -> Option<u8> {
        self.adobe_transform
    }
    pub fn density(&self) -> Option<Density> {
        self.density
    }
    pub fn metadata(&self) -> Metadata {
        self.metadata
    }
    pub fn source_bytes(&self) -> u64 {
        self.source_bytes
    }
    pub fn source_sha256(&self) -> [u8; 32] {
        self.source_sha256
    }
    pub fn losses(&self) -> Vec<String> {
        let mut losses = Vec::new();
        if self.metadata.icc_segments != 0 {
            losses.push("icc-profile-ignored".into());
        }
        if self.metadata.exif_bytes != 0 {
            losses.push("exif-metadata-ignored".into());
        }
        if self.metadata.xmp_bytes != 0
            || self.metadata.iptc_segments != 0
            || self.metadata.comment_segments != 0
            || self.metadata.other_segments != 0
            || self.metadata.jfif_thumbnail_bytes != 0
            || self.metadata.trailing_bytes != 0
            || self
                .density
                .is_some_and(|v| v.units != 0 || v.x != v.y || v.x == 0)
        {
            losses.push(LOSS_METADATA.into());
        }
        if matches!(self.model, Model::Cmyk { .. } | Model::Ycck) {
            losses.push(LOSS_CMYK.into());
        }
        if self.model
            == (Model::Cmyk {
                adobe_inverted: false,
            })
        {
            losses.push(LOSS_MODEL_ASSUMED.into());
        }
        losses
    }
}
#[derive(Debug)]
pub struct OrientedRgba {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub original_orientation: u8,
    pub normalized_orientation: u8,
    pub normalization_policy: &'static str,
}
fn malformed(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn unsupported(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
fn resource(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn hash(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<[u8; 32]> {
    let mut hash = Sha256::new();
    for chunk in bytes.chunks(65_536) {
        check()?;
        hash.update(chunk);
    }
    check()?;
    Ok(hash.finalize().into())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod decode_tests;
