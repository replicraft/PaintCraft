//! PCND8-only tagged property. Old1..7 records contain no byte here.
use super::*;
use paintcraft_document::LayerBlendMode;
pub(super) fn write<W: Write>(
    version: u8,
    mode: LayerBlendMode,
    out: &mut Output<W>,
) -> Result<()> {
    if version == 8 {
        out.bytes(&[mode as u8])?;
    }
    Ok(())
}
pub(super) fn read(version: u8, input: &mut Input<'_>) -> Result<LayerBlendMode> {
    if version != 8 {
        return Ok(LayerBlendMode::Normal);
    }
    match input.byte()? {
        0 => Ok(LayerBlendMode::Normal),
        1 => Ok(LayerBlendMode::Multiply),
        2 => Ok(LayerBlendMode::Screen),
        _ => Err(codec("unknown PCND8 layer blend tag")),
    }
}

/// New framed component; None preserves every old all-Normal identity branch.
/// Root validation/sorting and hashing spend the caller's existing base budget.
pub fn layer_blend_identity_checked(
    doc: &Document,
    scope: &paintcraft_core::types::resources::BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<Option<[u8; 32]>> {
    crate::pcnd::layer_appearance::validate_flat_shape_checked(doc, scope, check, spend)?;
    // One stored-mode scan, separately from the authenticated framed hash.
    spend(doc.layers.len() as u64)?;
    if !doc.has_non_normal_blends() {
        return Ok(None);
    }
    let count = u32::try_from(doc.roots.len()).map_err(|_| codec("layer blend count"))?;
    let bytes = (b"paintcraft.layer-blends/v1\0".len() as u64)
        .checked_add(20)
        .and_then(|n| n.checked_add(u64::from(count).checked_mul(17)?))
        .ok_or_else(|| codec("layer blend hash extent"))?;
    // Explicit 2048-unit policy slot for each typed root lookup. This is not
    // a worst-case HashMap probe bound, instruction count, or pixel blend price.
    spend(
        u64::from(count)
            .checked_mul(2048)
            .ok_or_else(|| codec("layer blend lookup work"))?,
    )?;
    spend(
        bytes
            .checked_mul(16)
            .ok_or_else(|| codec("layer blend hash work"))?,
    )?;
    let mut hash = Sha256::new();
    hash.update(b"paintcraft.layer-blends/v1\0");
    hash.update(doc.document_id.as_bytes());
    hash.update(count.to_be_bytes());
    for id in &doc.roots {
        check()?;
        hash.update(id.as_bytes());
        hash.update([doc.layers[id].blend_mode as u8]);
    }
    check()?;
    Ok(Some(hash.finalize().into()))
}
