//! PCND7 execution assessment; old requirements resolver remains version<=6.
use super::*;
use crate::export::layer_appearance::{
    self as shared, CoverageAbi, CoverageHasher, CoverageKey, CoverageRow, IdentityHasher,
    Inventory7, InventoryHasher, LayerMasks7, Requirements7,
};
use paintcraft_core::types::resources::{BudgetAccount, BudgetScope};
use paintcraft_document::Document;
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use std::mem::size_of;
mod filter_digest;
mod typed;
pub use filter_digest::filter_digest_checked;
pub(in crate::pcnd) use filter_digest::{flat_shape_scratch_quote, validate_flat_shape_checked};
mod wire;
pub use typed::{preflight_candidate_counts, preflight_duplicate_counts};
#[cfg(test)]
mod tests;
/// A pure target-ABI quote, not an allocation, counter, or capability grant.
pub fn coverage_decode_quote_checked(bands: usize, runs: usize) -> Result<u64> {
    let cap = Limits::default();
    if bands > cap.bands || runs > cap.runs {
        return Err(resource());
    }
    masks::decode_quote(bands, runs)
}
/// Derived ONLY from one already-strict typed FilterOp by the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateProvenance {
    Unchanged,
    Duplicate {
        layer: Uuid,
        source_filter: Uuid,
        new_filter: Uuid,
    },
}
#[derive(Debug)]
pub struct SourceCoverage {
    requirements: Requirements7,
    inventory: Inventory7,
    wire_tiles: u64,
    wire_metadata_bytes: u64,
}
#[derive(Clone, Debug)]
pub struct ProjectedCoverage7 {
    requirements: Requirements7,
    inventory: Inventory7,
}
#[derive(Debug)]
pub struct AdoptedCoverage7 {
    requirements: Requirements7,
    inventory: Inventory7,
}
impl SourceCoverage {
    pub fn wire_tile_records(&self) -> u64 {
        self.wire_tiles
    }
    pub fn wire_metadata_bytes(&self) -> u64 {
        self.wire_metadata_bytes
    }
    pub fn requirements(&self) -> &Requirements7 {
        &self.requirements
    }
    pub fn inventory(&self) -> Inventory7 {
        self.inventory
    }
    /// Main-side wire semantics projection. Live clipped backing may retain more
    /// storage than its canonical serialized field. This cannot mint adoption.
    pub fn project_semantics_checked(
        &self,
        base: &Document,
        scope: &BudgetScope,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<ProjectedCoverage7> {
        let mut control = scope.account();
        control.resize(CONTROL_BYTES)?;
        let checked = step(check, spend);
        let required = crate::export::prepared_capabilities::Requirements::capture(base, &checked)?;
        if required
            .pairs()
            .ne(self.requirements.requirements().pairs())
        {
            return Err(invalid());
        }
        let actual = typed::capture(
            base,
            base,
            CandidateProvenance::Unchanged,
            scope,
            check,
            spend,
            false,
        )?;
        if actual != self.inventory {
            return Err(invalid());
        }
        Ok(ProjectedCoverage7 {
            requirements: self.requirements.clone(),
            inventory: self.inventory,
        })
    }
    pub fn finish_adoption_checked(
        self,
        base: &Document,
        scope: &BudgetScope,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<AdoptedCoverage7> {
        let mut control = scope.account();
        control.resize(CONTROL_BYTES)?;
        let checked = step(check, spend);
        let required = shared::Requirements7::new(
            crate::export::prepared_capabilities::Requirements::capture(base, &checked)?,
            shared::appearance_registration(),
            &checked,
        )?;
        if required
            .requirements()
            .pairs()
            .ne(self.requirements.requirements().pairs())
        {
            return Err(invalid());
        }
        let actual = typed::capture(
            base,
            base,
            CandidateProvenance::Unchanged,
            scope,
            check,
            spend,
            true,
        )?;
        if actual != self.inventory {
            return Err(invalid());
        }
        Ok(AdoptedCoverage7 {
            requirements: self.requirements,
            inventory: self.inventory,
        })
    }
}
impl AdoptedCoverage7 {
    pub fn requirements(&self) -> &Requirements7 {
        &self.requirements
    }
    pub fn source_inventory(&self) -> Inventory7 {
        self.inventory
    }
    pub fn candidate_inventory_checked(
        &self,
        base: &Document,
        candidate: &Document,
        provenance: CandidateProvenance,
        scope: &BudgetScope,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<Inventory7> {
        // Metadata-only recapture protects against accidentally passing another
        // mutable base. This is NOT source/inner authentication or pixel hashing.
        let source = typed::capture(
            base,
            base,
            CandidateProvenance::Unchanged,
            scope,
            check,
            spend,
            true,
        )?;
        if source != self.inventory {
            return Err(invalid());
        }
        let mut actual = typed::capture(base, candidate, provenance, scope, check, spend, true)?;
        if actual.own != source.own {
            return Err(invalid());
        }
        actual.source_coverage = source.source_coverage;
        Ok(actual)
    }
}
impl ProjectedCoverage7 {
    pub fn requirements(&self) -> &Requirements7 {
        &self.requirements
    }
    pub fn source_inventory(&self) -> Inventory7 {
        self.inventory
    }
    pub fn candidate_inventory_checked(
        &self,
        base: &Document,
        candidate: &Document,
        provenance: CandidateProvenance,
        scope: &BudgetScope,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<Inventory7> {
        let source = typed::capture(
            base,
            base,
            CandidateProvenance::Unchanged,
            scope,
            check,
            spend,
            false,
        )?;
        if source != self.inventory {
            return Err(invalid());
        }
        let mut actual = typed::capture(base, candidate, provenance, scope, check, spend, false)?;
        if actual.own != source.own {
            return Err(invalid());
        }
        actual.source_coverage = source.source_coverage;
        Ok(actual)
    }
}
impl CheckedContainer<'_> {
    pub fn probe_layer_appearance_checked(
        &self,
        limits: RequirementsLimits,
        scope: &BudgetScope,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<SourceCoverage> {
        if self.version() != 7 {
            return Err(unsupported("Source7 requires PCND7"));
        }
        wire::probe(self, limits, scope, check, spend)
    }
}
fn invalid() -> Error {
    codec("Source7 adopted coverage does not match authenticated source")
}
// These charges are new Source7 source-action slots, NOT old callback changes,
// CPU cycles, or an execution grant. Full ledger must pass Root before execution.
pub const CONTROL_BYTES: u64 = (size_of::<SourceCoverage>()
    + size_of::<AdoptedCoverage7>()
    + size_of::<ProjectedCoverage7>()
    + 2 * size_of::<Requirements7>()
    + size_of::<catalog::Manifest<'static>>()
    + size_of::<CoverageHasher>()
    + size_of::<InventoryHasher>()
    + size_of::<IdentityHasher>()
    + 2 * size_of::<Totals>()
    + 2 * size_of::<Indices>()) as u64;
const STEP_WORK: u64 = 512;
const FILTER_LOOKUP_WORK: u64 = 256 * 8;
fn step<'a>(
    check: &'a dyn Fn() -> Result<()>,
    spend: &'a dyn Fn(u64) -> Result<()>,
) -> impl Fn() -> Result<()> + 'a {
    move || {
        spend(STEP_WORK)?;
        check()
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Index {
    id: [u8; 16],
    offset: usize,
}
/// Vec is always declared after its account, and drops before release on errors.
struct Indices {
    values: Vec<Index>,
    _account: BudgetAccount,
}
impl Indices {
    fn new(count: usize, scope: &BudgetScope) -> Result<Self> {
        let mut account = scope.account();
        account.resize(bytes::<Index>(count)?)?;
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| resource())?;
        account.resize(bytes::<Index>(values.capacity())?)?;
        Ok(Self {
            values,
            _account: account,
        })
    }
    fn push(&mut self, index: Index) -> Result<()> {
        if self.values.len() == self.values.capacity() {
            return Err(resource());
        }
        self.values.push(index);
        Ok(())
    }
    fn sort(
        &mut self,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<()> {
        let n = self.values.len();
        let bits = usize::BITS - n.leading_zeros();
        let slots = (n as u64)
            .checked_add(1)
            .and_then(|n| n.checked_mul(u64::from(bits) + 1))
            .and_then(|n| n.checked_mul(4 * 64))
            .ok_or_else(resource)?;
        spend(slots)?;
        // Explicit heapsort: no library-sort allocation/implementation assumption.
        fn sift(
            a: &mut [Index],
            mut root: usize,
            end: usize,
            check: &dyn Fn() -> Result<()>,
        ) -> Result<()> {
            loop {
                check()?;
                let Some(child) = root
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(1))
                    .filter(|n| *n < end)
                else {
                    break;
                };
                let child = if child + 1 < end && a[child].id < a[child + 1].id {
                    child + 1
                } else {
                    child
                };
                if a[root].id >= a[child].id {
                    break;
                }
                a.swap(root, child);
                root = child;
            }
            Ok(())
        }
        for root in (0..n / 2).rev() {
            sift(&mut self.values, root, n, check)?;
        }
        for end in (1..n).rev() {
            self.values.swap(0, end);
            sift(&mut self.values, 0, end, check)?;
        }
        for pair in self.values.windows(2) {
            check()?;
            if pair[0].id == pair[1].id {
                return Err(invalid());
            }
        }
        Ok(())
    }
}
impl Drop for Indices {
    fn drop(&mut self) {
        drop(std::mem::take(&mut self.values)); /* account still live */
    }
}
/// Caller reserves this transient inventory slice before any allocation. Actual
/// Vec capacity reconciliation may still refuse allocator overcapacity safely.
pub fn transient_quote(layers: usize) -> Result<u64> {
    if layers > LAYER_COUNT_MAX {
        return Err(resource());
    }
    CONTROL_BYTES
        .checked_mul(2)
        .and_then(|n| n.checked_add(bytes::<Index>(layers).ok()?))
        .and_then(|n| n.checked_add(bytes::<Index>(256).ok()?))
        .ok_or_else(resource)
}
fn bytes<T>(count: usize) -> Result<u64> {
    count
        .checked_mul(size_of::<T>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(resource)
}
#[derive(Default, Clone, Copy)]
struct Totals {
    occurrences: u32,
    stored: u32,
    enabled: u32,
    bands: u64,
    runs: u64,
    upper: u64,
    own_bands: u64,
    own_runs: u64,
    own_upper: u64,
}
impl Totals {
    fn row(&mut self, row: CoverageRow) -> Result<()> {
        self.occurrences = self
            .occurrences
            .checked_add(1)
            .filter(|n| *n <= shared::OCCURRENCE_MAX)
            .ok_or_else(resource)?;
        self.bands = self
            .bands
            .checked_add(row.bands)
            .filter(|n| *n <= 262_144)
            .ok_or_else(resource)?;
        self.runs = self
            .runs
            .checked_add(row.runs)
            .filter(|n| *n <= 262_144)
            .ok_or_else(resource)?;
        self.upper = self
            .upper
            .checked_add(row.decode_upper_bytes)
            .ok_or_else(resource)?;
        if row.key.kind == 1 {
            self.stored = self
                .stored
                .checked_add(1)
                .filter(|n| *n <= shared::OWN_MAX)
                .ok_or_else(resource)?;
            self.enabled = self
                .enabled
                .checked_add(u32::from(row.enabled))
                .ok_or_else(resource)?;
            self.own_bands = self.own_bands.checked_add(row.bands).ok_or_else(resource)?;
            self.own_runs = self.own_runs.checked_add(row.runs).ok_or_else(resource)?;
            self.own_upper = self
                .own_upper
                .checked_add(row.decode_upper_bytes)
                .ok_or_else(resource)?;
        }
        Ok(())
    }
    fn finish(
        self,
        abi: CoverageAbi,
        identity: [u8; 32],
        source: [u8; 32],
        inventory: [u8; 32],
    ) -> Result<Inventory7> {
        let own = LayerMasks7 {
            stored: self.stored,
            enabled: self.enabled,
            bands: self.own_bands,
            runs: self.own_runs,
            decode_upper_bytes: self.own_upper,
            identity,
            abi,
        };
        let facts = Inventory7 {
            own,
            occurrences: self.occurrences,
            bands: self.bands,
            runs: self.runs,
            decode_upper_bytes: self.upper,
            source_coverage: source,
            mask_inventory: inventory,
        };
        facts.validate()?;
        Ok(facts)
    }
}
fn mask_row(
    key: CoverageKey,
    mask: &A8Mask,
    enabled: bool,
    require_retained: bool,
) -> Result<CoverageRow> {
    let domain = mask.domain();
    let stats = mask.stats();
    let upper = coverage_decode_quote_checked(stats.bands, stats.runs)?;
    if require_retained && mask.storage_usage().map_or(0, |(_, n)| n) > upper {
        return Err(resource());
    }
    Ok(CoverageRow {
        key,
        enabled,
        domain: [domain.x0, domain.y0, domain.x1, domain.y1],
        digest: mask.digest(),
        bands: stats.bands as u64,
        runs: stats.runs as u64,
        decode_upper_bytes: upper,
    })
}
