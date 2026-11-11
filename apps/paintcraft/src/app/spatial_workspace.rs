//! One process-shared secondary workspace realm. Final output/source/GPU storage
//! remain priced by their existing host job, with this ceiling added for Box.
use paintcraft_core::types::{
    Result,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::Document;
use paintcraft_render::spatial::{WorkspaceAdmission, with_workspace};
use std::sync::OnceLock;

#[path = "spatial_workspace/noise.rs"]
mod noise;
pub(crate) use noise::{JobPolicy, legacy_document, refuse_noise};
pub(crate) const BYTES: u64 = 64 << 20;
pub(crate) const WORK: u64 = 1_000_000_000;

fn budget() -> SharedBudget {
    static SHARED: OnceLock<SharedBudget> = OnceLock::new();
    SHARED.get_or_init(|| SharedBudget::new(BYTES)).clone()
}
pub(crate) fn needed(doc: &Document) -> bool {
    doc.layers.values().any(needed_layer)
}
pub(crate) fn needed_layer(layer: &paintcraft_document::Layer) -> bool {
    !layer.styles.is_empty()
        || layer.blend_mode != paintcraft_core::types::LayerBlendMode::Normal
        || layer.mask.as_ref().is_some_and(|mask| mask.enabled())
        || layer
            .filters
            .iter()
            .any(|filter| filter.descriptor().is_spatial() || filter.descriptor().is_catalog_only())
}
/// The executing thread installs the context; submitting a closure on the UI
/// thread does not propagate a thread-local factory into an executor worker.
pub(crate) fn with_job<T>(required: bool, work: impl FnOnce() -> Result<T>) -> Result<T> {
    if !required {
        return work();
    }
    with_admission(budget(), work)
}
fn with_admission<T>(budget: SharedBudget, work: impl FnOnce() -> Result<T>) -> Result<T> {
    let admission = paintcraft_render::spatial::capture_workspace_or_current(
        || WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), BYTES, WORK),
        BYTES,
        WORK,
    )?;
    with_workspace(admission, work)
}

#[cfg(test)]
#[path = "spatial_workspace/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "spatial_workspace/layer_mask_tests.rs"]
mod layer_mask_tests;

#[cfg(test)]
#[path = "spatial_workspace/blend_tests.rs"]
mod blend_tests;
