//! Explicit integer precision conversion, for use after strict isolated decode.
//! No color conversion, premultiplication, orientation or alpha matte is applied.
use super::*;
use sha2::{Digest, Sha256};

pub const DEPTH16_POLICY: &str = "jxl-u16-to-u8-nearest-257-v1";
pub const DEPTH16_LOSS: &str = "depth-16-reduced-to-8";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assessment16 {
    /// Original encoded source digest supplied by the source-owning caller.
    /// This primitive cannot establish that arbitrary samples came from it.
    pub source_sha256: [u8; 32],
    pub converted_sha256: [u8; 32],
    pub width: u32,
    pub height: u32,
    /// Color components not exactly representable as n * 257.
    pub changed_rgb_components: u32,
    /// Alpha components not exactly representable as n * 257.
    pub changed_alpha: u32,
}
impl Assessment16 {
    pub fn changed_samples(&self) -> u64 {
        u64::from(self.changed_rgb_components) + u64::from(self.changed_alpha)
    }

    pub fn policy(&self) -> &'static str {
        DEPTH16_POLICY
    }
    /// Mandatory even when every supplied component is exactly representable:
    /// the converted editable copy no longer retains 16-bit storage precision.
    pub fn loss(&self) -> &'static str {
        DEPTH16_LOSS
    }
}
#[derive(Debug)]
pub struct Converted16 {
    pub rgba: Vec<u8>,
    pub assessment: Assessment16,
}

/// Convert straight RGBA16 into straight RGBA8 using nearest integer /257.
/// Odd denominator means there are no half-integer ties. Alpha and hidden RGB
/// are quantized independently. `output_bytes` bounds the new logical payload;
/// the caller must separately admit retained input, decoder scratch and output
/// coexistence. This is not a process memory/RSS cap or source-profile admission.
pub fn convert_rgba16_checked(
    width: u32,
    height: u32,
    samples: &[u16],
    source_sha256: [u8; 32],
    output_bytes: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Converted16> {
    check()?;
    let count = admit_size(width as usize, height as usize, PIXELS_MAX)?;
    if samples.len() != count {
        return Err(codec(
            "JPEG XL RGBA16 sample extent disagrees with geometry",
        ));
    }
    if count > output_bytes.min(OUTPUT_BYTES_MAX) {
        return Err(resource(
            "JPEG XL converted output exceeds the byte allowance",
        ));
    }
    let mut rgba = allocated::<u8>(count)?;
    let mut digest = Sha256::new();
    let mut changed_rgb_components = 0u32;
    let mut changed_alpha = 0u32;
    for (source, target) in samples.chunks(4096).zip(rgba.chunks_mut(4096)) {
        check()?;
        for (index, (&value, output)) in source.iter().zip(target.iter_mut()).enumerate() {
            *output = ((u32::from(value) + 128) / 257) as u8;
            if u16::from(*output) * 257 != value {
                if index % 4 == 3 {
                    changed_alpha += 1;
                } else {
                    changed_rgb_components += 1;
                }
            }
        }
        digest.update(target);
    }
    check()?;
    Ok(Converted16 {
        rgba,
        assessment: Assessment16 {
            source_sha256,
            converted_sha256: digest.finalize().into(),
            width,
            height,
            changed_rgb_components,
            changed_alpha,
        },
    })
}
