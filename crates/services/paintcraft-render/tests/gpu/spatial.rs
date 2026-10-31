use super::*;
use paintcraft_core::types::{
    Rect,
    filters::{BoxBlurV1, Descriptor},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_raster::BlendMode;
use paintcraft_render::{
    display::DisplayRequest,
    spatial::{WorkspaceAdmission, with_workspace},
};

fn stack() -> FilterStack {
    FilterStack::new(vec![FilterInstance::new(
        deterministic_id(&["gpu-spatial"]),
        true,
        Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
    )])
    .unwrap()
}
fn job<T>(budget: &SharedBudget, render: impl FnOnce() -> paintcraft_core::types::Result<T>) -> T {
    with_workspace(
        WorkspaceAdmission::new(
            budget.scope(AllocationClass::SharedBudget),
            8 << 20,
            u64::MAX,
        ),
        render,
    )
    .unwrap()
}
#[test]
fn admitted_box_reaches_general_single_and_scaled_gpu_sources_before_reduction() {
    let Some(adapter) = adapter() else { return };
    for single in [false, true] {
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let gpu = GpuRenderer::new(device, queue)
            .unwrap()
            .with_single_layer_display(single);
        let mut doc = new_document(2, 1, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
        let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
        layer.raster = RasterLayer::from_rgba(
            2,
            1,
            Rgba::TRANSPARENT,
            &[255, 0, 0, 255, 0, 255, 0, 0],
            &mut Admission::new(1 << 20),
        )
        .unwrap();
        layer.filters = stack();
        let original = layer.raster.clone();
        let budget = SharedBudget::new(8 << 20);
        job(&budget, || {
            assert_eq!(gpu.render(&doc)?, [255, 0, 0, 170, 255, 0, 0, 85]);
            // Display surfaces are premultiplied; ordinary render/export above
            // remains straight RGBA. These literals follow alpha170/85/128.
            for (level, expected) in [
                (0, vec![170, 0, 0, 170, 85, 0, 0, 85]),
                (1, vec![128, 0, 0, 128]),
            ] {
                let request = DisplayRequest::new(&doc, doc.canvas_rect(), level)?;
                assert_eq!(
                    gpu.render_display_texture(&doc, request, || Ok(()))?
                        .readback(|| Ok(()))?,
                    expected,
                    "single={single}, level={level}"
                );
            }
            Ok(())
        });
        assert_eq!(doc.layers[&doc.roots[0]].raster, original);
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn halo_only_raster_changes_cannot_reuse_or_patch_an_incomplete_gpu_dependency_key() {
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let mut doc = new_document(300, 1, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let layer = doc.layers.get_mut(&id).unwrap();
    layer.filters = stack();
    layer
        .raster
        .fill_rect(
            &Rect::new(255, 0, 1, 1).unwrap(),
            Rgba([255, 0, 0, 255]),
            BlendMode::Replace,
            &mut Admission::new(1 << 20),
        )
        .unwrap();
    let roi = Rect::new(256, 0, 1, 1).unwrap();
    let identity = (doc.document_id, doc.state_id);
    let budget = SharedBudget::new(8 << 20);
    job(&budget, || {
        let frame = gpu.render_region_with_stats(&doc, roi, || Ok(()))?;
        assert_eq!(frame.pixels, [255, 0, 0, 85]);
        assert_eq!(frame.stats.source_hits, 0);
        Ok(())
    });
    doc.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .fill_rect(
            &Rect::new(255, 0, 1, 1).unwrap(),
            Rgba([0, 255, 0, 255]),
            BlendMode::Replace,
            &mut Admission::new(1 << 20),
        )
        .unwrap();
    assert_eq!((doc.document_id, doc.state_id), identity);
    job(&budget, || {
        let frame = gpu.render_region_with_stats(&doc, roi, || Ok(()))?;
        assert_eq!(frame.pixels, [0, 255, 0, 85]);
        assert_eq!(frame.stats.source_hits, 0);
        assert_eq!(frame.stats.patched_sources, 0);
        assert!(frame.stats.source_allocations > 0);
        Ok(())
    });
    assert_eq!(budget.usage().reserved, 0);
}
