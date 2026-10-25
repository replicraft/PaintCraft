//! JPEG XL adapter qualification boundary, separate from frontend admission.
//!
//! Integer sRGB samples retain their original precision and straight alpha.
//! Float/HDR, custom profiles, animation and non-alpha extra channels are
//! refused, rather than silently flattened or narrowed. The upstream decoder's
//! sample limit and Main5 constraints bound geometry, not total scratch/ICC
//! memory. Hostile-input process confinement remains required for qualification.
mod container;
mod decode;
mod encode;
mod execution;

pub use decode::{decode_checked, decode_with_pixel_limit, inspect_checked};
pub use encode::{encode_rgba8, encode_rgba8_checked, encode_rgba16, encode_rgba16_checked};
use paintcraft_core::types::{Error, ErrorCode, Result};

/// Conservative adapter admission, independent of the engine's larger limit.
pub const PIXELS_MAX: u64 = 4_194_304;
pub const INPUT_BYTES_MAX: usize = 64 << 20;
pub const OUTPUT_BYTES_MAX: usize = 64 << 20;
pub const METADATA_BYTES_MAX: usize = 4 << 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Samples {
    Rgba8(Vec<u8>),
    Rgba16(Vec<u16>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub samples: Samples,
    pub losses: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspection {
    pub width: u32,
    pub height: u32,
    pub bits_per_sample: u32,
    pub floating_point: bool,
    pub animation: bool,
    pub extra_channels: usize,
    pub associated_alpha: bool,
    pub custom_icc: bool,
    pub metadata_boxes: bool,
}

pub fn sniff(bytes: &[u8]) -> bool {
    matches!(
        ::jxl::api::check_signature(bytes),
        ::jxl::api::JxlSignature::Container | ::jxl::api::JxlSignature::Codestream
    )
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn codec(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
fn admit_size(width: usize, height: usize, limit: u64) -> Result<usize> {
    let pixels = width
        .checked_mul(height)
        .ok_or_else(|| resource("JPEG XL dimensions overflow"))?;
    if width == 0 || height == 0 || width > 65_536 || height > 65_536 {
        return Err(codec("JPEG XL dimensions are out of range"));
    }
    if pixels as u64 > limit.min(PIXELS_MAX) {
        return Err(resource("JPEG XL exceeds the pixel limit"));
    }
    pixels
        .checked_mul(4)
        .ok_or_else(|| resource("JPEG XL sample count overflow"))
}
fn allocated<T: Default + Clone>(len: usize) -> Result<Vec<T>> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(len)
        .map_err(|_| resource("JPEG XL output allocation failed"))?;
    result.resize(len, T::default());
    Ok(result)
}
