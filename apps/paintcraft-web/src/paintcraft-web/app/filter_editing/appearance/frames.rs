//! The fixed prepared context. View changes never rebuild requirements or reset work.
use super::preparation::{Preparation7, Sealed7};
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{
    export::layer_appearance::{
        Fact7, Inventory7, Requirements7, Union7, admission_work_quote, framing_work_quote,
    },
    pcnd::layer_appearance::CandidateProvenance,
};

#[derive(Debug)]
pub(in crate::app::filter_editing) struct Frames7 {
    pub sealed: Sealed7,
    pub admission_sha256: [u8; 32],
    pub source: Fact7,
    pub candidate: Fact7,
    pub union: Union7,
    pub source_requirements: Requirements7,
    pub union_requirements: Requirements7,
}
impl Frames7 {
    /// Requirements/inventories come from the actual staged document and authentic
    /// source semantic projection. All scanner/staging work is already debited.
    /// Caller accounts this retained inline owner before constructing it.
    pub fn seal(
        prefix: Preparation7,
        source: (Requirements7, Inventory7),
        candidate: (Requirements7, Inventory7),
        union: Requirements7,
        _provenance: CandidateProvenance,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        // Exactly three final captures. No digest builder runs before sealing.
        let framing =
            [&source.0, &candidate.0, &union]
                .into_iter()
                .try_fold(0u64, |sum, requirements| {
                    sum.checked_add(framing_work_quote(requirements)?)
                        .ok_or_else(overflow)
                })?;
        // One separate cached admission SHA is needed by OPEN and echoed bindings.
        // Exact digest input bytes; this is not rebuilt on view-only generations.
        let admission_bytes = admission_work_quote();
        let sealed = prefix.seal(framing.checked_add(admission_bytes).ok_or_else(overflow)?)?;
        let admission_sha256 = sealed.admission.digest()?;
        sealed.admission.validate_requirements(&union)?;
        let source_fact = Fact7::capture(&source.0, sealed.admission, source.1, check)?;
        let candidate_fact = Fact7::capture(&candidate.0, sealed.admission, candidate.1, check)?;
        let union_fact =
            Union7::capture(&union, sealed.admission, source.1.source_coverage, check)?;
        Ok(Self {
            sealed,
            admission_sha256,
            source: source_fact,
            candidate: candidate_fact,
            union: union_fact,
            source_requirements: source.0,
            union_requirements: union,
        })
    }
}
fn overflow() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Source7 final framing work overflow",
    )
}
