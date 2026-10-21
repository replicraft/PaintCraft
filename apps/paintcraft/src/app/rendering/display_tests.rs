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
    for gpu_scaled in [false, true] {
        admitted_giant_display(gpu_scaled);
    }
}
fn admitted_giant_display(gpu_scaled: bool) {
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
    app.gpu_scaled_display = gpu_scaled;
    let ctx = egui::Context::default();
    let key = app.presentation_key();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app.texture_key != Some(key) {
        assert!(std::time::Instant::now() < deadline);
        app.sync_texture(&ctx);
        std::thread::yield_now();
    }
    assert_eq!(app.rendered_size(), Some([1024, 1024]));
    if gpu_scaled {
        assert!(app.rendered.is_empty());
        assert_eq!(
            app.gpu_texture
                .as_ref()
                .unwrap()
                .frame()
                .readback(|| Ok(()))
                .unwrap(),
            vec![255; 1024 * 1024 * 4]
        );
    } else {
        assert_eq!(app.rendered, vec![255; 1024 * 1024 * 4]);
        assert!(app.gpu_texture.is_none());
    }
    assert!(app.gpu_target.is_some());
    assert!(Arc::ptr_eq(app.gpu.as_ref().unwrap(), &gpu));
    assert_eq!(app.gpu_texture.is_some(), gpu_scaled);
    assert_eq!(app.editor.publications(), 0);
}

#[test]
fn scaled_storage_limit_falls_back_to_uploadable_cpu_pixels_and_keeps_native_gpu_working() {
    let limits = eframe::wgpu::Limits {
        max_storage_buffer_binding_size: 16384,
        ..Default::default()
    };
    let Some((target, gpu)) = gpu_display::tests::target_with_limits(0, 32, Some(limits)) else {
        return;
    };
    let ctx = egui::Context::default();
    let mut initial = ctx.run_ui(Default::default(), |ui| {
        ui.label("fallback upload initialization");
    });
    gpu_display::benchmark::upload(&target, &mut initial.textures_delta);
    let mut doc = paintcraft_document::new_document(
        129,
        65,
        Some(Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let id = doc.roots[0];
    doc.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .blend_pixel(
            7,
            9,
            Rgba([0, 0, 0, 255]),
            paintcraft_raster::BlendMode::Replace,
            &mut paintcraft_raster::Admission::new(65536),
        )
        .unwrap();
    let request = DisplayRequest::new(&doc, doc.canvas_rect(), 1).unwrap();
    let expected = render_display_cancellable(&doc, request, || Ok(()))
        .unwrap()
        .pixels;
    let mut app = App::new(None);
    app.editor = Editor::with_document(doc, None);
    app.gpu = Some(gpu.clone());
    app.gpu_target = Some(target.clone());
    app.render_level = 1;
    app.gpu_scaled_display = true;
    app.renderer = Some(
        Executor::new(WorkerLimits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 1 << 30,
        })
        .unwrap(),
    );
    let key = app.presentation_key();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app.texture_key != Some(key) {
        assert!(std::time::Instant::now() < deadline);
        app.sync_texture(&ctx);
        std::thread::yield_now();
    }
    assert!(app.gpu_texture.is_none());
    assert!(app.texture.is_some());
    assert_eq!(app.rendered, expected);
    assert!(Arc::ptr_eq(app.gpu.as_ref().unwrap(), &gpu));
    assert!(app.gpu_target.is_some());
    let mut delta = ctx.tex_manager().write().take_delta();
    gpu_display::benchmark::upload(&target, &mut delta);
    app.render_region = paintcraft_core::types::Rect::new(1, 1, 17, 19);
    app.render_level = 0;
    let key = app.presentation_key();
    while app.texture_key != Some(key) {
        assert!(std::time::Instant::now() < deadline);
        app.sync_texture(&ctx);
        std::thread::yield_now();
    }
    assert!(app.gpu_texture.is_some());
    assert!(app.rendered.is_empty());
    let mut delta = ctx.tex_manager().write().take_delta();
    gpu_display::benchmark::upload(&target, &mut delta);
}

#[test]
fn a_gpu_texture_with_matching_dimensions_but_wrong_sampling_level_is_rejected() {
    let Some((target, gpu)) = gpu_display::tests::target() else {
        return;
    };
    let mut app = App::new(None);
    app.editor = Editor::new_document(1, 1).unwrap();
    app.render_level = 2;
    app.gpu = Some(gpu.clone());
    app.gpu_target = Some(target);
    let key = app.presentation_key();
    let frame = gpu
        .render_texture(app.editor.document(), || Ok(()))
        .unwrap();
    assert_eq!(frame.size(), App::display_size(key));
    assert_ne!(frame.level(), key.4);
    app.install_gpu_texture(&egui::Context::default(), key, frame);
    assert!(app.gpu_texture.is_none());
    assert!(app.gpu.is_none());
}
