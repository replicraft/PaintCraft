//! Caller-owned coverage admission, independent of retained original ICC bytes.
use super::*;
use paintcraft_core::types::resources::{AllocationClass, BudgetScope};
use paintcraft_raster::coverage_mask::Limits;

/// `bytes` bounds aggregate retained A8 storage plus current decode temporaries.
/// `limits` bounds aggregate bands, runs and actual checked work for the document.
/// The shared realm survives returned masks; callers share it across live documents.
#[derive(Clone, Copy, Debug)]
pub struct CoverageAdmission<'a> {
    pub bytes: u64,
    pub budget: &'a SharedBudget,
    pub limits: Limits,
}
#[derive(Clone, Copy, Debug)]
pub struct ReadAdmission<'a> {
    pub metadata: MetadataAdmission<'a>,
    pub coverage: CoverageAdmission<'a>,
}
pub(super) struct CoverageReader<'a> {
    pub admission: CoverageAdmission<'a>,
    bands: usize,
    runs: usize,
    retained: u64,
    work: std::cell::Cell<u64>,
}
impl<'a> CoverageReader<'a> {
    pub fn new(admission: CoverageAdmission<'a>) -> Self {
        Self {
            bands: admission.limits.bands,
            runs: admission.limits.runs,
            work: std::cell::Cell::new(admission.limits.work),
            retained: 0,
            admission,
        }
    }
    pub fn step(&self, check: &dyn Fn() -> Result<()>) -> Result<()> {
        check()?;
        self.work
            .set(self.work.get().checked_sub(1).ok_or_else(limit)?);
        Ok(())
    }
    pub fn counts(&mut self, bands: usize, runs: usize) -> Result<()> {
        self.bands = self.bands.checked_sub(bands).ok_or_else(limit)?;
        self.runs = self.runs.checked_sub(runs).ok_or_else(limit)?;
        Ok(())
    }
    pub fn remaining(&self) -> (usize, usize) {
        (self.bands, self.runs)
    }
    pub fn reserve(&self, peak: u64) -> Result<BudgetScope> {
        if self
            .retained
            .checked_add(peak)
            .is_none_or(|n| n > self.admission.bytes)
        {
            return Err(limit());
        }
        Ok(self
            .admission
            .budget
            .reserve_capacity(peak)?
            .scope(AllocationClass::CoverageMasks))
    }
    pub fn retain(&mut self, bytes: u64) -> Result<()> {
        self.retained = self
            .retained
            .checked_add(bytes)
            .filter(|n| *n <= self.admission.bytes)
            .ok_or_else(limit)?;
        Ok(())
    }
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "PCND aggregate coverage admission",
    )
}

/// Conservative decode facts count each serialized mask instance separately;
/// shared host storage will not remain shared after independent wire decoding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoverageFacts {
    pub masks: u64,
    pub bands: u64,
    pub runs: u64,
    /// Sum of each final payload and its parse scratch; bounds the sequential
    /// reader's actual retained-plus-current-scratch peak without deduplication.
    pub decode_bytes: u64,
}
pub fn coverage_facts(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<CoverageFacts> {
    let mut facts = CoverageFacts::default();
    for layer in doc.layers.values() {
        check()?;
        for instance in layer.filters.iter() {
            check()?;
            let Some(mask) = instance.mask() else {
                continue;
            };
            let stats = mask.coverage().stats();
            facts.masks = facts.masks.checked_add(1).ok_or_else(limit)?;
            facts.bands = facts
                .bands
                .checked_add(stats.bands as u64)
                .ok_or_else(limit)?;
            facts.runs = facts
                .runs
                .checked_add(stats.runs as u64)
                .ok_or_else(limit)?;
            facts.decode_bytes = facts
                .decode_bytes
                .checked_add(super::binary::coverage_decode_quote(
                    stats.bands,
                    stats.runs,
                )?)
                .ok_or_else(limit)?;
        }
    }
    Ok(facts)
}

/// A normal saved file must fit normal aggregate Open counts. Shared backing
/// storage still serializes once per instance; disabled/zero intent persists.
pub(super) fn validate_counts(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<()> {
    let limits = Limits::default();
    let (mut bands, mut runs) = (0usize, 0usize);
    for layer in doc.layers.values() {
        for filter in layer.filters.iter() {
            let Some(mask) = filter.mask() else { continue };
            check()?;
            let stats = mask.coverage().stats();
            bands = bands
                .checked_add(stats.bands)
                .filter(|n| *n <= limits.bands)
                .ok_or_else(limit)?;
            runs = runs
                .checked_add(stats.runs)
                .filter(|n| *n <= limits.runs)
                .ok_or_else(limit)?;
        }
    }
    Ok(())
}
