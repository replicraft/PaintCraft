//! Authentic wire projection, with no decoded adoption or retained wire index.
use super::preparation::Preparation7;
use paintcraft_core::types::{Error, ErrorCode, Result, resources::BudgetScope};
use paintcraft_document::Document;
use paintcraft_formats::pcnd::{
    CheckedContainer, RequirementsLimits, layer_appearance::ProjectedCoverage7,
};

#[derive(Debug)]
pub(in crate::app::filter_editing) struct Projection7 {
    pub source_sha256: [u8; 32],
    pub coverage: ProjectedCoverage7,
    pub wire_tile_records: u64,
    pub wire_metadata_bytes: u64,
    /// Existing policy charge of the captured immutable source metadata owner.
    /// Worker adoption reconciles this allowance; no private-layout prediction.
    pub metadata_charge: u64,
}
impl Projection7 {
    /// `bytes` is the actual admitted snapshot, not a reconstructed authority.
    /// Caller owns its account throughout this function and eventual JS transfer.
    pub fn capture(
        bytes: &[u8],
        live: &Document,
        limits: RequirementsLimits,
        scope: &BudgetScope,
        prefix: &Preparation7,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if bytes.len() > 64 << 20 {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Source7 snapshot bounds",
            ));
        }
        // Two SHA-observation input slots per source byte, matching Worker7's
        // conservative intake model. The combined verifier clones hash state:
        // one body traversal, no separate whole-source digest or CPU-cost claim.
        prefix.spend((bytes.len() as u64).checked_mul(2).ok_or_else(|| {
            Error::new(
                ErrorCode::ResourceLimit,
                "Source7 authentication work overflow",
            )
        })?)?;
        let (witness, source_sha256) = CheckedContainer::verify_with_source_hash(bytes, check)?;
        if witness.version() != 7 {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Source7 needs PCND7",
            ));
        }
        let wire = witness
            .probe_layer_appearance_checked(limits, scope, check, &|units| prefix.spend(units))?;
        let coverage =
            wire.project_semantics_checked(live, scope, check, &|units| prefix.spend(units))?;
        Ok(Self {
            source_sha256,
            coverage,
            wire_tile_records: wire.wire_tile_records(),
            wire_metadata_bytes: wire.wire_metadata_bytes(),
            metadata_charge: live
                .source_metadata
                .as_ref()
                .map_or(0, |m| m.storage_usage().1),
        })
    }
}
