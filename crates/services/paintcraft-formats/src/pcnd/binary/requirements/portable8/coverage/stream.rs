//! Consuming SAME Plan whole-document read, one global coverage limit state.
use super::*;
use paintcraft_document::{FilterInstance, FilterMask, LayerMask};
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntentionReceipt8 {
    layers: u64,
    filters: u64,
    present: u64,
    nonempty: u64,
    bands: u64,
    runs: u64,
}
impl IntentionReceipt8 {
    pub fn layers(&self) -> u64 {
        self.layers
    }
    pub fn filters(&self) -> u64 {
        self.filters
    }
    pub fn mask_present(&self) -> u64 {
        self.present
    }
    pub fn mask_nonempty(&self) -> u64 {
        self.nonempty
    }
    pub fn bands(&self) -> u64 {
        self.bands
    }
    pub fn runs(&self) -> u64 {
        self.runs
    }
}
/// No Clone/raw cursor/Plan escape. Owned filter/mask returns require original
/// complete simultaneous A/M declarations through every returned alias.
/// ```compile_fail
/// use paintcraft_formats::pcnd::DocumentIntentions8;
/// fn detach(s: DocumentIntentions8<'_, '_, '_>) { let _ = s.clone(); }
/// ```
#[repr(C)]
pub struct DocumentIntentions8<'c, 'p, 'a> {
    owner: &'c PortableCoverageMaterializer8<'p, 'a>,
    remaining: paintcraft_raster::coverage_mask::Limits,
    next_layer: usize,
    filters: u64,
    present: u64,
    nonempty: u64,
    _control: BudgetAccount,
}
/// The parent moves with this cursor. Error consumes it; no poisoned-state reuse.
/// ```compile_fail
/// use paintcraft_formats::pcnd::{LayerIntentions8, PortableMaskView};
/// fn forge(s: LayerIntentions8<'_, '_, '_>, v: PortableMaskView<'_>) {
///     let _ = s.materialize(v);
/// }
/// ```
#[repr(C)]
pub struct LayerIntentions8<'c, 'p, 'a> {
    cursor: wire::Cursor<'c, 'c>,
    record: LayerRecord,
    expected: usize,
    next: usize,
    parent: DocumentIntentions8<'c, 'p, 'a>,
}
impl std::fmt::Debug for DocumentIntentions8<'_, '_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentIntentions8")
            .field("next_layer", &self.next_layer)
            .field("filters", &self.filters)
            .field("present", &self.present)
            .field("nonempty", &self.nonempty)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for LayerIntentions8<'_, '_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayerIntentions8")
            .field("layer", &self.parent.next_layer)
            .field("next_filter", &self.next)
            .field("expected", &self.expected)
            .finish_non_exhaustive()
    }
}
fn stream_control_bytes() -> Result<u64> {
    [
        size_of::<DocumentIntentions8<'static, 'static, 'static>>(),
        size_of::<LayerIntentions8<'static, 'static, 'static>>(),
        size_of::<PortableLayerHeader<'static>>(),
        size_of::<PortableFilterView<'static>>(),
        2 * size_of::<FilterInstance>(),
        size_of::<FilterMask>(),
        size_of::<LayerMask>(),
        size_of::<PortableMaskView<'static>>(),
        size_of::<wire::Cursor<'static, 'static>>(),
        size_of::<LayerRecord>(),
        size_of::<IntentionReceipt8>(),
        size_of::<paintcraft_raster::coverage_mask::Limits>(),
        2 * size_of::<paintcraft_core::types::filters::Descriptor>(),
        2 * size_of::<uuid::Uuid>(),
        size_of::<[u64; 8]>(),
        size_of::<[usize; 8]>(),
    ]
    .into_iter()
    .try_fold(0u64, |n, v| add(n, v as u64))
}
impl<'p, 'a> PortableCoverageMaterializer8<'p, 'a> {
    pub fn document_intentions_checked<'c>(&'c self) -> Result<DocumentIntentions8<'c, 'p, 'a>> {
        self.plan.meter.debit(64)?;
        let bytes = stream_control_bytes()?;
        let mut control = self.scope.account();
        control.resize(bytes)?;
        self.plan.meter.step(16)?;
        self.plan
            .meter
            .debit(size_of::<DocumentIntentions8<'static, 'static, 'static>>() as u64)?;
        Ok(DocumentIntentions8 {
            owner: self,
            remaining: self.plan.limits.coverage,
            next_layer: 0,
            filters: 0,
            present: 0,
            nonempty: 0,
            _control: control,
        })
    }
}
impl<'c, 'p, 'a> DocumentIntentions8<'c, 'p, 'a> {
    pub fn begin_layer(self) -> Result<(PortableLayerHeader<'c>, LayerIntentions8<'c, 'p, 'a>)> {
        // Cursor borrows external SAME Plan/meter, never this moved parent value.
        let owner = self.owner;
        let plan = owner.plan;
        plan.meter.step(32)?;
        let record = *plan.layers.get(self.next_layer).ok_or_else(invalid)?;
        let source = plan
            .checked
            .body()
            .get(record.header_start as usize..record.header_end as usize)
            .ok_or_else(invalid)?;
        let mut cursor = wire::Cursor::new(source, &plan.meter);
        let header = census::layer_header(&mut cursor)?;
        plan.meter.step(32)?;
        if *header.id.as_bytes() != record.id
            || header.name.len() != record.name_bytes as usize
            || header.filters != record.filter_count as usize
            || (header.width, header.height) != (record.width, record.height)
        {
            return Err(invalid());
        }
        plan.meter.debit(
            (size_of::<PortableLayerHeader<'static>>()
                + size_of::<LayerIntentions8<'static, 'static, 'static>>()) as u64,
        )?;
        Ok((
            header,
            LayerIntentions8 {
                cursor,
                record,
                expected: header.filters,
                next: 0,
                parent: self,
            },
        ))
    }
    fn account_mask(&mut self, view: &Option<PortableMaskView<'_>>) -> Result<()> {
        self.owner.plan.meter.debit(32)?;
        if let Some(v) = view {
            self.present = add(self.present, 1)?;
            if v.bands != 0 {
                self.nonempty = add(self.nonempty, 1)?;
            }
        }
        Ok(())
    }
    /// Diagnostic receipt, not coverage adoption, a reservation or source authority.
    pub fn finish(self) -> Result<IntentionReceipt8> {
        let plan = self.owner.plan;
        plan.meter.step(64)?;
        let f = &plan.facts; // Do not copy the whole facts struct for this comparison.
        let bands = usize::try_from(f.bands).map_err(|_| bound())?;
        let runs = usize::try_from(f.runs).map_err(|_| bound())?;
        let shape = add(add(f.layers, f.filters)?, add(f.bands, f.runs)?)?;
        if self.next_layer as u64 != f.layers
            || self.filters != f.filters
            || plan.limits.coverage.bands.checked_sub(bands) != Some(self.remaining.bands)
            || plan.limits.coverage.runs.checked_sub(runs) != Some(self.remaining.runs)
            || plan.limits.coverage.work.checked_sub(shape) != Some(self.remaining.work)
        {
            return Err(invalid());
        }
        plan.meter.debit(size_of::<IntentionReceipt8>() as u64)?;
        Ok(IntentionReceipt8 {
            layers: f.layers,
            filters: self.filters,
            present: self.present,
            nonempty: self.nonempty,
            bands: f.bands,
            runs: f.runs,
        })
    }
}
impl<'c, 'p, 'a> LayerIntentions8<'c, 'p, 'a> {
    pub fn next_filter(mut self) -> Result<(Self, Option<FilterInstance>)> {
        let plan = self.parent.owner.plan;
        plan.meter.step(32)?;
        if self.next == self.expected {
            plan.meter.debit(size_of::<Self>() as u64)?;
            return Ok((self, None));
        }
        let id = uuid::Uuid::from_bytes(self.cursor.id()?);
        let enabled = self.cursor.flag()?;
        let descriptor = self.cursor.descriptor_view(&plan.catalog)?;
        let view = self.cursor.mask(&mut self.parent.remaining)?;
        self.parent.account_mask(&view)?;
        let owned = self.parent.owner.materialize(view)?;
        plan.meter.debit(
            (32 + 2 * size_of::<FilterInstance>()
                + size_of::<Self>()
                + if owned.is_some() {
                    size_of::<FilterMask>()
                } else {
                    0
                }) as u64,
        )?;
        let mask = owned.map(|(enabled, coverage)| FilterMask::new(enabled, coverage));
        let instance = FilterInstance::from_owned_parts(id, enabled, descriptor, mask);
        self.next += 1;
        self.parent.filters = add(self.parent.filters, 1)?;
        Ok((self, Some(instance)))
    }
    pub fn finish_layer(mut self) -> Result<(DocumentIntentions8<'c, 'p, 'a>, Option<LayerMask>)> {
        let plan = self.parent.owner.plan;
        plan.meter.step(32)?;
        if self.next != self.expected || self.next != self.record.filter_count as usize {
            return Err(invalid());
        }
        let view = self.cursor.mask(&mut self.parent.remaining)?;
        self.parent.account_mask(&view)?;
        let owned = self.parent.owner.materialize(view)?;
        plan.meter.step(32)?;
        if self.cursor.input.remaining() != 0 {
            return Err(invalid());
        }
        plan.meter.debit(
            (32 + size_of::<DocumentIntentions8<'static, 'static, 'static>>()
                + if owned.is_some() {
                    2 * size_of::<LayerMask>()
                } else {
                    0
                }) as u64,
        )?;
        let mask = owned.map(|(enabled, coverage)| LayerMask::new(enabled, coverage));
        self.parent.next_layer += 1;
        Ok((self.parent, mask))
    }
}
#[cfg(test)]
mod tests;
