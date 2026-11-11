//! Source-dependent bounds for every future serial native Source8 call.
use super::{UnboundProgramForecast, absent, quote_unbound_programs_checked};
use crate::{
    blend::{self, rooted::RootFacts},
    display::DisplayAccumulator,
    noise_work::NoiseAdmission,
    prepared::blend_programs,
    spatial::{LayerAppearancePlan, WorkspaceQuote},
};
use paintcraft_core::types::{Rect, Result, filters::Descriptor};
use paintcraft_document::{Document, Layer};
use std::mem::size_of;
const CONTROLS: u64 = 32768;
const FORMATION: u64 = 81920;
const CHECK: u64 = 1024;
const EDGE: u32 = 64;

/// Fixed caller-owned query controls, discovered before admitting the query.
/// Pure defining declaration: no allocation, callback, counter or grant.
pub const fn unbound_rooted_frame_workspace_control_bytes() -> u64 {
    CONTROLS
}

/// Logical demand facts, not scopes, grants, cached authority or heap usage.
/// Worker-owned read/F/O/H declarations remain separate. In particular the
/// accumulator is original C storage and native RGBA is original O storage.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnboundRootedFrameWorkspace {
    pub prepared_retained_bytes: u64,
    pub preparation_peak_bytes: u64,
    pub spatial_workspace_bytes: u64,
    pub blend_context_bytes: u64,
    pub rooted_witness_bytes: u64,
    pub seed_witness_bytes: u64,
    pub noise_sidecar_bytes: u64,
    pub workspace_bytes: u64,
    pub native_rgba_bytes: u64,
    pub display_cells: u64,
    pub display_accumulator_bytes: u64,
    pub query_control_bytes: u64,
    pub requires_noise: bool,
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(blend::limit)
}
fn layout() -> Result<()> {
    let named = blend_programs::CAPTURE_BYTES
        + 3 * size_of::<UnboundRootedFrameWorkspace>() as u64
        + 3 * size_of::<Result<UnboundRootedFrameWorkspace>>() as u64
        + 2 * size_of::<UnboundProgramForecast>() as u64
        + 2 * size_of::<Result<UnboundProgramForecast>>() as u64
        + 2 * size_of::<LayerAppearancePlan<'static>>() as u64
        + 2 * size_of::<Result<LayerAppearancePlan<'static>>>() as u64
        + 2 * size_of::<WorkspaceQuote>() as u64
        + 2 * size_of::<RootFacts<'static>>() as u64
        + 6 * size_of::<Rect>() as u64
        + 16 * size_of::<Result<()>>() as u64
        + 16 * size_of::<u64>() as u64
        + 16 * size_of::<&u8>() as u64;
    if !crate::rooted_frame_inline_layout_supported()
        || size_of::<DisplayAccumulator>() != 40
        || size_of::<UnboundRootedFrameWorkspace>() > 112
        || size_of::<Result<UnboundRootedFrameWorkspace>>() > 128
        || named > CONTROLS
    {
        return Err(blend::limit());
    }
    Ok(())
}
// Any future spatial descriptor must be reviewed here, not silently treated as
// geometry preserving. Current point and coordinate-Noise stages do not grow.
fn spatial_descriptor(value: Descriptor) -> bool {
    match value {
        Descriptor::BoxBlurV1(_) => true,
        Descriptor::InvertRgbV1
        | Descriptor::BrightnessContrastV1(_)
        | Descriptor::BlackWhiteRgbV1(_)
        | Descriptor::ThresholdLumaV1(_)
        | Descriptor::PosterizeRgbV1(_)
        | Descriptor::SolarizeRgbV1
        | Descriptor::LevelsRgbV1(_)
        | Descriptor::ExposureSrgbV1(_)
        | Descriptor::CurvesRgbV1(_)
        | Descriptor::VibranceRgbV1(_)
        | Descriptor::HueSaturationRgbV1(_)
        | Descriptor::AddNoiseRgbV1(_) => false,
    }
}
fn centered(layer: &Layer, edge: u32) -> Result<Rect> {
    let width = layer.raster.width();
    let height = layer.raster.height();
    let w = width.min(edge);
    let h = height.min(edge);
    Rect::new(
        i64::from((width - w) / 2),
        i64::from((height - h) / 2),
        i64::from(w),
        i64::from(h),
    )
    .ok_or_else(blend::limit)
}
/// Forecast one fresh prepared owner and the maximum serial spatial workspace
/// for EVERY future native ROI whose width and height are at most64.
///
/// Both Base and Noise TLS must be absent before and after every callback.
/// The host retains and explicitly debits its original Main operation through
/// `spend_metadata`; this function creates no counter, scope or admission. The
/// host admits `query_control_bytes`32768 before calling. Formation81920 is paid
/// before layout/quote helpers; each raw check prepays1024; each layer pays
///1024+16384 per stored filter before classification/planning. Existing complete
/// key helper debits are forwarded to this SAME callback. Failed work remains
/// paid; retry does not renew the caller. Initial absent-TLS detection retains
/// the existing metadata bootstrap boundary.
///
/// Centering a maximal local64 rectangle simultaneously maximizes every
/// symmetric CLAMP halo in the supported reverse BoxBlur stack. Point/Noise
/// stages preserve geometry. The same centered plan also maximizes each blur
/// scratch product(output width * expanded input height). See source-only
/// analytic/exhaustive expectations; no full-canvas intermediate is substituted.
///
/// Successful supported-target calculation allocates no payload and invokes
/// only the explicit callbacks. Callback allocations and arbitrary owned errors,
/// allocator overcapacity/slack, inherited helper bookkeeping and GPU storage
/// are outside this logical forecast. Worker must independently enforce its
/// original role capacities and same-source binding; values grant no authority.
///
/// # Errors
/// Returns original callback errors unchanged; rejects installed contexts,
/// malformed/duplicate roots, absent stored nonNormal, unsupported layout,
/// invalid source geometry, key limits and checked arithmetic overflow. A
/// rejected diagnostic may own the existing Error message allocation.
pub fn quote_unbound_rooted_frame_workspace_checked(
    doc: &Document,
    check: &dyn Fn() -> Result<()>,
    spend_metadata: &dyn Fn(u64) -> Result<()>,
) -> Result<UnboundRootedFrameWorkspace> {
    absent()?;
    let spend = |n| {
        absent()?;
        spend_metadata(n)?;
        absent()
    };
    spend(FORMATION)?;
    layout()?;
    let checked = || {
        spend(CHECK)?;
        check()?;
        absent()
    };
    let _facts = RootFacts::capture(doc, &checked, &spend)?;
    let program = quote_unbound_programs_checked(doc, &checked, &spend)?;
    let mut spatial = 0;
    for id in &doc.roots {
        checked()?;
        let layer = doc.layers.get(id).ok_or_else(blend::limit)?;
        let fee = (layer.filters.len() as u64)
            .checked_mul(16384)
            .and_then(|n| n.checked_add(1024))
            .ok_or_else(blend::limit)?;
        spend(fee)?;
        let active = layer
            .filters
            .iter()
            .any(|f| f.enabled() && spatial_descriptor(f.descriptor()));
        if layer.visible && layer.opacity != 0 && active {
            let plan = LayerAppearancePlan::new(layer, centered(layer, EDGE)?, &checked)?;
            spatial = spatial.max(plan.quote().bytes);
        }
    }
    spend(1024)?;
    let context = blend::control_quote();
    let rooted = blend::rooted::control_bytes();
    let seed = blend::seed::CONTROL;
    let sidecar = if program.requires_noise {
        NoiseAdmission::control_quote()?.counter_bytes
    } else {
        0
    };
    let render = add(
        add(
            add(add(program.numerical.retained_bytes, spatial)?, context)?,
            rooted,
        )?,
        seed,
    )?;
    // Fresh preparation always admits Keys, including the zero-program case.
    // Its charge ends before the seeded render witnesses/context coexist.
    let preparation = program
        .numerical
        .preparation_peak_bytes
        .max(blend_programs::CAPTURE_BYTES);
    let workspace = add(sidecar, render.max(preparation))?;
    let cells = u64::from(doc.canvas.0.min(EDGE))
        .checked_mul(u64::from(doc.canvas.1.min(EDGE)))
        .filter(|n| *n != 0)
        .ok_or_else(blend::limit)?;
    checked()?;
    Ok(UnboundRootedFrameWorkspace {
        prepared_retained_bytes: program.numerical.retained_bytes,
        preparation_peak_bytes: preparation,
        spatial_workspace_bytes: spatial,
        blend_context_bytes: context,
        rooted_witness_bytes: rooted,
        seed_witness_bytes: seed,
        noise_sidecar_bytes: sidecar,
        workspace_bytes: workspace,
        native_rgba_bytes: cells.checked_mul(4).ok_or_else(blend::limit)?,
        display_cells: cells,
        display_accumulator_bytes: cells
            .checked_mul(size_of::<DisplayAccumulator>() as u64)
            .ok_or_else(blend::limit)?,
        query_control_bytes: CONTROLS,
        requires_noise: program.requires_noise,
    })
}

#[cfg(test)]
#[path = "frame_workspace/tests.rs"]
mod tests;
