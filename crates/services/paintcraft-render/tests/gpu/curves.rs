use super::*;
use paintcraft_core::types::{
    ErrorCode,
    filters::{CurvesRgbV1, Descriptor, ExposureSrgbV1, LevelsRgbV1},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_render::{
    display::DisplayRequest,
    prepared::{prepare_document_programs, quote_document_programs},
    spatial::{WorkspaceAdmission, with_workspace},
};
fn inverse() -> CurvesRgbV1 {
    CurvesRgbV1::new([255, 191, 127, 63, 0])
}
fn descriptor(f: usize) -> Descriptor {
    match f {
        0 => Descriptor::LevelsRgbV1(LevelsRgbV1::default()),
        1 => Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(100).unwrap()),
        _ => Descriptor::CurvesRgbV1(inverse()),
    }
}
fn scene(f: usize) -> Document {
    let mut d = paintcraft_document::import_image(
        2,
        1,
        &[64, 128, 255, 255, 16, 10, 11, 255],
        "gpu-curves",
    )
    .unwrap();
    d.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        deterministic_id(&["gpu-curve-key"]),
        true,
        descriptor(f),
    )])
    .unwrap();
    d
}
fn prepared(d: &Document, b: &SharedBudget) -> WorkspaceAdmission {
    let q = quote_document_programs(d, &|| Ok(())).unwrap();
    prepare_document_programs(
        d,
        WorkspaceAdmission::new(
            b.scope(AllocationClass::SharedBudget),
            1 << 20,
            q.compile_work,
        ),
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn all_six_missing_family_directions_refuse_even_after_gpu_target_cache_is_warm() {
    let Some(adapter) = adapter() else { return };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    for from in 0..3 {
        for to in 0..3 {
            if from == to {
                continue;
            }
            let target = scene(to);
            let donor = scene(from);
            let b = SharedBudget::new(1 << 20);
            let good = prepared(&target, &b);
            with_workspace(good, || gpu.render(&target)).unwrap();
            let wrong = prepared(&donor, &b);
            let work = wrong.work_used();
            let live = b.usage().reserved;
            assert_eq!(
                with_workspace(wrong.clone(), || gpu.render(&target))
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
            assert_eq!(wrong.work_used(), work);
            assert_eq!(b.usage().reserved, live);
        }
    }
}
#[test]
fn curves_complete_keys_general_single_display_and_repeated_frames_reuse_one_job() {
    let Some(adapter) = adapter() else { return };
    for single in [false, true] {
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let gpu = GpuRenderer::new(device, queue)
            .unwrap()
            .with_single_layer_display(single);
        let d = scene(2);
        let before = d.clone();
        let b = SharedBudget::new(1 << 20);
        let a = prepared(&d, &b);
        let work = a.work_used();
        with_workspace(a.clone(), || {
            for _ in 0..2 {
                assert_eq!(gpu.render(&d)?, [191, 127, 0, 255, 239, 245, 244, 255]);
            }
            let view = DisplayRequest::new(&d, d.canvas_rect(), 0)?;
            assert_eq!(
                gpu.render_display_texture(&d, view, || Ok(()))?
                    .readback(|| Ok(()))?,
                [191, 127, 0, 255, 239, 245, 244, 255]
            );
            Ok(())
        })
        .unwrap();
        assert!(d.same_semantics(&before));
        assert_eq!(a.work_used(), work);
        let mut changed = d.clone();
        let id = d.roots[0];
        let f = d.layers[&id].filters.as_slice()[0].with_descriptor(Descriptor::CurvesRgbV1(
            CurvesRgbV1::new([255, 191, 127, 63, 1]),
        ));
        changed.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![f]).unwrap();
        assert_eq!(
            with_workspace(a.clone(), || gpu.render(&changed))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(a.work_used(), work);
        drop(a);
        assert_eq!(b.usage().reserved, 0);
    }
}
