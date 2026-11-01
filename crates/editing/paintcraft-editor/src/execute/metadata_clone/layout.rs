//! Explicit clone peak under the pinned fresh HashMap/Vec/BTree policies.
use super::*;
use paintcraft_raster::raster::metadata_clone::fresh_map_layout;
use std::mem::{align_of, size_of};
#[derive(Clone, Copy, Debug)]
pub struct Quote {
    pub seed_bytes: u64,
    pub peak_bytes: u64,
    pub stage_work: u64,
    pub retained_alias_bytes: u64,
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
fn alias(operation: &FilterOp) -> Option<&str> {
    match operation {
        FilterOp::Add { alias, .. } | FilterOp::Duplicate { alias, .. } => Some(alias),
        _ => None,
    }
}
pub(super) fn document_bytes(
    doc: &Document,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<u64> {
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
    for id in &doc.roots {
        check()?;
        spend(1)?;
        let layer = doc
            .layers
            .get(id)
            .filter(|l| l.id == *id)
            .ok_or_else(|| invalid("metadata clone root missing"))?;
        total = add(total, layer.name.len() as u64)?;
        total = add(total, bytes::<FilterInstance>(layer.filters.len())?)?;
        total = add(total, layer.raster.compact_metadata_clone_quote()?.bytes)?;
    }
    Ok(total)
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
    // Target existence/alias mutation remain the actual generic executor's job.
    let seed_bytes = document_bytes(doc, check, spend)?;
    // Both passes may construct a256-entry target Vec while old source vectors
    // still live. Clone peak includes seed +planning clone +replay RHS clone.
    let temporary_filters = mul(bytes::<FilterInstance>(256)?, 2)?;
    let alias_bytes = if let Some(name) = alias(operation) {
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
    let deterministic_text = 36 + 36 + 1 + 147;
    let peak_bytes = add(
        mul(seed_bytes, 3)?,
        add(temporary_filters, add(alias_bytes, deterministic_text)?)?,
    )?;
    let hash_record = 16 + 1 + 2 + 96 + 4 + 6 + 1 + 32;
    let stack_hash = add(
        b"paintcraft.filter-stack/v3\0".len() as u64 + 2,
        mul(256, hash_record)?,
    )?;
    let validations = 256 * 255 / 2;
    let stage_work = add(
        add(mul(seed_bytes, 2)?, add(temporary_filters, alias_bytes)?)?,
        mul(2, add(validations, add(stack_hash, 2 * (147 + 64))?)?)?,
    )?;
    let retained_alias_bytes = if let Some(name) = alias(operation) {
        add(alias_leaf_upper()?, name.len() as u64)?
    } else {
        0
    };
    Ok(Quote {
        seed_bytes,
        peak_bytes,
        stage_work,
        retained_alias_bytes,
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
        total = add(
            total,
            layer
                .raster
                .retained_decoded_metadata_quote_checked(check, spend)?,
        )?;
    }
    Ok(total)
}
