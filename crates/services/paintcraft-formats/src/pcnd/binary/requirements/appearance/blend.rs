//! Separate PCND8 metadata anchors. No pixel, opacity or execution certificate.
use super::*;
pub use super::{
    CandidateProvenance, coverage_decode_quote_checked, preflight_candidate_counts,
    preflight_duplicate_counts,
};
use crate::export::layer_blend_appearance::{self as metadata8, Requirements8};
#[cfg(test)]
mod tests;
mod wire;

#[derive(Debug)]
pub struct SourceCoverage8 {
    requirements: Requirements8,
    inventory: Inventory7,
    layer_blend: [u8; 32],
    wire_tiles: u64,
    wire_metadata_bytes: u64,
}
#[derive(Clone, Debug)]
pub struct ProjectedCoverage8 {
    requirements: Requirements8,
    inventory: Inventory7,
    layer_blend: [u8; 32],
}
#[derive(Debug)]
pub struct AdoptedCoverage8 {
    requirements: Requirements8,
    inventory: Inventory7,
    layer_blend: [u8; 32],
}
/// Actual compiled-target controls, including reused ancestor helper controls.
/// Retained caller storage is separately owned; no index/source buffer escapes.
pub const CONTROL_BYTES: u64 = super::CONTROL_BYTES
    + (size_of::<SourceCoverage8>()
        + size_of::<ProjectedCoverage8>()
        + size_of::<AdoptedCoverage8>()
        + 2 * size_of::<Requirements8>()
        + size_of::<Sha256>()) as u64;
pub fn transient_quote(layers: usize) -> Result<u64> {
    CONTROL_BYTES
        .checked_mul(2)
        .and_then(|n| n.checked_add(bytes::<Index>(layers).ok()?))
        .and_then(|n| n.checked_add(bytes::<Index>(256).ok()?))
        .filter(|_| layers <= LAYER_COUNT_MAX)
        .ok_or_else(resource)
}
fn blend_matches(
    base: &Document,
    expected: [u8; 32],
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    if crate::pcnd::layer_blend_identity_checked(base, scope, check, spend)? != Some(expected) {
        return Err(invalid());
    }
    Ok(())
}
fn requirements_checked(base: &Document, check: &dyn Fn() -> Result<()>) -> Result<Requirements8> {
    metadata8::Requirements8::new(
        crate::export::prepared_capabilities::Requirements::capture(base, check)?,
        shared::appearance_registration(),
        metadata8::blend_registration(),
        check,
    )
}
impl SourceCoverage8 {
    pub fn requirements(&self) -> &Requirements8 {
        &self.requirements
    }
    pub fn inventory(&self) -> Inventory7 {
        self.inventory
    }
    pub fn layer_blend(&self) -> [u8; 32] {
        self.layer_blend
    }
    pub fn wire_tile_records(&self) -> u64 {
        self.wire_tiles
    }
    pub fn wire_metadata_bytes(&self) -> u64 {
        self.wire_metadata_bytes
    }
    /// Canonical metadata comparison only; clipped live A8 may exceed wire upper.
    pub fn project_semantics_checked(
        &self,
        base: &Document,
        scope: &BudgetScope,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<ProjectedCoverage8> {
        let mut controls = scope.account();
        controls.resize(CONTROL_BYTES)?;
        let checked = step(check, spend);
        let required = requirements_checked(base, &checked)?;
        if required
            .requirements()
            .pairs()
            .ne(self.requirements.requirements().pairs())
        {
            return Err(invalid());
        }
        blend_matches(base, self.layer_blend, scope, check, spend)?;
        if typed::capture(
            base,
            base,
            CandidateProvenance::Unchanged,
            scope,
            check,
            spend,
            false,
        )? != self.inventory
        {
            return Err(invalid());
        }
        Ok(ProjectedCoverage8 {
            requirements: self.requirements.clone(),
            inventory: self.inventory,
            layer_blend: self.layer_blend,
        })
    }
    /// Only fresh actual decoder output can attest retained allocation<=wire upper.
    pub fn finish_adoption_checked(
        self,
        base: &Document,
        scope: &BudgetScope,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<AdoptedCoverage8> {
        let mut controls = scope.account();
        controls.resize(CONTROL_BYTES)?;
        let checked = step(check, spend);
        let required = requirements_checked(base, &checked)?;
        if required
            .requirements()
            .pairs()
            .ne(self.requirements.requirements().pairs())
        {
            return Err(invalid());
        }
        blend_matches(base, self.layer_blend, scope, check, spend)?;
        if typed::capture(
            base,
            base,
            CandidateProvenance::Unchanged,
            scope,
            check,
            spend,
            true,
        )? != self.inventory
        {
            return Err(invalid());
        }
        Ok(AdoptedCoverage8 {
            requirements: self.requirements,
            inventory: self.inventory,
            layer_blend: self.layer_blend,
        })
    }
}
fn candidate(
    anchors: (Inventory7, [u8; 32], bool),
    docs: (&Document, &Document),
    provenance: CandidateProvenance,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<Inventory7> {
    let (inventory, blend, retained) = anchors;
    let (base, candidate) = docs;
    let mut controls = scope.account();
    controls.resize(CONTROL_BYTES)?;
    blend_matches(base, blend, scope, check, spend)?;
    blend_matches(candidate, blend, scope, check, spend)?;
    let source = typed::capture(
        base,
        base,
        CandidateProvenance::Unchanged,
        scope,
        check,
        spend,
        retained,
    )?;
    if source != inventory {
        return Err(invalid());
    }
    let mut actual = typed::capture(base, candidate, provenance, scope, check, spend, retained)?;
    if actual.own != source.own {
        return Err(invalid());
    }
    actual.source_coverage = source.source_coverage;
    Ok(actual)
}
impl ProjectedCoverage8 {
    pub fn requirements(&self) -> &Requirements8 {
        &self.requirements
    }
    pub fn source_inventory(&self) -> Inventory7 {
        self.inventory
    }
    pub fn layer_blend(&self) -> [u8; 32] {
        self.layer_blend
    }
    pub fn candidate_inventory_checked(
        &self,
        base: &Document,
        doc: &Document,
        provenance: CandidateProvenance,
        scope: &BudgetScope,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<Inventory7> {
        candidate(
            (self.inventory, self.layer_blend, false),
            (base, doc),
            provenance,
            scope,
            check,
            spend,
        )
    }
}
impl AdoptedCoverage8 {
    pub fn requirements(&self) -> &Requirements8 {
        &self.requirements
    }
    pub fn source_inventory(&self) -> Inventory7 {
        self.inventory
    }
    pub fn layer_blend(&self) -> [u8; 32] {
        self.layer_blend
    }
    pub fn candidate_inventory_checked(
        &self,
        base: &Document,
        doc: &Document,
        provenance: CandidateProvenance,
        scope: &BudgetScope,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<Inventory7> {
        candidate(
            (self.inventory, self.layer_blend, true),
            (base, doc),
            provenance,
            scope,
            check,
            spend,
        )
    }
}
impl CheckedContainer<'_> {
    pub fn probe_layer_blend_appearance_checked(
        &self,
        limits: RequirementsLimits,
        scope: &BudgetScope,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<SourceCoverage8> {
        if self.version() != 8 {
            return Err(unsupported("Source8 requires PCND8"));
        }
        wire::probe(self, limits, scope, check, spend)
    }
}
fn invalid() -> Error {
    codec("Source8 coverage/mode metadata differs from authenticated source")
}
