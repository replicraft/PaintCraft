//! Exact old v1 filter fingerprint, WITHOUT a bake witness or copy consent.
use super::*;
use paintcraft_core::types::policy::COORD_ABS_MAX;
/// Same digest bytes as filter_bake::inspect on valid None documents. Own masks
/// are intentionally bound separately by Source7 facts, not inserted into v1.
/// This function returns no Inventory/BakeWitness/Policy and cannot waive the
/// old inspect's require_no_layer_masks guard or authorize a foreign copy.
pub fn filter_digest_checked(
    doc: &Document,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<[u8; 32]> {
    validate_flat_shape_checked(doc, scope, check, spend)?;
    let checked = step(check, spend);
    let mut controls = scope.account();
    controls.resize(CONTROL_BYTES)?;
    // v1 fixed header: IDs32 + canvas8 + root count8. Every layer contributes
    // key16+filterDigest32+visibility/opacity2+origin16+dimensions8+defaultRGBA4.
    let hash_bytes = (b"paintcraft.filter-bake-witness/v1\0".len() as u64)
        .checked_add(48)
        .and_then(|n| n.checked_add((doc.roots.len() as u64).checked_mul(78)?))
        .ok_or_else(resource)?;
    spend(hash_bytes.checked_mul(16).ok_or_else(resource)?)?;
    let mut h = Sha256::new();
    h.update(b"paintcraft.filter-bake-witness/v1\0");
    h.update(doc.document_id.as_bytes());
    h.update(doc.state_id.as_bytes());
    h.update(doc.canvas.0.to_be_bytes());
    h.update(doc.canvas.1.to_be_bytes());
    h.update((doc.roots.len() as u64).to_be_bytes());
    for id in &doc.roots {
        checked()?;
        let layer = doc.layers.get(id).ok_or_else(invalid)?;
        h.update(id.as_bytes());
        h.update(layer.filters.digest());
        h.update([u8::from(layer.visible), layer.opacity]);
        h.update(layer.origin.0.to_be_bytes());
        h.update(layer.origin.1.to_be_bytes());
        h.update(layer.raster.width().to_be_bytes());
        h.update(layer.raster.height().to_be_bytes());
        h.update(layer.raster.default_pixel().0);
    }
    check()?;
    Ok(h.finalize().into())
}
/// Shared only within PCND: one scope-funded root index, freed before return.
pub(in crate::pcnd) fn flat_shape_scratch_quote(layers: usize) -> Result<u64> {
    if layers > LAYER_COUNT_MAX {
        return Err(resource());
    }
    CONTROL_BYTES
        .checked_add(bytes::<Index>(layers)?)
        .ok_or_else(resource)
}
pub(in crate::pcnd) fn validate_flat_shape_checked(
    doc: &Document,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    let checked = step(check, spend);
    if doc.roots.len() > LAYER_COUNT_MAX || doc.layers.len() > LAYER_COUNT_MAX {
        return Err(resource());
    }
    if doc.roots.len() != doc.layers.len() {
        return Err(invalid());
    }
    if [doc.canvas.0, doc.canvas.1]
        .iter()
        .any(|n| !(1..=1_048_576).contains(n))
    {
        return Err(invalid());
    }
    let mut controls = scope.account();
    controls.resize(CONTROL_BYTES)?;
    let mut roots = Indices::new(doc.roots.len(), scope)?;
    for id in &doc.roots {
        checked()?;
        roots.push(Index {
            id: *id.as_bytes(),
            offset: 0,
        })?;
    }
    // Sort ONLY for shape/uniqueness validation. Hash still uses actual root
    // order, never sorted UUID order. No HashSet or quadratic root scan.
    roots.sort(check, spend)?;
    for index in &roots.values {
        checked()?;
        let layer = doc
            .layers
            .get(&Uuid::from_bytes(index.id))
            .ok_or_else(invalid)?;
        if layer.id.as_bytes() != &index.id
            || layer.origin.0.unsigned_abs() > COORD_ABS_MAX as u64
            || layer.origin.1.unsigned_abs() > COORD_ABS_MAX as u64
            || [layer.raster.width(), layer.raster.height()]
                .iter()
                .any(|n| !(1..=1_048_576).contains(n))
        {
            return Err(invalid());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
