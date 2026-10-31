use super::*;
use paintcraft_core::types::{
    ErrorCode,
    filters::{CurvesRgbV1, Descriptor, VibranceRgbV1},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_render::{
    prepared,
    spatial::{WorkspaceAdmission, with_workspace},
};
fn scene() -> Document {
    let mut d = paintcraft_document::import_image(
        2,
        1,
        &[20, 115, 200, 255, 64, 16, 128, 255],
        "gpu-direct-v",
    )
    .unwrap();
    d.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        deterministic_id(&["gpu-v"]),
        true,
        Descriptor::VibranceRgbV1(VibranceRgbV1::new(100).unwrap()),
    )])
    .unwrap();
    d
}
fn factory(b: &SharedBudget) -> WorkspaceAdmission {
    WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, 1_000_000)
}
#[test]
fn admitted_vibrance_cpu_appearance_upload_has_literal_pixels_and_warm_cache_needs_parent() {
    let Some(adapter) = adapter() else { return };
    for single in [false, true] {
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let gpu = GpuRenderer::new(device, queue)
            .unwrap()
            .with_single_layer_display(single);
        let d = scene();
        let frozen = d.clone();
        let b = SharedBudget::new(1 << 20);
        let a = factory(&b);
        assert_eq!(
            with_workspace(a.clone(), || gpu.render(&d)).unwrap(),
            [4, 116, 216, 255, 62, 4, 140, 255]
        );
        assert!(a.work_used() > 0);
        let used = a.work_used();
        assert_eq!(
            gpu.render(&d).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(a.work_used(), used);
        assert_eq!(
            with_workspace(a.clone(), || gpu.render(&d)).unwrap(),
            [4, 116, 216, 255, 62, 4, 140, 255]
        );
        assert!(d.same_semantics(&frozen));
    }
}
#[test]
fn nonlinear_curves_and_direct_vibrance_share_one_parent_without_an_extra_prepared_key() {
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let mut d = paintcraft_document::import_image(1, 1, &[64, 16, 128, 255], "curves-v").unwrap();
    d.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(vec![
        FilterInstance::new(
            deterministic_id(&["curve-v-a"]),
            true,
            Descriptor::CurvesRgbV1(CurvesRgbV1::new([23, 9, 240, 73, 201])),
        ),
        FilterInstance::new(
            deterministic_id(&["curve-v-b"]),
            true,
            Descriptor::VibranceRgbV1(VibranceRgbV1::new(100).unwrap()),
        ),
    ])
    .unwrap();
    let b = SharedBudget::new(1 << 20);
    let q = prepared::quote_document_programs(&d, &|| Ok(())).unwrap();
    assert_eq!(q.unique_programs, 1);
    let a = prepared::prepare_document_programs(&d, factory(&b), &|| Ok(())).unwrap();
    let used = a.work_used();
    assert_eq!(
        with_workspace(a.clone(), || gpu.render(&d)).unwrap(),
        [1, 6, 248, 255]
    );
    assert!(a.work_used() > used);
    assert_eq!(a.prepared_curves().unwrap().len(), 1);
}

#[test]
fn scaled_fractional_mask_proofs_do_not_spend_more_than_the_tight_direct_bound() {
    use paintcraft_core::types::Rect;
    use paintcraft_document::FilterMask;
    use paintcraft_render::display::DisplayRequest;
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let mut d = scene();
    let id = d.roots[0];
    d.layers.get_mut(&id).unwrap().raster =
        paintcraft_raster::RasterLayer::new(2, 1, Rgba([20, 115, 200, 255]));
    let b = SharedBudget::new(1 << 20);
    let mask = paintcraft_raster::coverage_mask::A8Mask::uniform(
        Rect::new(0, 0, 1, 1).unwrap(),
        127,
        &b,
        Default::default(),
        &|| Ok(()),
    )
    .unwrap();
    let f = d.layers[&id].filters.as_slice()[0].with_mask(Some(FilterMask::new(true, mask)));
    d.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![f]).unwrap();
    let frozen = d.clone();
    let request = DisplayRequest::new(&d, d.canvas_rect(), 1).unwrap();
    let a = WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, 256);
    let frame = with_workspace(a.clone(), || {
        gpu.render_display_texture(&d, request, || Ok(()))
    })
    .unwrap();
    assert_eq!(frame.readback(|| Ok(())).unwrap(), [16, 115, 204, 255]);
    assert!(a.work_used() <= 256);
    let a = WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, 255);
    assert_eq!(
        with_workspace(a.clone(), || gpu.render_display_texture(
            &d,
            request,
            || Ok(())
        ))
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(a.work_used(), 0);
    assert!(d.same_semantics(&frozen));
}
