//! Independent literal framebuffer expectations; CPU rendering is not an oracle.
//! render_display_texture().readback() returns PREMULTIPLIED encoded-sRGB display
//! bytes. CPU document/export RGBA and alpha-weighted CPU LOD remain STRAIGHT.
use super::*;
use paintcraft_core::types::{
    Rect,
    filters::{AddNoiseRgbV1, BoxBlurV1, Descriptor, NoiseDistribution},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack, LayerMask};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use paintcraft_render::{
    display::DisplayRequest,
    spatial::{WorkspaceAdmission, with_workspace},
};
fn mask(x: i64, enabled: bool) -> LayerMask {
    LayerMask::new(
        enabled,
        A8Mask::uniform(
            Rect::new(x, 0, 1, 1).unwrap(),
            128,
            &SharedBudget::new(8192),
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap(),
    )
}
fn scene() -> Document {
    let mut d = new_document(2, 1, Some(Rgba([20, 115, 200, 128])), 0, POLICY).unwrap();
    let id = d.roots[0];
    let l = d.layers.get_mut(&id).unwrap();
    l.mask = Some(mask(0, true));
    l.opacity = 128;
    d
}
fn admitted<T>(f: impl FnOnce() -> paintcraft_core::types::Result<T>) -> T {
    let b = SharedBudget::new(4 << 20);
    with_workspace(
        WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 4 << 20, 10_000_000),
        f,
    )
    .unwrap()
}
#[test]
fn layer_mask_alpha_precedes_gpu_opacity_general_and_single_source_keys_include_mask() {
    let Some(a) = adapter() else { return };
    for single in [false, true] {
        let (device, queue) = pollster::block_on(a.request_device(&Default::default())).unwrap();
        let gpu = GpuRenderer::new(device, queue)
            .unwrap()
            .with_single_layer_display(single);
        let mut d = scene();
        let id = d.roots[0];
        let state = d.state_id;
        admitted(|| {
            let req = DisplayRequest::new(&d, d.canvas_rect(), 0)?;
            assert_eq!(
                gpu.render_display_texture(&d, req, || Ok(()))?
                    .readback(|| Ok(()))?,
                [3, 14, 25, 32, 0, 0, 0, 0]
            );
            let hit = gpu.render_display_texture(&d, req, || Ok(()))?;
            assert!(hit.stats.source_hits > 0);
            assert_eq!(hit.readback(|| Ok(()))?, [3, 14, 25, 32, 0, 0, 0, 0]);
            d.layers.get_mut(&id).unwrap().mask = Some(mask(1, true));
            let changed = gpu.render_display_texture(&d, req, || Ok(()))?;
            assert!(changed.stats.source_allocations > 0);
            assert_eq!(changed.readback(|| Ok(()))?, [0, 0, 0, 0, 3, 14, 25, 32]);
            d.layers.get_mut(&id).unwrap().mask = Some(mask(1, false));
            let disabled = gpu.render_display_texture(&d, req, || Ok(()))?;
            assert!(disabled.stats.source_allocations > 0);
            assert_eq!(
                disabled.readback(|| Ok(()))?,
                [5, 29, 50, 64, 5, 29, 50, 64]
            );
            Ok(())
        });
        assert_eq!(d.state_id, state);
        assert_eq!(d.layers[&id].raster.resident_tiles(), 0);
    }
}
#[test]
fn layer_mask_implicit_default_is_applied_per_native_pixel_before_gpu_lod() {
    let Some(a) = adapter() else { return };
    let (device, queue) = pollster::block_on(a.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let d = scene();
    admitted(|| {
        let req = DisplayRequest::new(&d, d.canvas_rect(), 1)?;
        assert_eq!(
            gpu.render_display_texture(&d, req, || Ok(()))?
                .readback(|| Ok(()))?,
            [1, 7, 13, 16]
        );
        Ok(())
    });
}
#[test]
fn layer_mask_is_after_noise_and_complete_box_prefix_on_the_same_paired_parent() {
    let Some(a) = adapter() else { return };
    let (device, queue) = pollster::block_on(a.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let mut d = new_document(2, 1, Some(Rgba([64, 16, 128, 255])), 0, POLICY).unwrap();
    let id = d.roots[0];
    let l = d.layers.get_mut(&id).unwrap();
    l.mask = Some(mask(0, true));
    l.opacity = 128;
    l.filters = FilterStack::new(vec![
        FilterInstance::new(
            deterministic_id(&["layer-mask-noise"]),
            true,
            Descriptor::AddNoiseRgbV1(
                AddNoiseRgbV1::new(100, 65535, false, NoiseDistribution::Uniform).unwrap(),
            ),
        ),
        FilterInstance::new(
            deterministic_id(&["layer-mask-box"]),
            true,
            Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
        ),
    ])
    .unwrap();
    let b = SharedBudget::new(4 << 20);
    let a = paintcraft_render::noise_work::NoiseAdmission::new_root(
        b.scope(AllocationClass::SharedBudget),
        paintcraft_render::noise_work::NoiseJobPolicy::BrowserV1,
        paintcraft_render::noise_work::NoiseLimits {
            bytes: 4 << 20,
            base_work: 1_000_000,
            noise_work: 1_000_000,
        },
    )
    .unwrap();
    paintcraft_render::noise_work::with_noise_workspace(a, || {
        let req = DisplayRequest::new(&d, d.canvas_rect(), 0)?;
        // Independent Noise [255,211,0]/[51,0,97], edge-clamped Box1 then alpha128 and opacity128.
        assert_eq!(
            gpu.render_display_texture(&d, req, || Ok(()))?
                .readback(|| Ok(()))?,
            [47, 35, 8, 64, 0, 0, 0, 0]
        );
        let req = DisplayRequest::new(&d, d.canvas_rect(), 1)?;
        assert_eq!(
            gpu.render_display_texture(&d, req, || Ok(()))?
                .readback(|| Ok(()))?,
            [23, 18, 4, 32]
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(d.layers[&id].raster.resident_tiles(), 0);
}
