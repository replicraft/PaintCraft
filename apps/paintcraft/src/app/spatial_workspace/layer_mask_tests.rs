use super::*;
use crate::{App, controller::Editor};
use paintcraft_core::types::{ErrorCode, Rect};
use paintcraft_document::LayerMask;
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use std::time::{Duration, Instant};

fn masked() -> Document {
    let mut doc = paintcraft_document::import_image(
        3,
        1,
        &[90, 60, 30, 128, 110, 80, 50, 255, 150, 120, 90, 255],
        "native-layer-mask/v1",
    )
    .unwrap();
    let mask = A8Mask::uniform(
        Rect::new(0, 0, 2, 1).unwrap(),
        128,
        &SharedBudget::new(4096),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().mask = Some(LayerMask::new(true, mask));
    doc
}
const EXPECTED: [u8; 12] = [90, 60, 30, 64, 110, 80, 50, 128, 0, 0, 0, 0];

#[test]
fn native_mask_only_uses_the_existing_job_realm_and_restores_context() {
    let mut doc = masked();
    assert!(needed(&doc));
    assert!(needed_layer(&doc.layers[&doc.roots[0]]));
    let policy = JobPolicy::capture(&doc, None, needed(&doc));
    assert_eq!(policy, JobPolicy::Legacy);
    assert_eq!(
        policy.run(|| paintcraft_render::render_flat(&doc)).unwrap(),
        EXPECTED
    );
    assert_eq!(
        paintcraft_render::render_flat(&doc).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let private = SharedBudget::new(BYTES);
    assert_eq!(
        with_admission(private.clone(), || paintcraft_render::render_flat(&doc)).unwrap(),
        EXPECTED
    );
    assert_eq!(private.usage().reserved, 0);
    let no_work = WorkspaceAdmission::new(private.scope(AllocationClass::SharedBudget), BYTES, 0);
    assert_eq!(
        with_workspace(no_work, || paintcraft_render::render_flat(&doc))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit,
    );
    assert_eq!(private.usage().reserved, 0);
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.mask = Some(layer.mask.as_ref().unwrap().with_enabled(false));
    assert!(!needed(&doc));
    assert_eq!(
        JobPolicy::capture(&doc, None, needed(&doc)),
        JobPolicy::None
    );
    assert_eq!(
        paintcraft_render::render_flat(&doc).unwrap(),
        [90, 60, 30, 128, 110, 80, 50, 255, 150, 120, 90, 255]
    );
    doc.layers.get_mut(&doc.roots[0]).unwrap().mask = None;
    assert!(!needed(&doc));
}

#[test]
fn real_native_canvas_mask_only_matches_in_sync_and_background_jobs() {
    for asynchronous in [false, true] {
        let ctx = eframe::egui::Context::default();
        let source = masked();
        let mut app = App::new(None);
        app.editor = Editor::with_document(source.clone(), None);
        if asynchronous {
            app.renderer = Some(
                paintcraft_runtime::Executor::with_budget(
                    paintcraft_runtime::Limits {
                        workers: 1,
                        pending_jobs: 2,
                        transient_bytes: 1 << 30,
                    },
                    app.worker_budget.clone(),
                )
                .unwrap(),
            );
        }
        app.sync_texture(&ctx);
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.texture_key != Some(app.presentation_key()) {
            assert!(Instant::now() < deadline, "{}", app.editor.status());
            app.sync_texture(&ctx);
            std::thread::yield_now();
        }
        assert_eq!(app.rendered, EXPECTED);
        assert!(app.editor.document().same_semantics(&source));
        assert_eq!(app.editor.undo_depth(), 0);
        if asynchronous {
            assert_eq!(app.renderer.as_ref().unwrap().reserved_bytes(), 0);
        }
    }
}
