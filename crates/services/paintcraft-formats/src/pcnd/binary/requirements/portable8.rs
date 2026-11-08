//! Authenticated portable Source8 structural plan only. Never an adopted source.
use super::*;
use crate::export::prepared_capabilities::{Registration, RegistryContract, resolve_for};
use paintcraft_core::types::{Rect, Rgba, filters::Descriptor, resources::BudgetAccount};
use paintcraft_document::LayerBlendMode;
use std::cell::Cell;
mod census;
mod cursor;
mod wire;
pub use cursor::{
    PortableBlockView, PortableFilterView, PortableHeaderView, PortableLayerCursor,
    PortableLayerHeader, PortableMaskView,
};
const SOURCE_MAX: usize = 64 << 20;
const CHECK: u64 = 512;
fn invalid() -> Error {
    codec("invalid portable Source8 structure")
}
fn bound() -> Error {
    Error::new(ErrorCode::ResourceLimit, "portable Source8 plan limit")
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(bound)
}
#[derive(Clone, Copy, Debug)]
pub struct PortableSource8Limits {
    pub layers: usize,
    pub tiles: u64,
    pub metadata_bytes: usize,
    pub canvas_pixels: u64,
    pub coverage: paintcraft_raster::coverage_mask::Limits,
    /// All build and subsequent cursor helper debits share this ceiling.
    pub work: u64,
}
impl Default for PortableSource8Limits {
    fn default() -> Self {
        Self {
            layers: LAYER_COUNT_MAX,
            tiles: STAGED_PIXELS_MAX / 65536,
            metadata_bytes: DOCUMENT_MAX as usize,
            canvas_pixels: 1u64 << 40,
            coverage: Default::default(),
            work: 268435456,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PortableSource8Facts {
    pub layers: u64,
    pub tiles: u64,
    pub filters: u64,
    pub bands: u64,
    pub runs: u64,
    pub string_bytes: u64,
    pub source_metadata_bytes: u64,
    pub metadata_bytes: u64,
    pub maximum_decoder_bytes: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LayerRecord {
    id: [u8; 16],
    header_start: u64,
    header_end: u64,
    blocks_start: u32,
    blocks_len: u32,
    width: u32,
    height: u32,
    name_bytes: u32,
    filter_count: u16,
    reserved: u16,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BlockRecord {
    x: u32,
    y: u32,
    encoding: u8,
    reserved: [u8; 3],
    decoded_bytes: u32,
    encoded_start: u32,
    encoded_len: u32,
}
struct Catalog {
    entries: [Option<&'static Registration>; 256],
    used: [bool; 256],
    len: usize,
}
impl Catalog {
    fn new() -> Self {
        Self {
            entries: [None; 256],
            used: [false; 256],
            len: 0,
        }
    }
}
struct Meter<'a> {
    check: &'a dyn Fn() -> Result<()>,
    spend: &'a dyn Fn(u64) -> Result<()>,
    used: Cell<u64>,
    checks: Cell<u64>,
    limit: u64,
}
impl Meter<'_> {
    fn debit(&self, n: u64) -> Result<()> {
        let next = add(self.used.get(), n)?;
        if next > self.limit {
            return Err(bound());
        }
        (self.spend)(n)?;
        self.used.set(next);
        Ok(())
    }
    fn step(&self, n: u64) -> Result<()> {
        // Own callback slot paid once BEFORE actual check. Host supplies raw check,
        // not an adapter already billing the same 512-unit structural slot.
        self.debit(add(CHECK, n)?)?;
        self.checks.set(add(self.checks.get(), 1)?);
        (self.check)()
    }
}
/// No public fields/Clone/raw source or replacement-scope constructor.
/// Integrity and structural validity are not pixel/ICC validity or authority.
/// Raw slices cannot manufacture a plan:
/// ```compile_fail
/// use paintcraft_formats::pcnd::PortableSource8Plan;
/// fn make(raw: &[u8]) { let _ = PortableSource8Plan::new(raw); }
/// ```
/// A helper cannot replace the original scope:
/// ```compile_fail
/// use paintcraft_formats::pcnd::PortableSource8Plan;
/// use paintcraft_core::types::resources::BudgetScope;
/// fn replace(plan: &PortableSource8Plan<'_>, other: &BudgetScope) {
///     let _ = plan.header(other);
/// }
/// ```
/// The original scope must outlive the bound plan:
/// ```compile_fail
/// use paintcraft_formats::pcnd::{CheckedContainer, PortableSource8Plan};
/// use paintcraft_core::types::{Result, resources::{SharedBudget, AllocationClass}};
/// fn detach<'a>(wire: &'a CheckedContainer<'a>, check: &'a dyn Fn() -> Result<()>,
///     spend: &'a dyn Fn(u64) -> Result<()>) -> PortableSource8Plan<'a> {
///     let budget = SharedBudget::new(4 << 20);
///     let scope = budget.scope(AllocationClass::PaintMetadata);
///     wire.portable_source8_plan_checked(Default::default(), &scope, check, spend).unwrap()
/// }
/// ```
pub struct PortableSource8Plan<'a> {
    layers: Vec<LayerRecord>,
    blocks: Vec<BlockRecord>,
    catalog: Catalog,
    checked: &'a CheckedContainer<'a>,
    scope: &'a BudgetScope,
    meter: Meter<'a>,
    limits: PortableSource8Limits,
    facts: PortableSource8Facts,
    header_start: usize,
    _records: BudgetAccount,
    _control: BudgetAccount,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortableSource8ControlBytes {
    pub plan: u64,
    pub scan: u64,
    pub layer_record: u64,
    pub block_record: u64,
    /// Returned borrowed views/handles live in caller-owned ABI/stack storage.
    pub layer_cursor: u64,
    pub header_view: u64,
    pub filter_view: u64,
    pub mask_view: u64,
    pub block_view: u64,
}
pub fn portable_source8_control_bytes() -> PortableSource8ControlBytes {
    PortableSource8ControlBytes {
        plan: size_of::<PortableSource8Plan<'static>>() as u64,
        scan: census::control_bytes(),
        layer_record: size_of::<LayerRecord>() as u64,
        block_record: size_of::<BlockRecord>() as u64,
        layer_cursor: size_of::<PortableLayerCursor<'static, 'static>>() as u64,
        header_view: size_of::<PortableHeaderView<'static>>() as u64,
        filter_view: size_of::<PortableFilterView<'static>>() as u64,
        mask_view: size_of::<PortableMaskView<'static>>() as u64,
        block_view: size_of::<PortableBlockView<'static>>() as u64,
    }
}
impl<'wire> CheckedContainer<'wire> {
    pub fn portable_source8_plan_checked<'a>(
        &'a self,
        limits: PortableSource8Limits,
        scope: &'a BudgetScope,
        check: &'a dyn Fn() -> Result<()>,
        spend: &'a dyn Fn(u64) -> Result<()>,
    ) -> Result<PortableSource8Plan<'a>>
    where
        'wire: 'a,
    {
        if self.version() != 8 {
            return Err(unsupported("portable Source8 requires container8"));
        }
        if self.bytes().len() > SOURCE_MAX {
            return Err(bound());
        }
        let mut control = scope.account();
        control.resize(portable_source8_control_bytes().plan)?;
        let meter = Meter {
            check,
            spend,
            used: Cell::new(0),
            checks: Cell::new(0),
            limit: limits.work,
        };
        let mut scan_control = scope.account();
        scan_control.resize(census::control_bytes())?;
        let (first, first_header) = {
            let pass = census::scan(self, &limits, &meter, None)?;
            (pass.facts, pass.header_start)
        };
        let bytes = add(
            first
                .layers
                .checked_mul(size_of::<LayerRecord>() as u64)
                .ok_or_else(bound)?,
            first
                .tiles
                .checked_mul(size_of::<BlockRecord>() as u64)
                .ok_or_else(bound)?,
        )?;
        let mut records = scope.account();
        records.resize(bytes)?;
        let mut layers = Vec::new();
        let mut blocks = Vec::new();
        layers
            .try_reserve_exact(first.layers as usize)
            .map_err(|_| bound())?;
        blocks
            .try_reserve_exact(usize::try_from(first.tiles).map_err(|_| bound())?)
            .map_err(|_| bound())?;
        if layers.capacity() != first.layers as usize || blocks.capacity() as u64 != first.tiles {
            return Err(bound());
        }
        let second = census::scan(self, &limits, &meter, Some((&mut layers, &mut blocks)))?;
        if first != second.facts || first_header != second.header_start {
            return Err(invalid());
        }
        drop(scan_control);
        Ok(PortableSource8Plan {
            layers,
            blocks,
            catalog: second.catalog,
            checked: self,
            scope,
            meter,
            limits,
            facts: second.facts,
            header_start: second.header_start,
            _records: records,
            _control: control,
        })
    }
}
impl PortableSource8Plan<'_> {
    pub fn facts(&self) -> PortableSource8Facts {
        self.facts
    }
    pub fn work_used(&self) -> u64 {
        self.meter.used.get()
    }
    pub fn check_calls(&self) -> u64 {
        self.meter.checks.get()
    }
    pub fn document_id(&self) -> Result<uuid::Uuid> {
        Ok(self.header()?.document_id)
    }
    pub fn state_id(&self) -> Result<uuid::Uuid> {
        Ok(self.header()?.state_id)
    }
    fn helper_control(&self) -> Result<BudgetAccount> {
        let mut c = self.scope.account();
        c.resize(census::control_bytes())?;
        Ok(c)
    }
}
#[cfg(test)]
mod tests;

impl std::fmt::Debug for PortableSource8Plan<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortableSource8Plan")
            .field("facts", &self.facts)
            .field("work_used", &self.meter.used.get())
            .field("check_calls", &self.meter.checks.get())
            .finish_non_exhaustive()
    }
}
