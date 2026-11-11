//! Private fresh-only assembly. Table provenance is never reconstructed from len/cap.
use super::{assembly_ledger as work, entry::EntryAuth8, owner::RetainedFactories8};
use paintcraft_core::types::{Result, resources::BudgetAccount};
use paintcraft_document::{
    Document, FilterInstance, FilterMask, FilterStack, Layer, LayerMask, SourceMetadata, StyleStack,
};
use paintcraft_formats::pcnd::{IntentionReceipt8, PortableHeaderView, PortableLayerHeader};
use paintcraft_raster::{RasterLayer, raster::metadata_clone::fresh_map_layout};
use std::{
    collections::{HashMap, hash_map::Entry},
    mem::size_of,
};
use uuid::Uuid;

#[repr(C)]
pub(super) struct PartialDoc8 {
    pub document_id: Uuid,
    pub state_id: Uuid,
    pub canvas: (u32, u32),
    pub roots: Vec<Uuid>,
    pub layers: HashMap<Uuid, Layer>,
    pub title: Option<String>,
    pub source_metadata: Option<SourceMetadata>,
}
impl PartialDoc8 {
    pub fn into_document(self) -> Document {
        Document {
            document_id: self.document_id,
            state_id: self.state_id,
            canvas: self.canvas,
            roots: self.roots,
            layers: self.layers,
            title: self.title,
            source_metadata: self.source_metadata,
        }
    }
}
#[repr(C)]
pub(super) struct FreshLayerMap8 {
    pub entries: usize,
    pub buckets: usize,
    pub capacity: usize,
    pub written: usize,
    pub allocation_bytes: u64,
    pub retirement_paid: bool,
}
#[repr(C)]
pub(super) struct GuardedBuild8 {
    pub parts: PartialDoc8,
    pub fresh: FreshLayerMap8,
    pub m_guard: BudgetAccount,
    pub factories: RetainedFactories8,
}
pub(super) fn controls() -> Result<u64> {
    if size_of::<Entry<'static, Uuid, Layer>>() > 40
        || size_of::<Option<Layer>>() > size_of::<Layer>() + 8
        || size_of::<Option<LayerMask>>() > size_of::<LayerMask>() + 8
        || size_of::<Option<SourceMetadata>>() > if usize::BITS == 64 { 16 } else { 8 }
    {
        return Err(work::limit());
    }
    let bytes: usize = if usize::BITS == 64 {
        2696
    } else if usize::BITS == 32 {
        2308
    } else {
        return Err(work::limit());
    };
    // The containing Layer adds its empty stack, plus four separately named
    // empty-stack formation/move controls. The original4096 slot is unchanged.
    let old_layer = if usize::BITS == 64 { 352 } else { 304 };
    let bytes = bytes
        .checked_add(size_of::<Layer>())
        .and_then(|n| n.checked_sub(old_layer))
        .and_then(|n| n.checked_add(4 * size_of::<StyleStack>()))
        .ok_or_else(work::limit)?;
    // Defining fields are independently checked, never enlarged from runtime demand.
    if size_of::<Document>() != if usize::BITS == 64 { 144 } else { 104 }
        || size_of::<Layer>() != if usize::BITS == 64 { 408 } else { 344 }
        || size_of::<StyleStack>() != if usize::BITS == 64 { 56 } else { 44 }
        || size_of::<FilterInstance>() != if usize::BITS == 64 { 184 } else { 176 }
        || size_of::<FilterMask>() != if usize::BITS == 64 { 152 } else { 144 }
        || size_of::<FilterStack>() != if usize::BITS == 64 { 56 } else { 44 }
        || bytes > 4096
    {
        return Err(work::limit());
    }
    let extra = size_of::<PortableHeaderView<'static>>()
        + size_of::<PortableLayerHeader<'static>>()
        + size_of::<StackFacts>()
        + size_of::<work::AdoptionFacts>()
        + size_of::<IntentionReceipt8>();
    if bytes + extra > 4096 {
        return Err(work::limit());
    }
    Ok(4096)
}
pub(super) fn string(value: &str, auth: &EntryAuth8<'_>) -> Result<String> {
    auth.spend(64)?;
    auth.spend(work::mul(value.len() as u64, 2)?)?;
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| work::limit())?;
    if result.capacity() != value.len() {
        return Err(work::limit());
    }
    result.push_str(value);
    Ok(result)
}
impl GuardedBuild8 {
    pub fn new(
        header: PortableHeaderView<'_>,
        m_guard: BudgetAccount,
        factories: RetainedFactories8,
        auth: &EntryAuth8<'_>,
    ) -> Result<Self> {
        auth.spend(64)?;
        let root_bytes = work::mul(header.layers as u64, 16)?;
        auth.spend(root_bytes)?;
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(header.layers)
            .map_err(|_| work::limit())?;
        if roots.capacity() != header.layers {
            return Err(work::limit());
        }
        auth.spend(64)?;
        let layout = fresh_map_layout::<Uuid, Layer>(header.layers)?;
        let buckets = work::buckets(header.layers as u64)? as usize;
        auth.spend(layout.bytes)?;
        let mut layers = HashMap::new();
        layers
            .try_reserve(header.layers)
            .map_err(|_| work::limit())?;
        if layers.capacity() != layout.capacity {
            return Err(work::limit());
        }
        let title = header.title.map(|s| string(s, auth)).transpose()?;
        Ok(Self {
            parts: PartialDoc8 {
                document_id: header.document_id,
                state_id: header.state_id,
                canvas: header.canvas,
                roots,
                layers,
                title,
                source_metadata: None,
            },
            fresh: FreshLayerMap8 {
                entries: header.layers,
                buckets,
                capacity: layout.capacity,
                written: 0,
                allocation_bytes: layout.bytes,
                retirement_paid: false,
            },
            m_guard,
            factories,
        })
    }
}
pub(super) fn insert(
    parts: &mut PartialDoc8,
    fresh: &mut FreshLayerMap8,
    layer: Layer,
    auth: &EntryAuth8<'_>,
) -> Result<()> {
    if !fresh.retirement_paid {
        auth.spend(fresh.buckets as u64)?;
        fresh.retirement_paid = true;
    }
    auth.spend(64)?;
    auth.spend(work::sum(&[
        65,
        work::mul(2, fresh.buckets as u64)?,
        work::mul(18, fresh.written as u64)?,
        2 * size_of::<(Uuid, Layer)>() as u64,
    ])?)?;
    if (fresh.entries != 0 && fresh.allocation_bytes == 0)
        || fresh.written >= fresh.entries
        || parts.layers.len() != fresh.written
        || parts.layers.capacity() != fresh.capacity
    {
        return Err(work::invalid());
    }
    let id = layer.id;
    match parts.layers.entry(id) {
        Entry::Vacant(slot) => {
            slot.insert(layer);
        }
        Entry::Occupied(_) => return Err(work::invalid()),
    }
    parts.roots.push(id);
    fresh.written += 1;
    Ok(())
}

#[derive(Default)]
pub(super) struct StackFacts {
    n: u64,
    ids: u64,
    params: u64,
    masks: u64,
    catalog: bool,
}
impl StackFacts {
    pub fn capture(&mut self, filter: &FilterInstance, auth: &EntryAuth8<'_>) -> Result<()> {
        auth.spend(128)?;
        let d = filter.descriptor();
        self.n = work::add(self.n, 1)?;
        self.ids = work::add(self.ids, d.id().len() as u64)?;
        self.params = work::add(self.params, d.semantic_parameter_bytes().1 as u64)?;
        self.catalog |= d.is_catalog_only();
        self.masks = work::add(self.masks, u64::from(filter.mask().is_some()))?;
        Ok(())
    }
    pub fn masks(&self) -> u64 {
        self.masks
    }
    pub fn finish(
        self,
        filters: Vec<FilterInstance>,
        auth: &EntryAuth8<'_>,
    ) -> Result<FilterStack> {
        auth.spend(64)?;
        let h = work::sum(&[
            b"paintcraft.filter-stack/v1\0".len() as u64 + 2,
            work::mul(19, self.n)?,
            self.ids,
            self.params,
            if self.catalog {
                work::mul(4, self.n)?
            } else {
                0
            },
            if self.catalog || self.masks != 0 {
                self.n
            } else {
                0
            },
            work::mul(33, self.masks)?,
        ])?;
        auth.spend(
            work::stack(self.n, h)?
                .checked_sub(64)
                .ok_or_else(work::limit)?,
        )?;
        FilterStack::new(filters)
    }
}
pub(super) fn filters(n: usize, auth: &EntryAuth8<'_>) -> Result<Vec<FilterInstance>> {
    auth.spend(64)?;
    if n > 256 {
        return Err(work::limit());
    }
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| work::limit())?;
    if v.capacity() != n {
        return Err(work::limit());
    }
    Ok(v)
}
pub(super) fn layer(
    header: PortableLayerHeader<'_>,
    name: String,
    raster: RasterLayer,
    filters: FilterStack,
    mask: Option<LayerMask>,
    auth: &EntryAuth8<'_>,
) -> Result<Layer> {
    auth.spend(3 * size_of::<Layer>() as u64)?;
    Ok(Layer {
        id: header.id,
        name,
        origin: header.origin,
        visible: header.visible,
        opacity: header.opacity,
        blend_mode: header.blend_mode,
        raster,
        filters,
        styles: paintcraft_document::StyleStack::empty(),
        mask,
    })
}
