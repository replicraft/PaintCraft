//! Stored blend modes never acquire a GPU execution grant from a None caller.
use super::*;
use paintcraft_core::types::{ErrorCode, LayerBlendMode};
use paintcraft_render::display::DisplayRequest;
use std::cell::Cell;
#[test]
fn gpu_all_public_paths_refuse_modes_before_callbacks_or_source_allocation() {
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    for mode in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        for (visible, opacity) in [(true, 255), (false, 255), (true, 0)] {
            let mut doc = new_document(1, 1, Some(Rgba([17, 32, 255, 128])), 0, POLICY).unwrap();
            let id = doc.roots[0];
            let layer = doc.layers.get_mut(&id).unwrap();
            layer.blend_mode = mode;
            layer.visible = visible;
            layer.opacity = opacity;
            let usage = gpu.source_cache_usage();
            let checks = Cell::new(0);
            let request = DisplayRequest::new(&doc, doc.canvas_rect(), 1).unwrap();
            let verify = || {
                assert_eq!(
                    gpu.render_cancellable(&doc, || {
                        checks.set(checks.get() + 1);
                        Ok(())
                    })
                    .err()
                    .unwrap()
                    .code,
                    ErrorCode::CapabilityUnsupported
                );
                assert_eq!(
                    gpu.render_region_texture(&doc, doc.canvas_rect(), || {
                        checks.set(checks.get() + 1);
                        Ok(())
                    })
                    .err()
                    .unwrap()
                    .code,
                    ErrorCode::CapabilityUnsupported
                );
                assert_eq!(
                    gpu.render_display_texture(&doc, request, || {
                        checks.set(checks.get() + 1);
                        Ok(())
                    })
                    .err()
                    .unwrap()
                    .code,
                    ErrorCode::CapabilityUnsupported
                );
                assert_eq!(checks.get(), 0);
                assert_eq!(gpu.source_cache_usage(), usage);
            };
            verify();
            assert_eq!(gpu.source_cache_usage(), usage);
        }
    }
}
