use super::*;
use paintcraft_render::display::{DisplayRequest, render_display_cancellable};

#[test]
fn sampling_level_changes_retire_preview_progress_and_preserve_authoritative_pixels() {
    let mut app = App::new(None);
    let ctx = egui::Context::default();
    let original = paintcraft_render::render_flat(app.editor.document()).unwrap();
    app.editor.select_tool(Tool::Brush);
    app.editor.pointer_down((41, 39));
    app.sync_texture(&ctx);
    let old = app.presentation_key();
    app.editor.pointer_move((48, 44));
    app.render_level = 2;
    app.sync_texture(&ctx);
    let key = app.presentation_key();
    assert_eq!(key.3, old.3);
    assert!(!app.accept_render_key(old));
    assert_eq!(app.rendered_size(), Some([250, 122]));
    assert_eq!(app.rendered_sample_step(), Some(4));
    let staged = paintcraft_editor::execute::execute(
        app.editor.document(),
        app.editor.selection(),
        &app.editor.preview_batch().unwrap().unwrap(),
    )
    .unwrap();
    let request = DisplayRequest::new(&staged.doc, key.3, 2).unwrap();
    let expected = render_display_cancellable(&staged.doc, request, || Ok(()))
        .unwrap()
        .pixels;
    assert_eq!(app.rendered, expected);
    let visible = app.rendered.clone();
    app.install_render(&ctx, old, Ok(original.clone()));
    assert_eq!(app.rendered, visible);
    app.editor.cancel_gesture();
    app.render_level = 0;
    app.sync_texture(&ctx);
    assert_eq!(app.rendered, original);
}

#[test]
fn poisoned_display_cache_falls_back_to_bounded_exact_filtering() {
    let renderer = cpu_rendering::CpuRenderer::new();
    let failing = renderer.clone();
    assert!(
        std::thread::spawn(move || {
            let _guard = failing.2.lock().unwrap();
            panic!("injected display cache panic")
        })
        .join()
        .is_err()
    );
    let doc = paintcraft_document::new_document(
        73,
        51,
        Some(Rgba([17, 37, 53, 127])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let request = DisplayRequest::new(&doc, doc.canvas_rect(), 3).unwrap();
    let expected = render_display_cancellable(&doc, request, || Ok(()))
        .unwrap()
        .pixels;
    for _ in 0..2 {
        assert_eq!(
            renderer
                .render_display(&doc, request.region(), 3, false, || Ok(()))
                .unwrap(),
            expected
        );
    }
    assert!(renderer.2.lock().unwrap().is_none());
}

#[test]
fn admitted_giant_display_uses_reduced_output_without_disabling_the_native_gpu() {
    let Some((target, gpu)) = gpu_display::tests::target() else {
        return;
    };
    let mut app = App::new(None);
    let doc = paintcraft_document::new_document(
        1_048_576,
        1_048_576,
        Some(Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    app.editor = Editor::with_document(doc, None);
    app.gpu = Some(gpu.clone());
    app.gpu_target = Some(target);
    app.renderer = Some(
        Executor::new(WorkerLimits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 1 << 30,
        })
        .unwrap(),
    );
    app.render_level = 10;
    let ctx = egui::Context::default();
    let key = app.presentation_key();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app.texture_key != Some(key) {
        assert!(std::time::Instant::now() < deadline);
        app.sync_texture(&ctx);
        std::thread::yield_now();
    }
    assert_eq!(app.rendered_size(), Some([1024, 1024]));
    assert_eq!(app.rendered, vec![255; 1024 * 1024 * 4]);
    assert!(app.gpu_target.is_some());
    assert!(Arc::ptr_eq(app.gpu.as_ref().unwrap(), &gpu));
    assert!(app.gpu_texture.is_none());
    assert_eq!(app.editor.publications(), 0);
}
