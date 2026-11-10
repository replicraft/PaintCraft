//! Read-only SAME Plan layout recipe. No source materialization or grants.
use super::*;
use paintcraft_core::types::resources::BudgetLease;
use paintcraft_document::{Document, FilterInstance, Layer, SourceMetadata};
use paintcraft_raster::{
    RasterLayer, ReservedDecodeAdmission, ReservedDecodeControlQuote, RetainedTileTableQuote,
    coverage_mask,
    raster::metadata_clone::{FreshMapLayout, fresh_map_layout},
};

/// Diagnostic decomposition only. Copying counts cannot construct a recipe.
/// ```compile_fail
/// use paintcraft_formats::pcnd::RetainedSourceLayoutSummary8;
/// let forged = RetainedSourceLayoutSummary8 { layers: 1 };
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedSourceLayoutSummary8 {
    layers: u64,
    tiles: u64,
    filters: u64,
    mask_present: u64,
    mask_nonempty: u64,
    bands: u64,
    runs: u64,
    string_bytes: u64,
    metadata_bytes: u64,
    attachment_count: u64,
    profile_bytes: u64,
    scratch_bytes: u64,
    m_struct: u64,
    m_tile: u64,
    m_cov: u64,
    m_icc: u64,
    a_retained: u64,
    icc_retained: u64,
    decoded_bytes: u64,
    q_fixed: u64,
    doc_inline: u64,
}
macro_rules! getters {
    ($($field:ident),* $(,)?) => {$(
        pub fn $field(&self) -> u64 { self.$field }
    )*};
}
impl RetainedSourceLayoutSummary8 {
    getters!(
        layers,
        tiles,
        filters,
        mask_present,
        mask_nonempty,
        bands,
        runs,
        string_bytes,
        metadata_bytes,
        attachment_count,
        profile_bytes,
        scratch_bytes,
        m_struct,
        m_tile,
        m_cov,
        m_icc,
        a_retained,
        icc_retained,
        decoded_bytes,
        q_fixed,
        doc_inline,
    );
    fn zero() -> Self {
        Self {
            layers: 0,
            tiles: 0,
            filters: 0,
            mask_present: 0,
            mask_nonempty: 0,
            bands: 0,
            runs: 0,
            string_bytes: 0,
            metadata_bytes: 0,
            attachment_count: 0,
            profile_bytes: 0,
            scratch_bytes: 0,
            m_struct: 0,
            m_tile: 0,
            m_cov: 0,
            m_icc: 0,
            a_retained: 0,
            icc_retained: 0,
            decoded_bytes: 0,
            q_fixed: 0,
            doc_inline: 0,
        }
    }
}
/// Opaque, non-Clone SAME Plan borrow. No raw source/domain/account escape.
/// This cannot fund/adopt a source or replace original read realms.
/// ```compile_fail
/// use paintcraft_formats::pcnd::{PortableSource8Plan, PortableMaskView};
/// fn forge(plan: &PortableSource8Plan<'_>, view: PortableMaskView<'_>) {
///     let _ = plan.retained_source_layout_checked(view);
/// }
/// ```
#[repr(C)]
pub struct PortableRetainedSourceLayout8<'p, 'a> {
    _plan: &'p PortableSource8Plan<'a>,
    summary: RetainedSourceLayoutSummary8,
    _control: BudgetAccount,
}
impl PortableRetainedSourceLayout8<'_, '_> {
    pub fn summary(&self) -> &RetainedSourceLayoutSummary8 {
        &self.summary
    }
}
impl std::fmt::Debug for PortableRetainedSourceLayout8<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortableRetainedSourceLayout8")
            .field("layers", &self.summary.layers)
            .field("tiles", &self.summary.tiles)
            .field("masks", &self.summary.mask_present)
            .finish_non_exhaustive()
    }
}
fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(bound)
}
pub(super) fn control_bytes() -> Result<u64> {
    let bytes = [
        size_of::<PortableRetainedSourceLayout8<'static, 'static>>(),
        size_of::<RetainedSourceLayoutSummary8>(),
        size_of::<PortableHeaderView<'static>>(),
        size_of::<PortableLayerHeader<'static>>(),
        size_of::<PortableMaskView<'static>>(),
        size_of::<PortableBlockView<'static>>(),
        size_of::<wire::Cursor<'static, 'static>>(),
        size_of::<FreshMapLayout>(),
        size_of::<RetainedTileTableQuote>(),
        size_of::<ReservedDecodeControlQuote>(),
        size_of::<paintcraft_document::source_metadata::ReservedSourceMetadataLayout>(),
        size_of::<coverage_mask::Limits>(),
        size_of::<LayerRecord>(),
        size_of::<Descriptor>(),
        size_of::<BudgetAccount>(),
        size_of::<[u64; 16]>(),
    ]
    .into_iter()
    .try_fold(0u64, |n, v| add(n, v as u64))?;
    // Named byte declaration, not work credit or a containing source lease.
    let expected = if usize::BITS == 64 { 1040 } else { 936 };
    if !matches!(usize::BITS, 32 | 64) || bytes != expected {
        return Err(bound());
    }
    Ok(bytes)
}
impl<'a> PortableSource8Plan<'a> {
    /// Paid linear layout-only pass. No caller views/counters/realms accepted.
    /// All defining quotes and replays use the original bound Plan meter.
    /// T0 intentionally quotes no decoder: a later reader MUST skip its creation.
    pub fn retained_source_layout_checked<'p>(
        &'p self,
    ) -> Result<PortableRetainedSourceLayout8<'p, 'a>> {
        self.meter.debit(64)?;
        let bytes = control_bytes()?;
        let mut control = self.scope.account();
        control.resize(bytes)?;
        self.meter.step(16)?;
        self.meter.debit(168)?;
        let mut s = RetainedSourceLayoutSummary8::zero();
        let header = self.header()?;
        self.meter.debit(64)?;
        s.layers = self.layers.len() as u64;
        s.doc_inline = size_of::<Document>() as u64;
        s.string_bytes = header.title.map_or(0, |v| v.len() as u64);
        s.m_struct = add(
            fresh_map_layout::<uuid::Uuid, Layer>(self.layers.len())?.bytes,
            add(mul(s.layers, 16)?, s.string_bytes)?,
        )?;
        if let Some(metadata) = header.source_metadata {
            self.meter.debit(64)?;
            let q = SourceMetadata::reserved_layout_quote()?;
            s.attachment_count = 1;
            s.profile_bytes = (metadata.len() as u64)
                .checked_sub(192)
                .ok_or_else(invalid)?;
            s.m_icc = add(q.inner_allocation_upper, q.lease_control_allocation_upper)?;
            self.meter.debit(64)?;
            s.icc_retained = SourceMetadata::reserved_retained_quote(s.profile_bytes)?;
        }
        // One GLOBAL remaining coverage declaration, matching original R1 scan.
        let mut remaining = self.limits.coverage;
        let mut encoded_payload = 0u64;
        for i in 0..self.layers.len() {
            self.meter.debit(64)?;
            self.meter.step(16)?;
            let record = *self.layers.get(i).ok_or_else(invalid)?;
            let source = self
                .checked
                .body()
                .get(record.header_start as usize..record.header_end as usize)
                .ok_or_else(invalid)?;
            let mut c = wire::Cursor::new(source, &self.meter);
            let h = census::layer_header(&mut c)?;
            s.string_bytes = add(s.string_bytes, h.name.len() as u64)?;
            s.filters = add(s.filters, h.filters as u64)?;
            s.m_struct = add(s.m_struct, h.name.len() as u64)?;
            s.m_struct = add(
                s.m_struct,
                mul(h.filters as u64, size_of::<FilterInstance>() as u64)?,
            )?;
            self.meter.debit(64)?;
            let index = RasterLayer::fresh_decode_index_quote(record.blocks_len as usize)?;
            s.m_struct = add(s.m_struct, index.allocation_upper)?;
            for _ in 0..h.filters {
                self.meter.debit(64)?;
                c.id()?;
                c.flag()?;
                c.descriptor_view(&self.catalog)?;
                self.layout_mask(&mut s, c.mask(&mut remaining)?)?;
            }
            self.meter.debit(64)?;
            self.layout_mask(&mut s, c.mask(&mut remaining)?)?;
            self.meter.step(32)?;
            if c.input.remaining() != 0
                || *h.id.as_bytes() != record.id
                || h.name.len() != record.name_bytes as usize
                || h.filters != record.filter_count as usize
                || (h.width, h.height) != (record.width, record.height)
            {
                return Err(invalid());
            }
            for block in 0..record.blocks_len as usize {
                self.meter.debit(64)?;
                let b = self.block(i, block)?;
                // The prepaid outer64 covers this replay length/prefix4 read and max arithmetic.
                encoded_payload = add(encoded_payload, b.stored_bytes().len() as u64)?;
                let d = u64::from(b.decoded_bytes);
                if d == 0 || d > 262144 || !d.is_multiple_of(4) {
                    return Err(invalid());
                }
                let scratch = match b.encoding {
                    1 => 0,
                    2 | 4 => d,
                    5 => {
                        let stored = b.stored_bytes();
                        let prefix: [u8; 4] = stored
                            .get(..4)
                            .ok_or_else(invalid)?
                            .try_into()
                            .map_err(|_| invalid())?;
                        let r = u64::from(u32::from_le_bytes(prefix));
                        if stored.len() <= 4 || r == 0 || !r.is_multiple_of(6) || r > mul(6, d / 4)?
                        {
                            return Err(invalid());
                        }
                        add(r, d)?
                    }
                    _ => return Err(invalid()),
                };
                s.scratch_bytes = s.scratch_bytes.max(scratch);
                s.tiles = add(s.tiles, 1)?;
            }
        }
        if s.tiles != 0 {
            self.meter.debit(64)?;
            let q = ReservedDecodeAdmission::control_quote()?;
            self.meter.debit(64)?;
            let charge = BudgetLease::control_allocation_quote()?;
            s.q_fixed = q.fixed_bytes;
            s.m_tile = mul(s.tiles, add(q.tile_header_bytes, charge)?)?;
        }
        self.meter.step(32)?;
        s.decoded_bytes = mul(s.tiles, 262144)?;
        s.metadata_bytes = add(self.checked.body().len() as u64, MAGIC.len() as u64)?
            .checked_sub(encoded_payload)
            .ok_or_else(invalid)?;
        let f = &self.facts;
        let mask_steps = add(add(s.layers, s.filters)?, add(s.bands, s.runs)?)?;
        if s.layers != header.layers as u64
            || s.layers != f.layers
            || s.tiles != f.tiles
            || s.filters != f.filters
            || s.bands != f.bands
            || s.runs != f.runs
            || s.string_bytes != f.string_bytes
            || s.metadata_bytes != f.metadata_bytes
            || header.source_metadata.map_or(0, |v| v.len() as u64) != f.source_metadata_bytes
            || s.scratch_bytes != f.maximum_decoder_bytes
            || self.limits.coverage.bands.checked_sub(s.bands as usize) != Some(remaining.bands)
            || self.limits.coverage.runs.checked_sub(s.runs as usize) != Some(remaining.runs)
            || self.limits.coverage.work.checked_sub(mask_steps) != Some(remaining.work)
        {
            return Err(invalid());
        }
        self.meter.debit(216)?;
        Ok(PortableRetainedSourceLayout8 {
            _plan: self,
            summary: s,
            _control: control,
        })
    }
    fn layout_mask(
        &self,
        s: &mut RetainedSourceLayoutSummary8,
        mask: Option<PortableMaskView<'_>>,
    ) -> Result<()> {
        if let Some(m) = mask {
            self.meter.debit(64)?;
            let retained = coverage_mask::retained_quote(
                usize::try_from(m.bands()).map_err(|_| bound())?,
                usize::try_from(m.runs()).map_err(|_| bound())?,
            )?;
            s.mask_present = add(s.mask_present, 1)?;
            s.bands = add(s.bands, m.bands())?;
            s.runs = add(s.runs, m.runs())?;
            s.a_retained = add(s.a_retained, retained)?;
            if m.bands() != 0 {
                self.meter.debit(64)?;
                let full = coverage_mask::canonical_a8_storage_arc_quote()?;
                s.mask_nonempty = add(s.mask_nonempty, 1)?;
                s.m_cov = add(s.m_cov, full)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
