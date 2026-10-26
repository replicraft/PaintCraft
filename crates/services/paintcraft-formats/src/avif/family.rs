//! Explicit validated source-family selection; never a decoder-error fallback.
use super::{
    admission::{self, ColorPolicy},
    *,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FamilyAssessment {
    Identity(ImportAssessment),
    Sdr(sdr::ImportAssessment),
}
impl FamilyAssessment {
    pub fn sha256(&self) -> [u8; 32] {
        match self {
            Self::Identity(v) => v.sha256(),
            Self::Sdr(v) => v.sha256(),
        }
    }
    pub fn source_len(&self) -> u64 {
        match self {
            Self::Identity(v) => v.source_len(),
            Self::Sdr(v) => v.source_len(),
        }
    }
}
/// Validate associated declarations and actual AV1 sequence once. Only an exact
/// accepted matrix selects a profile; parse, resource and cancellation failures
/// propagate without trying another profile or invoking any native decoder.
pub fn assess_family(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<FamilyAssessment> {
    let admitted = admission::admit_with_policy(bytes, pixels, check, ColorPolicy::Family)?;
    match admitted.primary.sequence.matrix {
        0 => Ok(FamilyAssessment::Identity(import::from_admitted(
            bytes, admitted, check,
        )?)),
        1 => Ok(FamilyAssessment::Sdr(sdr::import::from_admitted(
            bytes, admitted, check,
        )?)),
        _ => unreachable!("admission has resolved the qualified family matrix"),
    }
}

#[cfg(test)]
mod tests;
