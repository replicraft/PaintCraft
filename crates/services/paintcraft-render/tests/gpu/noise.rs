//! Same source-coordinate CPU appearance upload on real general/single GPU paths.
use super::*;
use paintcraft_core::types::{
    ErrorCode,
    filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_render::noise_work::{
    NoiseAdmission, NoiseJobPolicy, NoiseLimits, with_noise_workspace,
};
#[test]
fn noise_general_and_single_upload_literal_field_cache_and_parent_guards() {
    let Some(adapter) = adapter() else { return };
    for single in [false, true] {
        for (distribution, expected) in [
            (
                NoiseDistribution::Uniform,
                [255, 211, 0, 255, 51, 0, 97, 255],
            ),
            (
                NoiseDistribution::Gaussian,
                [47, 139, 0, 255, 0, 4, 46, 255],
            ),
        ] {
            let (device, queue) =
                pollster::block_on(adapter.request_device(&Default::default())).unwrap();
            let gpu = GpuRenderer::new(device, queue)
                .unwrap()
                .with_single_layer_display(single);
            let mut doc = paintcraft_document::new_document(
                2,
                1,
                Some(Rgba([64, 16, 128, 255])),
                0,
                paintcraft_document::POLICY,
            )
            .unwrap();
            let id = doc.roots[0];
            doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
                deterministic_id(&["noise-upload"]),
                true,
                Descriptor::AddNoiseRgbV1(
                    AddNoiseRgbV1::new(100, 65535, false, distribution).unwrap(),
                ),
            )])
            .unwrap();
            let before = doc.clone();
            let b = SharedBudget::new(64 << 20);
            for _ in 0..2 {
                let a = NoiseAdmission::new_root(
                    b.scope(AllocationClass::SharedBudget),
                    NoiseJobPolicy::BrowserV1,
                    NoiseLimits {
                        bytes: 64 << 20,
                        base_work: 268_435_456,
                        noise_work: 1_000_000,
                    },
                )
                .unwrap();
                assert_eq!(
                    with_noise_workspace(a, || gpu.render(&doc)).unwrap(),
                    expected
                );
            }
            assert_eq!(
                gpu.render(&doc).unwrap_err().code,
                ErrorCode::CapabilityUnsupported
            );
            assert!(doc.same_semantics(&before));
            assert_eq!(doc.layers[&id].raster.resident_tiles(), 0);
            let layer = doc.layers.get_mut(&id).unwrap();
            layer.filters = FilterStack::new(vec![layer.filters.as_slice()[0].with_descriptor(
                Descriptor::AddNoiseRgbV1(
                    AddNoiseRgbV1::new(0, 65535, false, distribution).unwrap(),
                ),
            )])
            .unwrap();
            assert_eq!(doc.state_id, before.state_id);
            let a = NoiseAdmission::new_root(
                b.scope(AllocationClass::SharedBudget),
                NoiseJobPolicy::BrowserV1,
                NoiseLimits {
                    bytes: 64 << 20,
                    base_work: 268_435_456,
                    noise_work: 1_000_000,
                },
            )
            .unwrap();
            assert_eq!(
                with_noise_workspace(a, || gpu.render(&doc)).unwrap(),
                [64, 16, 128, 255, 64, 16, 128, 255]
            );
        }
    }
}
