//! Explicit clone peak under the pinned fresh HashMap/Vec/BTree policies.
use super::*;
use paintcraft_raster::raster::metadata_clone::fresh_map_layout;
use std::mem::{align_of, size_of};
#[derive(Clone, Copy, Debug)]
pub struct Quote {
    pub seed_bytes: u64,
    pub peak_bytes: u64,
    /// Storage only; the separate query guard is never assigned twice.
    pub storage_peak_bytes: u64,
    pub stage_work: u64,
    /// Future compact raster work only; the quote pays its own scalar math.
    pub raster_seed_supplement_work: u64,
    pub generic_lifecycle_work: u64,
    pub retained_alias_bytes: u64,
    /// Nested account carved from storage_peak, never a second full grant.
    pub generic_style_bytes: u64,
}
pub const fn quote_control_bytes() -> u64 {
    (size_of::<Quote>() + size_of::<BudgetAccount>()) as u64
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(limit)
}
fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(limit)
}
fn bytes<T>(count: usize) -> Result<u64> {
    mul(count as u64, size_of::<T>() as u64)
}
/// One11-slot BTree leaf plus header under Rust1.99. No private struct ABI is
/// guessed: sum field sizes and maximum per-field padding bounds any ordering.
fn alias_leaf_upper() -> Result<u64> {
    let fields = [
        (size_of::<usize>(), align_of::<usize>()),
        (2, 2),
        (2, 2),
        (size_of::<String>() * 11, align_of::<String>()),
        (size_of::<(Uuid, Uuid)>() * 11, align_of::<(Uuid, Uuid)>()),
    ];
    let maximum_alignment = fields.iter().map(|(_, align)| *align).max().unwrap_or(1);
    let fields = fields
        .into_iter()
        .try_fold(0u64, |n, (width, align)| add(n, (width + align - 1) as u64))?;
    add(fields, (maximum_alignment - 1) as u64) // final tail padding, any field order
}
pub(super) fn document_bytes(
    doc: &Document,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<(u64, u64, u64, u64, u64)> {
    if doc.layers.len() != doc.roots.len() || doc.roots.len() > paintcraft_document::LAYER_COUNT_MAX
    {
        return Err(invalid("metadata clone requires flat source roots"));
    }
    let mut total = add(
        size_of::<Document>() as u64,
        fresh_map_layout::<Uuid, Layer>(doc.roots.len())?.bytes,
    )?;
    total = add(total, bytes::<Uuid>(doc.roots.len())?)?;
    total = add(total, doc.title.as_ref().map_or(0, |s| s.len()) as u64)?;
    let mut raster_seed_supplement_work = 0;
    let mut filters = 0;
    let mut tiles = 0;
    let mut tile_scans = 0;
    for id in &doc.roots {
        check()?;
        spend(1)?;
        let layer = doc
            .layers
            .get(id)
            .filter(|l| l.id == *id)
            .ok_or_else(|| invalid("metadata clone root missing"))?;
        // Own arithmetic is funded before the first per-layer layout query.
        spend(64)?;
        total = add(total, layer.name.len() as u64)?;
        total = add(total, bytes::<FilterInstance>(layer.filters.len())?)?;
        let raster_layout = layer.raster.compact_metadata_clone_quote()?;
        total = add(total, raster_layout.bytes)?;
        if !layer.styles.is_empty() {
            spend(32)?;
            total = add(total, bytes::<StyleInstance>(layer.styles.len())?)?;
        }
        filters = add(
            filters,
            add(layer.filters.len() as u64, layer.styles.len() as u64)?,
        )?;
        let n = layer.raster.resident_tiles() as u64;
        tiles = add(tiles, n)?;
        if n != 0 {
            tile_scans = add(tile_scans, work::scan(raster_layout)?)?;
        }
        raster_seed_supplement_work = add(
            raster_seed_supplement_work,
            layer.raster.compact_metadata_clone_retained_work_quote()?,
        )?;
    }
    Ok((
        total,
        raster_seed_supplement_work,
        filters,
        tiles,
        tile_scans,
    ))
}
pub fn quote_checked(
    doc: &Document,
    batch: &Batch,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<Quote> {
    check()?;
    let operation = operation(batch)?;
    if batch.base_state != doc.state_id {
        return Err(Error::new(
            ErrorCode::ConflictStaleBase,
            "metadata clone batch base changed",
        ));
    }
    if batch.policy != paintcraft_document::POLICY {
        return Err(invalid("metadata clone policy changed"));
    }
    operation
        .stable_uuid_refs()
        .ok_or_else(|| invalid("metadata clone needs stable filter target"))?;
    // Fund global/target arithmetic BEFORE even the first fresh layer layout.
    spend(64)?;
    // Target existence/alias mutation remain the actual generic executor's job.
    let (seed_bytes, raster_seed_supplement_work, filters, tiles, tile_scans) =
        document_bytes(doc, check, spend)?;
    let generic_lifecycle_work = add(
        work::lifecycle(doc, operation, filters, tiles, tile_scans)?,
        add(
            super::super::style_controls::formation_work(),
            mul(
                64,
                add(doc.layers.len() as u64, batch.operations.len() as u64)?,
            )?,
        )?,
    )?;
    // Both passes may construct a256-entry target Vec while old source vectors
    // still live. Clone peak includes seed +planning clone +replay RHS clone.
    let temporary_filters = match operation {
        MetadataOp::Filter(_) => mul(bytes::<FilterInstance>(256)?, 2)?,
        MetadataOp::Style(_) => 0, // replacement vectors belong to nested Style account
    };
    // Pure query scan/layout arithmetic is funded before its defining helper.
    let has_styles = doc.layers.values().any(|layer| !layer.styles.is_empty());
    let style_path = has_styles || matches!(operation, MetadataOp::Style(_));
    let generic_style_bytes = if style_path {
        spend(super::super::style_controls::formation_work() + (doc.layers.len() as u64) * 64)?;
        super::super::style_controls::quote_checked(doc, batch, check)?.bytes
    } else {
        0
    };
    let alias_bytes = if let Some(name) = operation.alias() {
        if name.len() > 64 {
            return Err(invalid("metadata clone alias length"));
        }
        let map = fresh_map_layout::<String, Option<(Uuid, Uuid)>>(1)?.bytes;
        add(
            add(map, alias_leaf_upper()?)?,
            add(bytes::<(String, (Uuid, Uuid))>(4)?, name.len() as u64)?,
        )?
    } else {
        0
    };
    // Real deterministic ID code allocates base-state text36, layer text36,
    // index text1 and joined filter/UUID/base/digest/index bytes147. These are
    // bounded control temporaries, not an unpriced opaque allowance.
    let deterministic_text = 36
        + 36
        + 1
        + match operation {
            MetadataOp::Filter(_) => 147,
            MetadataOp::Style(_) => 152,
        };
    let storage_peak_bytes = add(
        mul(seed_bytes, 3)?,
        add(
            generic_style_bytes,
            add(temporary_filters, add(alias_bytes, deterministic_text)?)?,
        )?,
    )?;
    let hash_record = 16 + 1 + 2 + 96 + 4 + 6 + 1 + 32;
    let stack_hash = add(
        b"paintcraft.filter-stack/v3\0".len() as u64 + 2,
        mul(256, hash_record)?,
    )?;
    let validations = 256 * 255 / 2;
    let mut stage_work = add(
        add(mul(seed_bytes, 2)?, add(temporary_filters, alias_bytes)?)?,
        mul(2, add(validations, add(stack_hash, 2 * (147 + 64))?)?)?,
    )?;
    if style_path {
        // Extra source vectors in both generic clones/retirements, plus bounded
        // target validation/hash/descriptor and deterministic-ID control passes.
        let style_records = doc
            .layers
            .values()
            .try_fold(0u64, |n, l| add(n, l.styles.len() as u64))?;
        stage_work = add(
            stage_work,
            add(
                mul(style_records, 4 * (size_of::<StyleInstance>() as u64 + 8))?,
                2 * (256 * 255 / 2 + 256 * (16 + 1 + 2 + 20 + 4 + 5) + 64 + 2 * 256),
            )?,
        )?;
    }
    let retained_alias_bytes = if let Some(name) = operation.alias() {
        add(alias_leaf_upper()?, name.len() as u64)?
    } else {
        0
    };
    Ok(Quote {
        seed_bytes,
        peak_bytes: add(storage_peak_bytes, quote_control_bytes())?,
        storage_peak_bytes,
        stage_work,
        raster_seed_supplement_work,
        generic_lifecycle_work,
        retained_alias_bytes,
        generic_style_bytes,
    })
}

/// REQUIRE actual decoder->immutable owner or compact-seed provenance. Public
/// capacity equality is consistency only, never a certificate for arbitrary live
/// maps with hidden buckets/tombstones. Caller retains its preexisting baseline.
pub fn retained_quote_checked(
    doc: &Document,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<u64> {
    check()?;
    if doc.layers.len() != doc.roots.len() || doc.roots.len() > paintcraft_document::LAYER_COUNT_MAX
    {
        return Err(invalid("decoded metadata root shape"));
    }
    let layout = fresh_map_layout::<Uuid, Layer>(doc.layers.len())?;
    if doc.layers.capacity() != layout.capacity {
        return Err(invalid(
            "decoded layer map differs from pinned fresh layout",
        ));
    }
    let mut total = add(size_of::<Document>() as u64, layout.bytes)?;
    total = add(total, bytes::<Uuid>(doc.roots.capacity())?)?;
    total = add(total, doc.title.as_ref().map_or(0, |s| s.capacity()) as u64)?;
    for id in &doc.roots {
        check()?;
        spend(1)?;
        let layer = doc
            .layers
            .get(id)
            .filter(|l| l.id == *id)
            .ok_or_else(|| invalid("decoded metadata root missing"))?;
        total = add(total, layer.name.capacity() as u64)?;
        total = add(total, layer.filters.storage_bytes() as u64)?;
        total = add(total, layer.styles.storage_bytes() as u64)?;
        total = add(
            total,
            layer
                .raster
                .retained_decoded_metadata_quote_checked(check, spend)?,
        )?;
    }
    Ok(total)
}
