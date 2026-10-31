use super::*;
use paintcraft_core::types::{
    ErrorCode,
    filters::{Descriptor, ExposureSrgbV1, LevelsRgbV1},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_render::{
    display::DisplayRequest,
    prepared::{prepare_document_programs, quote_document_programs},
    spatial::{WorkspaceAdmission, with_workspace},
};
fn scene() -> Document {
    let mut doc = paintcraft_document::import_image(
        2,
        1,
        &[64, 128, 255, 255, 16, 10, 11, 255],
        "gpu-exposure",
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::new(vec![
        FilterInstance::new(
            deterministic_id(&["gpu-exposure-levels"]),
            true,
            Descriptor::LevelsRgbV1(LevelsRgbV1::default()),
        ),
        FilterInstance::new(
            deterministic_id(&["gpu-exposure"]),
            true,
            Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(100).unwrap()),
        ),
    ])
    .unwrap();
    doc
}
fn factory(b: &SharedBudget, work: u64) -> WorkspaceAdmission {
    WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 8 << 20, work)
}
#[test]
fn exposure_generic_owner_reaches_general_single_scaled_and_cached_gpu_paths_once() {
    let Some(adapter) = adapter() else { return };
    for single in [false, true] {
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let gpu = GpuRenderer::new(device, queue)
            .unwrap()
            .with_single_layer_display(single);
        let doc = scene();
        let before = doc.clone();
        let q = quote_document_programs(&doc, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(8 << 20);
        let a =
            prepare_document_programs(&doc, factory(&budget, q.compile_work), &|| Ok(())).unwrap();
        with_workspace(a.clone(), || {
            for _ in 0..2 {
                assert_eq!(gpu.render(&doc)?, [90, 176, 255, 255, 26, 18, 19, 255]);
            }
            for (level, expected) in [
                (0, vec![90, 176, 255, 255, 26, 18, 19, 255]),
                (1, vec![58, 97, 137, 255]),
            ] {
                let request = DisplayRequest::new(&doc, doc.canvas_rect(), level)?;
                assert_eq!(
                    gpu.render_display_texture(&doc, request, || Ok(()))?
                        .readback(|| Ok(()))?,
                    expected
                );
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(a.work_used(), q.compile_work);
        assert!(doc.same_semantics(&before));
        assert_eq!(
            gpu.render(&doc).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        let mut changed = doc.clone();
        let id = changed.roots[0];
        let mut filters = changed.layers[&id].filters.as_slice().to_vec();
        filters[1] = filters[1].with_descriptor(Descriptor::ExposureSrgbV1(
            ExposureSrgbV1::new(101).unwrap(),
        ));
        changed.layers.get_mut(&id).unwrap().filters = FilterStack::new(filters).unwrap();
        assert_eq!(
            with_workspace(a.clone(), || gpu.render(&changed))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(a.work_used(), q.compile_work);
        drop(a);
        assert_eq!(budget.usage().reserved, 0);
    }
}
