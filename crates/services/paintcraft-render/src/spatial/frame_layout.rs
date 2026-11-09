//! Defining-type ceilings for the opt-in closed-frame structural policy.
//! This is a layout refusal guard, never a capacity quote or resource grant.
use super::{LayerAppearancePlan, TiledQuote, WorkspaceQuote, plan::Stage, workspace::Workspace};
use paintcraft_core::types::{Rect, Rgba, resources::BudgetAccount};
use paintcraft_effects::spatial::BoxBlurPlan;
use std::mem::size_of;

pub(crate) const fn rooted_frame_inline_layout_supported() -> bool {
    let native = size_of::<usize>() == 8;
    let wasm = size_of::<usize>() == 4;
    (native || wasm)
        && size_of::<Stage>() <= 256
        && size_of::<paintcraft_core::types::Result<Stage>>() <= 256
        && size_of::<BoxBlurPlan>() <= 192
        && size_of::<paintcraft_core::types::Result<BoxBlurPlan>>() <= 192
        && size_of::<LayerAppearancePlan<'static>>() <= 160
        && size_of::<paintcraft_core::types::Result<LayerAppearancePlan<'static>>>() <= 160
        && size_of::<Workspace>() <= (if native { 136 } else { 72 })
        && size_of::<paintcraft_core::types::Result<Workspace>>() <= 160
        && size_of::<WorkspaceQuote>() <= (if native { 40 } else { 32 })
        && size_of::<paintcraft_core::types::Result<WorkspaceQuote>>() <= 128
        && size_of::<TiledQuote>() <= 24
        && size_of::<paintcraft_core::types::Result<TiledQuote>>() <= 128
        && size_of::<crate::noise_work::ScopedLayerQuote>() <= 32
        && size_of::<paintcraft_core::types::Result<crate::noise_work::ScopedLayerQuote>>() <= 128
        && size_of::<Rect>() <= 32
        && size_of::<Rgba>() == 4
        && size_of::<BudgetAccount>() <= (if native { 40 } else { 24 })
}

#[cfg(test)]
mod tests {
    #[test]
    fn closed_frame_spatial_layout_fits_predeclared_ceilings() {
        assert!(super::rooted_frame_inline_layout_supported());
    }
}
