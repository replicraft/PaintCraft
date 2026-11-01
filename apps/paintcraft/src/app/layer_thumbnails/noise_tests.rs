use super::*;
use paintcraft_core::types::{
    filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution},
    ids::deterministic_id,
};
use paintcraft_document::{FilterInstance, FilterStack, POLICY};
use std::time::Instant;
#[test]
fn actual_layer_thumbnail_worker_evaluates_noise_field_under_captured_single_layer_policy() {
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
        let mut doc =
            paintcraft_document::new_document(2, 1, Some(Rgba([64, 16, 128, 255])), 0, POLICY)
                .unwrap();
        let id = doc.roots[0];
        doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["layer thumbnail Noise"]),
            true,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(100, 65535, false, distribution).unwrap()),
        )])
        .unwrap();
        let source = Arc::new(doc);
        let before = source.clone();
        let ctx = egui::Context::default();
        let budget = paintcraft_runtime::SharedBudget::new(128 << 20);
        let mut service = LayerThumbnails::default();
        service.begin(&ctx, &source, &budget);
        service.request(&ctx, source.clone(), &[id], &budget);
        let (_, actual, handle) = service.pending.take().expect("thumbnail submitted");
        assert_eq!(actual, id);
        let deadline = Instant::now() + Duration::from_secs(10);
        let frame = loop {
            if let Some(result) = service.executor.as_mut().unwrap().poll_for(handle) {
                break result.unwrap();
            }
            assert!(Instant::now() < deadline, "Noise thumbnail stalled");
            std::thread::yield_now();
        };
        assert_eq!(frame.request.size(), [2, 1]);
        assert_eq!(frame.pixels, expected);
        assert!(source.same_semantics(&before));
        assert_eq!(source.layers[&id].raster.resident_tiles(), 0);
        assert_eq!(budget.usage().reserved, 0);
    }
}
