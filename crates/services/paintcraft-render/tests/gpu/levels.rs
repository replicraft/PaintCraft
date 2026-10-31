use super::*;
use paintcraft_core::types::{
    ErrorCode, Rect,
    filters::{Descriptor, LevelsRgbV1},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_render::{
    display::DisplayRequest,
    prepared::{prepare_document_levels, quote_document_levels},
    spatial::{WorkspaceAdmission, with_workspace},
};
fn scene() -> Document {
    let mut doc =
        paintcraft_document::import_image(2, 1, &[1, 0, 4, 255, 4, 1, 0, 255], "gpu-levels")
            .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["gpu-levels"]),
            true,
            Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 4, 200, 0, 255).unwrap()),
        )])
        .unwrap();
    doc
}
fn factory(budget: &SharedBudget, work: u64) -> WorkspaceAdmission {
    WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 8 << 20, work)
}
#[test]
fn prepared_levels_reach_gpu_general_single_scaled_and_dirty_source_views_once() {
    let Some(adapter) = adapter() else { return };
    for single in [false, true] {
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let gpu = GpuRenderer::new(device, queue)
            .unwrap()
            .with_single_layer_display(single);
        let mut doc = scene();
        let before = doc.layers[&doc.roots[0]].raster.clone();
        let quote = quote_document_levels(&doc, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(8 << 20);
        let a = prepare_document_levels(&doc, factory(&budget, quote.compile_work), &|| Ok(()))
            .unwrap();
        with_workspace(a.clone(), || {
            assert_eq!(gpu.render(&doc)?, [128, 0, 255, 255, 255, 128, 0, 255]);
            for (level, expected) in [
                (0, vec![128, 0, 255, 255, 255, 128, 0, 255]),
                (1, vec![192, 64, 128, 255]),
            ] {
                let view = DisplayRequest::new(&doc, doc.canvas_rect(), level)?;
                assert_eq!(
                    gpu.render_display_texture(&doc, view, || Ok(()))?
                        .readback(|| Ok(()))?,
                    expected
                );
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(a.work_used(), quote.compile_work);
        assert_eq!(doc.layers[&doc.roots[0]].raster, before);
        let id = doc.roots[0];
        doc.layers
            .get_mut(&id)
            .unwrap()
            .raster
            .fill_rect(
                &Rect::new(0, 0, 1, 1).unwrap(),
                Rgba([4, 0, 1, 255]),
                paintcraft_raster::BlendMode::Replace,
                &mut Admission::new(1 << 20),
            )
            .unwrap();
        with_workspace(a.clone(), || {
            assert_eq!(gpu.render(&doc)?, [255, 0, 128, 255, 255, 128, 0, 255]);
            Ok(())
        })
        .unwrap();
        assert_eq!(a.work_used(), quote.compile_work);
        drop(a);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn gpu_cached_and_uniform_paths_require_correct_prepared_inventory() {
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let mut doc = scene();
    let budget = SharedBudget::new(8 << 20);
    let a = prepare_document_levels(&doc, factory(&budget, u64::MAX), &|| Ok(())).unwrap();
    with_workspace(a.clone(), || gpu.render(&doc)).unwrap();
    assert_eq!(
        gpu.render(&doc).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let before = (doc.document_id, doc.state_id);
    let id = doc.roots[0];
    doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        deterministic_id(&["gpu-levels"]),
        true,
        Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 4, 200, 73, 73).unwrap()),
    )])
    .unwrap();
    assert_eq!((doc.document_id, doc.state_id), before);
    assert_eq!(
        with_workspace(a.clone(), || gpu.render(&doc))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    drop(a);
    assert_eq!(budget.usage().reserved, 0);
    let mut uniform = new_document(2, 1, Some(Rgba([1, 0, 4, 255])), 0, POLICY).unwrap();
    uniform.layers.get_mut(&uniform.roots[0]).unwrap().filters =
        scene().layers.values().next().unwrap().filters.clone();
    let a = factory(&budget, u64::MAX);
    let request = DisplayRequest::new(&uniform, uniform.canvas_rect(), 1).unwrap();
    with_workspace(a, || {
        assert_eq!(
            gpu.render_display_texture(&uniform, request, || Ok(()))?
                .readback(|| Ok(()))?,
            [128, 0, 255, 255]
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.usage().reserved, 0);
}
