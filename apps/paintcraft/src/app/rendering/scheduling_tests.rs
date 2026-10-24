use super::*;
#[test]
fn moving_brush_preserves_inflight_progress_and_then_renders_the_latest_samples() {
    let mut app = App::new(None);
    let ctx = egui::Context::default();
    app.editor.select_tool(Tool::Brush);
    app.editor.pointer_down((10, 10));
    app.sync_texture(&ctx);
    let old = app.presentation_key();
    let pixels = app.rendered.clone();
    let (send, wait) = std::sync::mpsc::channel();
    let mut worker = Executor::new(WorkerLimits {
        workers: 1,
        pending_jobs: 2,
        transient_bytes: 512 << 20,
    })
    .unwrap();
    let handle = worker
        .submit(
            1024,
            move |_| {
                wait.recv().unwrap();
                Ok(RenderedFrame {
                    key: old,
                    pixels,
                    fallback: None,
                    device_epoch: None,
                    gpu_texture: None,
                })
            },
            || {},
        )
        .unwrap();
    app.renderer = Some(worker);
    app.pending_frame = Some((old, handle));
    app.texture_key = None;
    for i in 1..40 {
        app.editor.pointer_move((10 + i, 10));
        app.sync_texture(&ctx);
        assert_eq!(app.pending_frame, Some((old, handle)));
    }
    send.send(()).unwrap();
    let current = app.presentation_key();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app.texture_key != Some(current) {
        assert!(std::time::Instant::now() < deadline);
        app.sync_texture(&ctx);
        std::thread::yield_now();
    }
    let staged = paintcraft_editor::execute::execute(
        app.editor.document(),
        app.editor.selection(),
        &app.editor.preview_batch().unwrap().unwrap(),
    )
    .unwrap();
    assert_eq!(app.rendered, render_flat(&staged.doc).unwrap());
}
#[test]
fn idle_repaints_reuse_the_owned_preview_even_after_an_equal_value_refresh() {
    let mut app = App::new(None);
    let ctx = egui::Context::default();
    app.editor.pointer_down((1, 1));
    app.editor.pointer_move((2, 1));
    app.sync_texture(&ctx);
    let first = app.rendered_preview.clone().unwrap();
    let revision = app.preview_revision;
    app.sync_texture(&ctx);
    assert!(Arc::ptr_eq(&first, app.rendered_preview.as_ref().unwrap()));
    assert_eq!(app.preview_revision, revision);
    // This changes the source stamp, but the gesture captured its own tool.
    app.editor.tool = Tool::Brush;
    app.sync_texture(&ctx);
    let refreshed = app.rendered_preview.clone().unwrap();
    assert_eq!(first, refreshed);
    assert_eq!(app.preview_revision, revision);
    app.sync_texture(&ctx);
    assert!(Arc::ptr_eq(
        &refreshed,
        app.rendered_preview.as_ref().unwrap()
    ));
    app.editor.pointer_move((3, 1));
    app.sync_texture(&ctx);
    assert!(app.preview_revision > revision);
    assert_ne!(first, app.rendered_preview.clone().unwrap());
}
#[test]
fn native_preview_capture_and_caret_lookup_do_not_wait_for_a_busy_font_rasterizer() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    let mut app = App::new(None);
    let ctx = egui::Context::default();
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((8, 8));
    app.editor.type_text("Hello");
    app.renderer = Some(
        Executor::new(WorkerLimits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 1 << 30,
        })
        .unwrap(),
    );
    let fonts = app.text_renderer.clone();
    let (started, wait_started) = mpsc::sync_channel(1);
    let (release, wait_release) = mpsc::channel();
    let forced = Arc::new(AtomicBool::new(false));
    let timed_out = forced.clone();
    let holder = std::thread::spawn(move || {
        let _guard = fonts.lock().unwrap();
        started.send(()).unwrap();
        if wait_release.recv_timeout(Duration::from_secs(8)).is_err() {
            timed_out.store(true, Ordering::SeqCst);
        }
    });
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    // The old UI-side batch builder waited for this same mutex before it
    // could even submit a render job. This call must return with it held.
    app.sync_texture(&ctx);
    assert!(app.pending_frame.is_some());
    assert!(
        !forced.load(Ordering::SeqCst),
        "UI waited for font rasterization"
    );
    assert!(
        app.editor
            .cached_font_text_mask("Hello", &app.settings.text, (8, 8))
            .is_none()
    );
    app.editor.type_text(" Ω");
    app.sync_texture(&ctx);
    release.send(()).unwrap();
    holder.join().unwrap();
    let current = app.presentation_key();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app.texture_key != Some(current) {
        assert!(std::time::Instant::now() < deadline);
        app.sync_texture(&ctx);
        std::thread::yield_now();
    }
    let staged = paintcraft_editor::execute::execute(
        app.editor.document(),
        app.editor.selection(),
        &app.editor.preview_batch().unwrap().unwrap(),
    )
    .unwrap();
    assert_eq!(app.rendered, render_flat(&staged.doc).unwrap());
    assert_eq!(app.editor.publications(), 0);
}
fn delayed_frame(
    app: &mut App,
    pixels: Vec<u8>,
) -> (RenderKey, JobHandle, std::sync::mpsc::Sender<()>) {
    let old = app.presentation_key();
    let (release, wait) = std::sync::mpsc::channel();
    let (started, wait_started) = std::sync::mpsc::sync_channel(1);
    let mut renderer = Executor::new(WorkerLimits {
        workers: 1,
        pending_jobs: 2,
        transient_bytes: 1 << 30,
    })
    .unwrap();
    let handle = renderer
        .submit(
            1024,
            move |_| {
                started.send(()).unwrap();
                wait.recv_timeout(Duration::from_secs(10)).unwrap();
                Ok(RenderedFrame {
                    key: old,
                    pixels,
                    fallback: None,
                    device_epoch: None,
                    gpu_texture: None,
                })
            },
            || {},
        )
        .unwrap();
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    app.renderer = Some(renderer);
    app.pending_frame = Some((old, handle));
    app.texture_key = None;
    (old, handle, release)
}
fn finish_current(app: &mut App, ctx: &egui::Context) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let current = app.presentation_key();
    while app.texture_key != Some(current) {
        assert!(std::time::Instant::now() < deadline);
        app.sync_texture(ctx);
        std::thread::yield_now();
    }
}
#[test]
fn deleting_text_retires_inflight_longer_text_instead_of_treating_it_as_progress() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((8, 8));
    app.editor.type_text("Hello Ω");
    app.sync_texture(&ctx);
    let pixels = app.rendered.clone();
    let (old, handle, release) = delayed_frame(&mut app, pixels);
    app.editor.backspace();
    app.sync_texture(&ctx);
    assert!(!app.accept_render_key(old));
    assert_ne!(app.pending_frame, Some((old, handle)));
    release.send(()).unwrap();
    finish_current(&mut app, &ctx);
    let staged = paintcraft_editor::execute::execute(
        app.editor.document(),
        app.editor.selection(),
        &app.editor.preview_batch().unwrap().unwrap(),
    )
    .unwrap();
    assert_eq!(app.rendered, render_flat(&staged.doc).unwrap());
}
#[test]
fn cancelling_and_retrying_identical_queued_paint_retires_the_old_owner_even_with_equal_pixels() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.initialize_editing(ctx.clone());
    let gate = app.editing.as_ref().unwrap().pause();
    let stroke = paintcraft_tools::Stroke {
        media: Default::default(),
        layer: app.editor.target().unwrap(),
        points: vec![(3, 3)],
        point_scale: 1,
        color: Rgba([255, 0, 0, 128]),
        size: 1,
        round: false,
        mode: paintcraft_tools::BlendMode::SourceOver,
    };
    app.editor.request(101, Action::Stroke(stroke.clone()));
    app.editor.run_pending();
    app.sync_texture(&ctx);
    let pixels = app.rendered.clone();
    let (old, handle, release) = delayed_frame(&mut app, pixels);
    app.editor.cancel_pending_edits();
    app.editor.request(102, Action::Stroke(stroke));
    app.editor.run_pending();
    app.sync_texture(&ctx);
    assert!(!app.accept_render_key(old));
    assert_ne!(app.pending_frame, Some((old, handle)));
    release.send(()).unwrap();
    finish_current(&mut app, &ctx);
    assert_eq!(
        &app.rendered[(3 * 1000 + 3) * 4..][..4],
        &[255, 127, 127, 255]
    );
    app.editor.cancel_pending_edits();
    gate.send(()).unwrap();
}
