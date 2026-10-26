//! Sealed source assessment for the existing isolated native import service.
//! Bounded Rust admission only; no native decoder is called by the parent.
use super::*;
use paintcraft_core::types::{json::Value, policy::hex};
use sha2::{Digest, Sha256};

pub const KNOWN_IMPORT_LOSSES: &[&str] = &["avif-item-names-not-retained"];
pub const COLOR_POLICY: &str = "avif-explicit-identity-srgb8-straight-alpha-v1";
#[derive(Clone, Debug, PartialEq, Eq)]
struct Image {
    id: u32,
    name: String,
    sequence: Sequence,
    coded_bytes: u64,
}
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
    pub fn primary_sequence(&self) -> &Sequence {
        &self.primary.sequence
    }
    pub fn alpha_id(&self) -> Option<u32> {
        self.alpha.as_ref().map(|image| image.id)
    }
    pub fn alpha_name(&self) -> Option<&str> {
        self.alpha.as_ref().map(|image| image.name.as_str())
    }
    pub fn alpha_sequence(&self) -> Option<&Sequence> {
        self.alpha.as_ref().map(|image| &image.sequence)
    }
    pub fn color_policy(&self) -> &'static str {
        COLOR_POLICY
    }
    pub fn losses(&self) -> &'static [&'static str] {
        if !self.primary.name.is_empty()
            || self
                .alpha
                .as_ref()
                .is_some_and(|image| !image.name.is_empty())
        {
            KNOWN_IMPORT_LOSSES
        } else {
            &[]
        }
    }
    pub fn require_losses(&self, allowed: &[String]) -> Result<()> {
        if allowed
            .iter()
            .any(|code| !KNOWN_IMPORT_LOSSES.contains(&code.as_str()))
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Unknown AVIF import loss consent",
            ));
        }
        if self
            .losses()
            .iter()
            .any(|loss| !allowed.iter().any(|given| given == loss))
        {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "AVIF import requires consent for reported item-name loss",
            ));
        }
        Ok(())
    }
    pub fn to_value(&self) -> Value {
        fn image(value: &Image) -> Value {
            let seq = &value.sequence;
            Value::object([
                ("item_id", Value::Int(i64::from(value.id))),
                ("item_name", Value::str(&value.name)),
                ("width", Value::Int(i64::from(seq.width))),
                ("height", Value::Int(i64::from(seq.height))),
                ("coded_bytes", Value::Int(value.coded_bytes as i64)),
                ("sequence_profile", Value::Int(i64::from(seq.profile))),
                ("sequence_level_index", Value::Int(i64::from(seq.level))),
                ("sample_depth", Value::Int(i64::from(seq.bit_depth))),
                ("monochrome", Value::Bool(seq.monochrome)),
                ("subsampling_x", Value::Bool(seq.subsampling_x)),
                ("subsampling_y", Value::Bool(seq.subsampling_y)),
                (
                    "chroma_position",
                    Value::Int(i64::from(seq.chroma_position)),
                ),
                ("primaries", Value::Int(i64::from(seq.primaries))),
                ("transfer", Value::Int(i64::from(seq.transfer))),
                ("matrix", Value::Int(i64::from(seq.matrix))),
                ("full_range", Value::Bool(seq.full_range)),
                ("still_picture", Value::Bool(true)),
                ("reduced_still_picture_header", Value::Bool(true)),
            ])
        }
        Value::object([
            ("source_sha256", Value::str(hex(&self.sha256))),
            ("source_bytes", Value::Int(self.source_len as i64)),
            ("width", Value::Int(i64::from(self.size().0))),
            ("height", Value::Int(i64::from(self.size().1))),
            ("primary", image(&self.primary)),
            ("alpha", self.alpha.as_ref().map_or(Value::Null, image)),
            ("straight_alpha", Value::Bool(self.alpha())),
            ("color_policy", Value::str(COLOR_POLICY)),
            (
                "reconstruction",
                Value::str("identity-gbr-reordering-with-independent-straight-alpha"),
            ),
            (
                "losses",
                Value::Array(self.losses().iter().copied().map(Value::str).collect()),
            ),
            ("preserving_save", Value::Bool(false)),
        ])
    }
}
fn image(value: CodedItem<'_>) -> Result<Image> {
    let mut name = String::new();
    name.try_reserve_exact(value.name.len())
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "AVIF item name allocation"))?;
    name.push_str(value.name);
    Ok(Image {
        id: value.id,
        name,
        sequence: value.sequence,
        coded_bytes: value.byte_len as u64,
    })
}
/// Constructor is private to this source-bound validation path. The sealed
/// result records exact item identities/names, coded/container-agreed sequence
/// color/geometry/alpha and whole-source SHA. The helper must still decode under
/// its independent limits and return a matching validated frame before adoption.
pub fn assess_import(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<ImportAssessment> {
    let admitted = admit_srgb8(bytes, pixels, check)?;
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
