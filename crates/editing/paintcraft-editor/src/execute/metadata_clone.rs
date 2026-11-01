//! A compact admitted seed feeds the EXISTING generic Stage, never a second filter engine.
use super::*;
use paintcraft_commands::FilterOp;
use paintcraft_core::types::resources::{BudgetAccount, BudgetScope};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_raster::raster::metadata_clone::fresh_map_layout;
use std::mem::size_of;
mod layout;
pub use layout::{Quote, quote_checked, retained_quote_checked};
#[cfg(test)]
mod tests;
#[derive(Debug)]
pub struct StagedMetadata {
    staged: Staged,
    // Staged Document and alias tree drop before their declaration.
    _account: BudgetAccount,
}
impl StagedMetadata {
    pub fn document(&self) -> &Document {
        &self.staged.doc
    }
    pub fn filter_aliases(&self) -> &FilterAliases {
        &self.staged.filter_aliases
    }
}
fn operation(batch: &Batch) -> Result<&FilterOp> {
    let [Op::Filter(operation)] = batch.operations.as_slice() else {
        return Err(invalid("metadata clone requires one filter operation"));
    };
    if matches!(operation, FilterOp::Mask { .. }) {
        return Err(invalid("metadata clone does not mutate masks"));
    }
    Ok(operation)
}
pub fn stage_checked(
    base: &Document,
    batch: &Batch,
    admission: Admission,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<StagedMetadata> {
    let quote = quote_checked(base, batch, check, spend)?;
    let mut account = scope.account();
    account.resize(quote.peak_bytes)?;
    let seed = compact_seed(base, &account, check, spend)?;
    // Precharge both generic clones and bounded target validations/hash inputs.
    // Cancellation/control callbacks below additionally debit one actual visit.
    spend(quote.stage_work)?;
    let checked = || {
        check()?;
        spend(1)
    };
    let staged = execute_with_admission_checked(
        &seed,
        Selection::Inactive,
        batch,
        admission.with_paint_metadata_scope(scope.clone()),
        &checked,
    )?;
    drop(seed);
    if !staged.aliases.is_empty() || staged.filter_aliases.len() > 1 {
        return Err(invalid("metadata clone unexpected aliases"));
    }
    // No layer/raster map is mutated by these six non-mask FilterOp variants.
    // The pinned clone keeps the seed's fresh bucket count exactly.
    let retained = retained_quote_checked(&staged.doc, check, spend)?
        .checked_add(quote.retained_alias_bytes)
        .ok_or_else(limit)?;
    if retained > quote.peak_bytes {
        return Err(limit());
    }
    account.resize(retained)?;
    Ok(StagedMetadata {
        staged,
        _account: account,
    })
}
fn compact_seed(
    base: &Document,
    account: &BudgetAccount,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<Document> {
    check()?;
    let map_layout = fresh_map_layout::<Uuid, Layer>(base.roots.len())?;
    spend(map_layout.bytes)?;
    let mut layers = HashMap::new();
    layers.try_reserve(base.roots.len()).map_err(|_| limit())?;
    if layers.capacity() != map_layout.capacity {
        return Err(invalid("fresh layer map differs from pinned layout"));
    }
    let mut roots = Vec::new();
    spend((base.roots.len() * size_of::<Uuid>()) as u64)?;
    roots
        .try_reserve_exact(base.roots.len())
        .map_err(|_| limit())?;
    if roots.capacity() != base.roots.len() {
        return Err(invalid("fresh roots capacity differs"));
    }
    for id in &base.roots {
        check()?;
        spend(size_of::<Layer>() as u64)?;
        let layer = base
            .layers
            .get(id)
            .ok_or_else(|| invalid("metadata seed root missing"))?;
        let raster = layer
            .raster
            .compact_metadata_clone_precharged_checked(account, check, spend)?;
        let mut instances = Vec::new();
        spend((layer.filters.len() * size_of::<FilterInstance>()) as u64)?;
        instances
            .try_reserve_exact(layer.filters.len())
            .map_err(|_| limit())?;
        if instances.capacity() != layer.filters.len() {
            return Err(invalid("fresh filter capacity differs"));
        }
        instances.extend(layer.filters.iter().cloned());
        // Preserve cached semantic digest through checked constructor; charge its
        // bounded pair validation and exact published record-input upper first.
        let n = instances.len() as u64;
        spend(n * n.saturating_sub(1) / 2 + n * 158 + 32)?;
        let filters = FilterStack::new(instances)?;
        let name = text(&layer.name, spend)?;
        if layers
            .insert(
                *id,
                Layer {
                    id: layer.id,
                    name,
                    origin: layer.origin,
                    visible: layer.visible,
                    opacity: layer.opacity,
                    raster,
                    filters,
                    mask: layer.mask.clone(),
                },
            )
            .is_some()
        {
            return Err(invalid("metadata seed root repeats"));
        }
        roots.push(*id);
    }
    let title = base.title.as_deref().map(|s| text(s, spend)).transpose()?;
    Ok(Document {
        document_id: base.document_id,
        state_id: base.state_id,
        canvas: base.canvas,
        roots,
        layers,
        title,
        source_metadata: base.source_metadata.clone(),
    })
}
fn text(value: &str, spend: &dyn Fn(u64) -> Result<()>) -> Result<String> {
    spend(value.len() as u64)?;
    let mut result = String::new();
    result.try_reserve_exact(value.len()).map_err(|_| limit())?;
    if result.capacity() != value.len() {
        return Err(invalid("fresh text capacity differs"));
    }
    result.push_str(value);
    Ok(result)
}
fn limit() -> Error {
    Error::new(ErrorCode::ResourceLimit, "metadata stage peak exhausted")
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
