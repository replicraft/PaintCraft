use super::*;
use crate::files::blend_adoption_tests::source;
use paintcraft_core::types::LayerBlendMode;
use std::time::Instant;

#[test]
fn actual_isolated_layer_thumbnail_job_retains_mask_but_excludes_opacity_and_backdrop() {
    let ctx = egui::Context::default();
    for mode in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        let doc = Arc::new(source(mode, 2, 1, true));
        let raw = doc.clone();
        let id = *doc.roots.last().unwrap();
        let budget = paintcraft_runtime::SharedBudget::new(128 << 20);
        let mut service = LayerThumbnails::default();
        service.begin(&ctx, &doc, &budget);
        service.request(&ctx, doc.clone(), &[id], &budget);
        let (_, pending_layer, handle) = service.pending.expect("real admitted layer job");
        assert_eq!(pending_layer, id);
        let deadline = Instant::now() + Duration::from_secs(20);
        let frame = loop {
            if let Some(result) = service.executor.as_mut().unwrap().poll_for(handle) {
                break result.unwrap();
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        };
        service.pending = None;
        assert_eq!(frame.request.size(), [2, 1]);
        // Isolation forces visibility/opacity255; both blends over transparency
        // preserve sourceRGB. Own mask128 remains exactly once in alpha.
        assert_eq!(frame.pixels, [204, 77, 13, 128].repeat(2));
        assert!(doc.same_semantics(&raw));
        drop(frame);
        drop(service);
        assert_eq!(budget.usage().reserved, 0);
    }
}
