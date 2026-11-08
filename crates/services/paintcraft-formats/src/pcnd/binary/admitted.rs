//! Admitted portable PCND snapshot writer; legacy writer bodies are untouched.
use super::*;
use paintcraft_core::types::resources::BudgetScope;
use paintcraft_raster::{TileCoord, TileLease};
use std::{cell::RefCell, mem::size_of};

fn write_encoded_admitted(
    doc: &Document,
    mut output: Output<impl Write>,
    check: &dyn Fn() -> Result<()>,
    scope: &BudgetScope,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    check()?;
    crate::pcnd::layer_appearance::validate_flat_shape_checked(doc, scope, check, spend)?;
    // Explicit bounded host eligibility: paid once at snapshot freeze, not per tile.
    spend(doc.layers.len() as u64)?;
    doc.require_normal_blends()?;
    if [doc.canvas.0, doc.canvas.1]
        .iter()
        .any(|value| !(1..=1_048_576).contains(value))
    {
        return Err(codec("invalid canvas dimensions"));
    }
    super::super::coverage::validate_counts(doc, check)?;
    let mut magic = *MAGIC;
    let layer_masks = doc.has_layer_masks();
    let live_filters = layer_masks || doc.layers.values().any(|layer| !layer.filters.is_empty());
    let masked = doc
        .layers
        .values()
        .any(|layer| layer.filters.iter().any(|filter| filter.mask().is_some()));
    let spatial = has_spatial(doc);
    let catalog_only = has_catalog(doc);
    let manifest = if layer_masks {
        Some(catalog::Manifest::capture_v7(doc, check)?)
    } else if catalog_only {
        Some(catalog::Manifest::capture(doc, check)?)
    } else {
        None
    };
    if layer_masks {
        magic[4] = 7;
    } else if catalog_only {
        magic[4] = 6;
    } else if spatial {
        magic[4] = 5;
    } else if masked {
        magic[4] = 4;
    } else if live_filters {
        magic[4] = 3;
    } else if doc.source_metadata.is_some() {
        magic[4] = 2;
    }
    output.bytes(&magic)?;
    if let Some(manifest) = &manifest {
        manifest.write(&mut output, check)?;
    }
    output.bytes(doc.document_id.as_bytes())?;
    output.bytes(doc.state_id.as_bytes())?;
    output.number(u64::from(doc.canvas.0))?;
    output.number(u64::from(doc.canvas.1))?;
    match &doc.title {
        None => output.bytes(&[0])?,
        Some(title) => {
            if title.chars().count() > 4096 {
                return Err(codec("title exceeds 4096 characters"));
            }
            output.bytes(&[1])?;
            output.string(title)?;
        }
    }
    if live_filters {
        output.bytes(&[u8::from(doc.source_metadata.is_some())])?;
    }
    if let Some(source) = &doc.source_metadata {
        source_metadata::write(source, &mut output, check)?;
    }
    if doc.roots.len() > LAYER_COUNT_MAX || doc.roots.len() != doc.layers.len() {
        return Err(codec("invalid flat document roots"));
    }
    output.number(doc.roots.len() as u64)?;
    let mask_work = std::cell::Cell::new(paintcraft_raster::coverage_mask::Limits::default().work);
    let mask_check = || {
        check()?;
        mask_work.set(mask_work.get().checked_sub(1).ok_or_else(|| {
            Error::new(ErrorCode::ResourceLimit, "PCND aggregate mask writer work")
        })?);
        Ok(())
    };
    for id in &doc.roots {
        check()?;
        let layer = doc
            .layers
            .get(id)
            .filter(|layer| layer.id == *id)
            .ok_or_else(|| codec("invalid flat document roots"))?;
        if layer.name.chars().count() > 1024 {
            return Err(codec("layer name exceeds 1024 characters"));
        }
        if [layer.raster.width(), layer.raster.height()]
            .iter()
            .any(|value| !(1..=1_048_576).contains(value))
        {
            return Err(codec("invalid layer dimensions"));
        }
        output.bytes(id.as_bytes())?;
        output.string(&layer.name)?;
        output.bytes(&[u8::from(layer.visible), layer.opacity])?;
        output.signed(layer.origin.0)?;
        output.signed(layer.origin.1)?;
        output.number(u64::from(layer.raster.width()))?;
        output.number(u64::from(layer.raster.height()))?;
        output.bytes(&layer.raster.default_pixel().0)?;
        if live_filters {
            if let Some(manifest) = &manifest {
                catalog::write_stack(manifest, &layer.filters, &mut output, check, &mask_check)?;
            } else {
                filters::write(&layer.filters, &mut output, magic[4], check, &mask_check)?;
            }
        }
        if layer_masks {
            masks::write_layer(layer.mask.as_ref(), &mut output, &mask_check)?;
        }
        // A terminator avoids a counting pass over decoded tiles. Each record is
        // bounded independently, allowing the sink to apply backpressure.
        blocks(&layer.raster, scope, check, spend, &mut |bx, by, data| {
            let stored = compression::admitted::block(data, scope, check, spend)?;
            output.bytes(&[match stored.encoding() {
                "raw" => 1,
                "rgba-runs" => 2,
                "lz4" => 4,
                "runs-lz4" => 5,
                _ => unreachable!(),
            }])?;
            output.number(u64::from(bx))?;
            output.number(u64::from(by))?;
            output.number(stored.bytes().len() as u64)?;
            output.payload(stored.bytes())
        })?;
        output.bytes(&[0])?;
    }
    check()?;
    output.finish()
}

/// Upper for simultaneous portable compressor payloads + coordinate/lease index.
/// Output Vec and retained Main controls are separate caller declarations.
/// Constants derive from one256²RGBA tile, six bytes per RLE run, LZ4 maximum
/// output API, and pinned4096*u32 table payload. No native Zstandard instance.
pub fn portable_snapshot_scratch_quote(doc: &Document) -> Result<u64> {
    let tile = 256usize * 256 * 4;
    let runs = tile / 4 * 6;
    let packed_runs = 4 + lz4_flex::block::get_maximum_output_size(runs);
    let packed_raw = lz4_flex::block::get_maximum_output_size(tile);
    let max_tiles = doc
        .layers
        .values()
        .map(|l| l.raster.resident_tiles())
        .max()
        .unwrap_or(0);
    let index = max_tiles
        .checked_mul(size_of::<(TileCoord, TileLease)>())
        .ok_or_else(limit)?;
    // The shared root validator frees its own index/controls before tile phase.
    let root_phase = crate::pcnd::layer_appearance::flat_shape_scratch_quote(doc.roots.len())?;
    let tile_phase = [tile, runs, packed_runs, packed_raw, index, 4096 * 4]
        .into_iter()
        .try_fold(0u64, |n, v| n.checked_add(v as u64).ok_or_else(limit))?;
    Ok(root_phase.max(tile_phase))
}
/// Caller has reserved the scratch quote and output/control bytes in ONE scope.
/// Model units: actual writer callback1, output input-byte copy+SHA2, plus named
/// default comparisons, index comparisons, RLE and opaque-LZ4 input/output slots.
pub fn write_portable_snapshot_admitted_checked(
    doc: &Document,
    sink: impl Write,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    let failure = RefCell::new(None);
    let sink = Metered {
        sink,
        check,
        spend,
        failure: &failure,
    };
    // Precharge unhooked bounded metadata passes before the legacy helpers.
    // This prewalk only reads O(1) lengths; it never scans pixels/profile bytes.
    check()?;
    spend(doc.title.as_ref().map_or(0, |t| t.len()) as u64)?;
    for layer in doc.layers.values() {
        check()?;
        spend(1)?; // this prewalk's layer visit
        // Six layer visits: own presence/live filters, mask/spatial/catalog flags,
        // and aggregate coverage validation. Four filter visits in the latter
        // mask/spatial/catalog/coverage passes; short-circuit savings are retained
        // as conservative model slots, never a fresh counter or larger limit.
        spend(
            6u64.checked_add(
                (layer.filters.len() as u64)
                    .checked_mul(4)
                    .ok_or_else(limit)?,
            )
            .and_then(|n| n.checked_add(layer.name.len() as u64))
            .ok_or_else(limit)?,
        )?;
    }
    if let Some(source) = &doc.source_metadata {
        spend(
            (paintcraft_document::source_metadata::WIRE_HEADER_BYTES as u64)
                .checked_add(source.profile().map_or(0, |p| p.len()) as u64)
                .ok_or_else(limit)?,
        )?;
    }
    let checked = || {
        check()?;
        spend(1)
    };
    let result = write_encoded_admitted(doc, Output::new(sink), &checked, scope, spend);
    if let Some(error) = failure.into_inner() {
        return Err(error);
    }
    result
}
struct Metered<'a, W> {
    sink: W,
    check: &'a dyn Fn() -> Result<()>,
    spend: &'a dyn Fn(u64) -> Result<()>,
    failure: &'a RefCell<Option<Error>>,
}
impl<W: Write> Write for Metered<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let permit = (self.check)()
            .and_then(|()| (self.spend)((bytes.len() as u64).checked_mul(2).ok_or_else(limit)?));
        if let Err(error) = permit {
            *self.failure.borrow_mut() = Some(error);
            return Err(std::io::Error::other("snapshot preparation refused"));
        }
        self.sink.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.sink.flush()
    }
}
type BlockEmitter<'a> = dyn FnMut(u32, u32, &[u8]) -> Result<()> + 'a;
fn blocks(
    raster: &RasterLayer,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
    emit: &mut BlockEmitter<'_>,
) -> Result<()> {
    let count = raster.resident_tiles();
    let mut index_account = scope.account();
    index_account.resize(
        count
            .checked_mul(size_of::<(TileCoord, TileLease)>())
            .ok_or_else(limit)? as u64,
    )?;
    let mut index = Vec::new();
    index.try_reserve_exact(count).map_err(|_| limit())?;
    if let Err(e) = index_account.resize(
        index
            .capacity()
            .checked_mul(size_of::<(TileCoord, TileLease)>())
            .ok_or_else(limit)? as u64,
    ) {
        drop(index);
        return Err(e);
    }
    for entry in raster.tile_leases() {
        check()?;
        spend(1)?;
        index.push(entry);
    }
    // Heapsort, with actual comparison work/cancellation, no hidden sort scratch.
    sort(&mut index, check, spend)?;
    for (coord, lease) in &index {
        check()?;
        spend(lease.bytes().len() as u64)?;
        if lease
            .bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == raster.default_pixel().0)
        {
            continue;
        }
        let bx = u32::try_from(coord.tx).map_err(|_| codec("invalid tile coordinate"))?;
        let by = u32::try_from(coord.ty).map_err(|_| codec("invalid tile coordinate"))?;
        let w = (raster.width() - bx * 256).min(256) as usize;
        let h = (raster.height() - by * 256).min(256) as usize;
        let length = w
            .checked_mul(h)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(limit)?;
        spend(length as u64)?;
        let mut account = scope.account();
        account.resize(length as u64)?;
        let mut data = Vec::new();
        data.try_reserve_exact(length).map_err(|_| limit())?;
        if let Err(e) = account.resize(data.capacity() as u64) {
            drop(data);
            return Err(e);
        }
        for y in 0..h {
            check()?;
            let at = y * 256 * 4;
            data.extend_from_slice(&lease.bytes()[at..at + w * 4]);
        }
        emit(bx, by, &data)?;
        // data drops before its account, index before its own account.
    }
    Ok(())
}
fn sort(
    values: &mut [(TileCoord, TileLease)],
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    fn down(
        v: &mut [(TileCoord, TileLease)],
        mut root: usize,
        end: usize,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<()> {
        while root * 2 + 1 < end {
            let mut child = root * 2 + 1;
            check()?;
            spend(1)?;
            if child + 1 < end
                && (v[child].0.ty, v[child].0.tx) < (v[child + 1].0.ty, v[child + 1].0.tx)
            {
                child += 1;
            }
            check()?;
            spend(1)?;
            if (v[root].0.ty, v[root].0.tx) >= (v[child].0.ty, v[child].0.tx) {
                break;
            }
            v.swap(root, child);
            root = child;
        }
        Ok(())
    }
    let length = values.len();
    for root in (0..length / 2).rev() {
        down(values, root, length, check, spend)?;
    }
    for end in (1..length).rev() {
        values.swap(0, end);
        down(values, 0, end, check, spend)?;
    }
    Ok(())
}
fn limit() -> Error {
    Error::new(ErrorCode::ResourceLimit, "admitted snapshot scratch limit")
}

mod blend;
pub use blend::{
    portable_blend_snapshot_scratch_quote, write_portable_blend_snapshot_admitted_checked,
};

mod normal_rooted;
pub use normal_rooted::{
    portable_normal_snapshot_rooted_scratch_quote,
    write_portable_normal_snapshot_rooted_admitted_checked,
};

mod counted;
pub use counted::{
    CountedNormalSnapshot, CountedSnapshotBytes, NormalCountedControlBytes,
    count_portable_normal_snapshot_rooted_checked, normal_counted_snapshot_control_bytes,
};

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
