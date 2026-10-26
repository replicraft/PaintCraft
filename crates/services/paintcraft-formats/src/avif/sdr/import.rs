//! Sealed source assessment for the distinct, explicitly selected SDR profile.
use super::*;
use crate::avif::import::{Image, image, image_value};
use paintcraft_core::types::{Error, ErrorCode, json::Value, policy::hex};
use sha2::{Digest, Sha256};
pub const KNOWN_IMPORT_LOSSES: &[&str] =
    &["avif-color-reconstruction", "avif-item-names-not-retained"];
pub const COLOR_POLICY: &str = "avif-explicit-bt709-srgb8-bilinear-csp12-straight-alpha-v1";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportAssessment {
    primary: Image,
    alpha: Option<Image>,
    source_len: u64,
    sha256: [u8; 32],
}
impl ImportAssessment {
    pub fn size(&self) -> (u32, u32) {
        (self.primary.sequence.width, self.primary.sequence.height)
    }
    pub fn alpha(&self) -> bool {
        self.alpha.is_some()
    }
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
    pub fn source_len(&self) -> u64 {
        self.source_len
    }
    pub fn primary_id(&self) -> u32 {
        self.primary.id
    }
    pub fn primary_name(&self) -> &str {
        &self.primary.name
    }
    pub fn primary_sequence(&self) -> &crate::avif::Sequence {
        &self.primary.sequence
    }
    pub fn alpha_id(&self) -> Option<u32> {
        self.alpha.as_ref().map(|value| value.id)
    }
    pub fn alpha_name(&self) -> Option<&str> {
        self.alpha.as_ref().map(|value| value.name.as_str())
    }
    pub fn alpha_sequence(&self) -> Option<&crate::avif::Sequence> {
        self.alpha.as_ref().map(|value| &value.sequence)
    }
    pub fn color_policy(&self) -> &'static str {
        COLOR_POLICY
    }
    pub fn losses(&self) -> &'static [&'static str] {
        if !self.primary.name.is_empty()
            || self
                .alpha
                .as_ref()
                .is_some_and(|value| !value.name.is_empty())
        {
            KNOWN_IMPORT_LOSSES
        } else {
            &KNOWN_IMPORT_LOSSES[..1]
        }
    }
    pub fn require_losses(&self, allowed: &[String]) -> Result<()> {
        if allowed
            .iter()
            .any(|code| !KNOWN_IMPORT_LOSSES.contains(&code.as_str()))
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Unknown AVIF SDR import loss consent",
            ));
        }
        if self
            .losses()
            .iter()
            .any(|loss| !allowed.iter().any(|given| given == loss))
        {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "AVIF SDR import requires consent for actual reconstruction and name losses",
            ));
        }
        Ok(())
    }
    pub fn to_value(&self) -> Value {
        let sequence = &self.primary.sequence;
        let vertical_half = u8::from(sequence.subsampling_y && sequence.chroma_position == 1);
        Value::object([
            ("source_sha256", Value::str(hex(&self.sha256))),
            ("source_bytes", Value::Int(self.source_len as i64)),
            ("width", Value::Int(i64::from(self.size().0))),
            ("height", Value::Int(i64::from(self.size().1))),
            ("primary", image_value(&self.primary)),
            (
                "alpha",
                self.alpha.as_ref().map_or(Value::Null, image_value),
            ),
            ("straight_alpha", Value::Bool(self.alpha())),
            ("color_policy", Value::str(COLOR_POLICY)),
            (
                "chroma_origin_twice_luma",
                Value::Array(vec![Value::Int(0), Value::Int(i64::from(vertical_half))]),
            ),
            (
                "reconstruction",
                Value::str(
                    "bt709-range-expansion-explicit-origin-bilinear-edge-extension-round-clip-rgb8",
                ),
            ),
            (
                "losses",
                Value::Array(self.losses().iter().copied().map(Value::str).collect()),
            ),
            ("preserving_save", Value::Bool(false)),
        ])
    }
}
pub fn assess_import(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<ImportAssessment> {
    let admitted = admit_ycbcr8(bytes, pixels, check)?;
    work_layout(&admitted, bytes.len() as u64)?;
    let primary = image(admitted.primary)?;
    let alpha = admitted.alpha.map(image).transpose()?;
    let mut hash = Sha256::new();
    for chunk in bytes.chunks(64 << 10) {
        check()?;
        hash.update(chunk);
    }
    check()?;
    Ok(ImportAssessment {
        primary,
        alpha,
        source_len: bytes.len() as u64,
        sha256: hash.finalize().into(),
    })
}

#[cfg(test)]
mod tests;
