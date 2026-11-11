//! Priced direct tuple evaluation. No LUT, program owner or fallback realm.
use crate::*;
use paintcraft_core::types::filters::Descriptor;
use paintcraft_document::Layer;

pub(crate) fn active(layer: &Layer) -> bool {
    crate::styles::stored(layer)
        || layer
            .filters
            .iter()
            .any(|f| f.enabled() && priced(f.descriptor()))
}
/// Even zero-amount Noise keeps the coordinate-profile proof and pricing path.
pub(crate) fn coordinate_dependent(layer: &Layer) -> bool {
    layer
        .filters
        .iter()
        .any(|f| f.enabled() && matches!(f.descriptor(), Descriptor::AddNoiseRgbV1(_)))
}
pub(crate) fn coordinate_document(doc: &Document) -> bool {
    doc.layers.values().any(coordinate_dependent)
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
pub(crate) fn priced(descriptor: Descriptor) -> bool {
    matches!(
        descriptor,
        Descriptor::VibranceRgbV1(_)
            | Descriptor::HueSaturationRgbV1(_)
            | Descriptor::AddNoiseRgbV1(_)
    )
}
/// Derive each direct stage's price from its qualified Effects contract.
/// Old scalar/prepared profiles contribute no additional direct work.
pub(crate) fn work(descriptor: Descriptor, count: u64) -> Result<u64> {
    match descriptor {
        Descriptor::AddNoiseRgbV1(policy) => {
            let count = usize::try_from(count).map_err(|_| limit())?;
            Ok(paintcraft_effects::noise::quote(policy, count)?.work)
        }
        Descriptor::VibranceRgbV1(_) => {
            let count = usize::try_from(count).map_err(|_| limit())?;
            Ok(paintcraft_effects::vibrance::quote(count)?.work)
        }
        Descriptor::HueSaturationRgbV1(_) => {
            let count = usize::try_from(count).map_err(|_| limit())?;
            Ok(paintcraft_effects::hue_saturation::quote(count)?.work)
        }
        _ => Ok(0),
    }
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
    crate::layer_mask::require_document(doc)?;
    if coordinate_document(doc) {
        crate::noise_work::required()?;
    }
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
    if matches!(descriptor, Descriptor::AddNoiseRgbV1(_)) {
        return crate::noise_work::current_for(admission.ok_or_else(missing)?)?.reserve_components(
            crate::noise_work::WorkComponents {
                base: 0,
                noise: work(descriptor, count)?,
            },
        );
    }
    if priced(descriptor) {
        admission
            .ok_or_else(missing)?
            .reserve_work(work(descriptor, count)?)?;
    }
    Ok(())
}

/// Bound direct arithmetic before cached output/allocation without repricing old paths.
pub(crate) fn preflight(
    doc: &Document,
    region: Rect,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    if doc.layers.values().any(active) || crate::layer_mask::active_document(doc) {
        crate::spatial::preflight(doc, region, check)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "direct_tests.rs"]
mod tests;
