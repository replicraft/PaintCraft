//! Closed assembly only: all borrowed authenticated views die before the payload.
pub(super) use super::metadata::{FreshLayerMap8, GuardedBuild8};
use super::{
    Source8Inspection, Source8InspectionRequest, Source8ReadRealms,
    assembly_ledger::{self as work, AdoptionFacts},
    entry::EntryAuth8,
    metadata,
    owner::{RetainedFactories8, SourcePayload8},
};
use paintcraft_core::types::Result;
use paintcraft_document::source_metadata::ReservedSourceMetadataReader;
use paintcraft_document::{Document, FilterInstance, FilterMask, SourceMetadata};
use paintcraft_formats::pcnd::RequirementsLimits;
use paintcraft_raster::{Admission, RasterLayer};
use std::mem::size_of;

pub(super) fn read_payload_checked(
    bytes: &[u8],
    request: &Source8InspectionRequest,
    realms: &Source8ReadRealms<'_>,
    auth: &EntryAuth8<'_>,
) -> Result<SourcePayload8> {
    let (checked, source_sha256) = super::authenticate::checked(bytes, auth)?;
    let raw = || auth.check();
    let spend = |n| auth.spend(n);
    let plan = checked.portable_source8_plan_checked(request.source, realms.p, &raw, &spend)?;
    let layout = plan.retained_source_layout_checked()?;
    auth.spend(168)?;
    let summary = *layout.summary();
    drop(layout);
    auth.spend(64)?;
    if summary
        .m_struct()
        .checked_add(summary.m_tile())
        .and_then(|n| n.checked_add(summary.m_cov()))
        .and_then(|n| n.checked_add(summary.m_icc()))
        .is_none_or(|n| n > realms.caps.m)
        || summary.a_retained() > realms.caps.a
        || summary.icc_retained() > realms.caps.i
        || summary.decoded_bytes() > realms.caps.q
        || summary
            .tiles()
            .checked_mul(65536)
            .is_none_or(|n| n > realms.caps.staged_pixels)
    {
        return Err(work::limit());
    }
    super::probe_ledger::prepay(&summary, auth)?;
    let source = checked.probe_layer_blend_appearance_checked(
        RequirementsLimits {
            layers: request.source.layers,
            tiles: request.source.tiles,
            metadata_bytes: request.source.metadata_bytes,
            coverage: request.source.coverage,
            work: request.source.work,
        },
        realms.p,
        &raw,
        &spend,
    )?;
    auth.spend(64)?;
    let mut control = realms.c.account();
    control.resize(metadata::controls()?)?;
    auth.spend(8192)?;
    auth.spend(work::retire(
        summary.layers(),
        summary.filters(),
        summary.mask_present(),
    )?)?;
    let mut m_guard = realms.m.account();
    m_guard.resize(work::sum(&[
        summary.m_struct(),
        summary.m_tile(),
        summary.m_cov(),
        summary.m_icc(),
    ])?)?;
    let factories = RetainedFactories8::capture_checked(realms, auth)?;
    let header = plan.header()?;
    let mut guard = metadata::GuardedBuild8::new(header, m_guard, factories, auth)?;
    if let Some(wire) = header.source_metadata {
        auth.spend(64)?;
        if wire.len() < 192 {
            return Err(work::invalid());
        }
        let reader = ReservedSourceMetadataReader::new(
            realms.i,
            realms.c,
            &guard.m_guard,
            request.source.metadata_bytes as u64,
            &raw,
            &spend,
        )?;
        let recipe = reader.inspect(&wire[..192], &wire[192..])?;
        guard.parts.source_metadata = Some(recipe.materialize()?);
    }
    let mut decoder = if summary.tiles() != 0 {
        auth.spend(1024)?;
        let constructor_check = || auth.step(0);
        Some(Admission::with_reserved_decode_factories(
            realms.caps.staged_pixels,
            realms.q.clone(),
            realms.raster.clone(),
            realms.c,
            &constructor_check,
        )?)
    } else {
        None
    };
    let coverage = plan.coverage_materializer(realms.a, &guard.m_guard)?;
    let mut stream = coverage.document_intentions_checked()?;
    let mut facts = AdoptionFacts::default();
    for index in 0..summary.layers() as usize {
        let (h, mut layer_stream) = stream.begin_layer()?;
        let name = metadata::string(h.name, auth)?;
        let mut filters = metadata::filters(h.filters, auth)?;
        let mut stack = metadata::StackFacts::default();
        for _ in 0..h.filters {
            // Prepaid move/store envelope before the stream produces an owned intent.
            auth.spend((4 * size_of::<FilterInstance>() + 2 * size_of::<FilterMask>()) as u64)?;
            let (next, filter) = layer_stream.next_filter()?;
            layer_stream = next;
            let filter = filter.ok_or_else(work::invalid)?;
            stack.capture(&filter, auth)?;
            filters.push(filter);
        }
        let filter_masks = stack.masks();
        let (next, mask) = layer_stream.finish_layer()?;
        stream = next;
        auth.spend(64)?;
        facts.layer(h.filters as u64, filter_masks, mask.is_some())?;
        let filters = stack.finish(filters, auth)?;
        let raster = if let Some(admission) = decoder.as_mut() {
            let mut pixels = plan.pixel_materializer(admission, realms.c, &guard.m_guard)?;
            pixels.layer(index)?
        } else {
            auth.spend(2 * size_of::<RasterLayer>() as u64)?;
            RasterLayer::new(h.width, h.height, h.default)
        };
        let layer = metadata::layer(h, name, raster, filters, mask, auth)?;
        metadata::insert(&mut guard.parts, &mut guard.fresh, layer, auth)?;
    }
    let receipt = stream.finish()?;
    drop(coverage);
    drop(decoder);
    auth.spend(2 * size_of::<Document>() as u64)?;
    let document = guard.parts.into_document();
    auth.step(0)?;
    auth.spend(64)?;
    if receipt.layers() != summary.layers()
        || receipt.filters() != summary.filters()
        || receipt.mask_present() != summary.mask_present()
        || receipt.mask_nonempty() != summary.mask_nonempty()
        || receipt.bands() != summary.bands()
        || receipt.runs() != summary.runs()
    {
        return Err(work::invalid());
    }
    if document.roots.len() as u64 != summary.layers()
        || facts.filters != summary.filters()
        || facts.masks != summary.mask_present()
    {
        return Err(work::invalid());
    }
    let adopted = super::adoption::finish(source, &document, &guard.fresh, &facts, realms.c, auth)?;
    let inspection = Source8Inspection {
        source_sha256,
        document_id: *document.document_id.as_bytes(),
        state_id: *document.state_id.as_bytes(),
        canvas: [document.canvas.0, document.canvas.1],
        layers: summary.layers(),
        tiles: summary.tiles(),
        filters: summary.filters(),
        masks_present: summary.mask_present(),
        masks_nonempty: summary.mask_nonempty(),
        bands: summary.bands(),
        runs: summary.runs(),
        metadata_kind: metadata_kind(document.source_metadata.as_ref()),
        profile_bytes: summary.profile_bytes(),
    };
    Ok(SourcePayload8 {
        _document: document,
        _adopted: adopted,
        inspection,
        _m_guard: guard.m_guard,
        _factories: guard.factories,
    })
}
fn metadata_kind(metadata: Option<&SourceMetadata>) -> u64 {
    match metadata {
        None => 0,
        Some(m) if m.profile().is_some() => 2,
        Some(_) => 1,
    }
}
