//! Priced direct tuple evaluation. No LUT, program owner or fallback realm.
use crate::*;
use paintcraft_core::types::filters::Descriptor;
use paintcraft_document::Layer;

pub(crate) fn active(layer: &Layer) -> bool {
    layer
        .filters
        .iter()
        .any(|f| f.enabled() && matches!(f.descriptor(), Descriptor::VibranceRgbV1(_)))
}
pub(crate) fn active_document(doc: &Document) -> bool {
    doc.layers.values().any(active)
}
pub(crate) fn relevant(doc: &Document, rect: Rect) -> bool {
    doc.layers_in_order().any(|layer| {
        layer.visible
            && layer.opacity != 0
            && active(layer)
            && rect.intersect(&layer.extent()).is_some()
    })
}
pub(crate) fn work(count: u64) -> Result<u64> {
    let count = usize::try_from(count).map_err(|_| limit())?;
    Ok(paintcraft_effects::vibrance::quote(count)?.work)
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Direct point work exceeds caller admission",
    )
}
pub(crate) fn missing() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Priced direct appearance requires an explicit caller admission",
    )
}
/// Callback-free for old-only documents; runs before public CPU/GPU cache hits.
pub(crate) fn require_document(doc: &Document) -> Result<()> {
    if doc.layers.values().any(active) && crate::spatial::current().is_none() {
        return Err(missing());
    }
    Ok(())
}
pub(crate) fn reserve(
    descriptor: Descriptor,
    count: u64,
    admission: Option<&crate::spatial::WorkspaceAdmission>,
) -> Result<()> {
    if matches!(descriptor, Descriptor::VibranceRgbV1(_)) {
        admission.ok_or_else(missing)?.reserve_work(work(count)?)?;
    }
    Ok(())
}

/// Bound direct arithmetic before cached output/allocation without repricing old paths.
pub(crate) fn preflight(
    doc: &Document,
    region: Rect,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    if doc.layers.values().any(active) {
        crate::spatial::preflight(doc, region, check)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "direct_tests.rs"]
mod tests;
