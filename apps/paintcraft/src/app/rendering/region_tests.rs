use super::*;
use paintcraft_core::types::Rect as DocumentRect;

#[test]
fn regional_cpu_frames_follow_view_bounds_and_reject_stale_progress() {
    let mut app = App::new(None);
    let ctx = egui::Context::default();
    let initial = app.editor.document().clone();
    app.editor.select_tool(Tool::Brush);
    app.editor.pointer_down((40, 40));
    app.render_region = DocumentRect::new(31, 29, 127, 93);
    app.sync_texture(&ctx);
    let old = app.presentation_key();
    let staged = paintcraft_editor::execute::execute(
        app.editor.document(),
        app.editor.selection(),
        &app.editor.preview_batch().unwrap().unwrap(),
    )
    .unwrap();
    assert_eq!(
        app.rendered,
        paintcraft_render::render_region(&staged.doc, old.3).unwrap()
    );
    assert_eq!(app.texture.as_ref().unwrap().size(), [127, 93]);
    app.editor.pointer_move((48, 44));
    app.render_region = DocumentRect::new(500, 300, 63, 71);
    app.sync_texture(&ctx);
    let current = app.presentation_key();
    assert_ne!(old.3, current.3);
    assert!(!app.accept_render_key(old));
    let current_pixels = app.rendered.clone();
    app.install_render(&ctx, old, Ok(vec![0; old.3.area() as usize * 4]));
    assert_eq!(app.rendered, current_pixels);
    assert_eq!(app.texture_key, Some(current));
    app.editor.cancel_gesture();
    app.render_region = None;
    app.sync_texture(&ctx);
    assert_eq!(
        app.rendered,
        paintcraft_render::render_flat(&initial).unwrap()
    );
}

#[test]
fn regional_native_texture_checks_its_own_bounds_before_registration() {
    let Some((target, gpu)) = crate::gpu_display::tests::target() else {
        return;
    };
    let mut app = App::new(None);
    app.gpu = Some(gpu.clone());
    app.gpu_target = Some(target);
    let ctx = egui::Context::default();
    app.render_region = DocumentRect::new(701, 301, 61, 73);
    let key = app.presentation_key();
    let frame = gpu
        .render_region_texture(app.editor.document(), key.3, || Ok(()))
        .unwrap();
    app.install_gpu_texture(&ctx, key, frame);
    assert!(app.gpu_texture.is_some());
    assert_eq!(app.rendered_region(), Some(key.3));
    let wrong = DocumentRect::new(700, 301, 61, 73).unwrap();
    let frame = gpu
        .render_region_texture(app.editor.document(), wrong, || Ok(()))
        .unwrap();
    app.install_gpu_texture(&ctx, key, frame);
    assert!(app.gpu_texture.is_none());
    assert!(app.gpu.is_none());
    app.sync_texture(&ctx);
    assert_eq!(
        app.rendered,
        paintcraft_render::render_region(app.editor.document(), key.3).unwrap()
    );
}

#[test]
fn cpu_display_limit_failure_is_reported_without_a_texture_panic() {
    let mut app = App::new(None);
    app.editor = Editor::new_document(4096, 10).unwrap();
    app.render_region = DocumentRect::new(0, 0, 3000, 5);
    let ctx = egui::Context::default();
    app.sync_texture(&ctx);
    assert!(app.texture.is_none());
    assert!(app.rendered.is_empty());
    assert!(app.rendered_region().is_none());
    app.render_region = DocumentRect::new(2000, 0, 1000, 5);
    app.sync_texture(&ctx);
    assert_eq!(app.texture.as_ref().unwrap().size(), [1000, 5]);
}
