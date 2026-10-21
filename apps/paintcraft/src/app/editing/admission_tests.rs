use super::*;
use paintcraft_runtime::SharedBudget;
use std::sync::mpsc;
use std::time::Instant;

#[test]
fn shared_contention_keeps_completed_strokes_queued_and_retries_the_current_frame() {
    let budget = SharedBudget::new(3 << 30);
    let mut blocker = Executor::<u8>::with_budget(
        WorkerLimits {
            workers: 1,
            pending_jobs: 1,
            transient_bytes: 3 << 30,
        },
        budget.clone(),
    )
    .unwrap();
    let (started, wait_started) = mpsc::channel();
    let (release, wait_release) = mpsc::channel();
    let (done, wait_done) = mpsc::channel();
    let handle = blocker
        .submit(
            3 << 30,
            move |_| {
                started.send(()).unwrap();
                wait_release.recv_timeout(Duration::from_secs(10)).unwrap();
                Ok(1)
            },
            move || {
                done.send(()).unwrap();
            },
        )
        .unwrap();
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    let mut app = App::new(None);
    let ctx = egui::Context::default();
    app.worker_budget = budget.clone();
    app.initialize_editing(ctx.clone());
    app.renderer = Some(
        Executor::with_budget(
            WorkerLimits {
                workers: 1,
                pending_jobs: 2,
                transient_bytes: 1 << 30,
            },
            budget.clone(),
        )
        .unwrap(),
    );
    app.editor.pointer_down((3, 3));
    app.editor.pointer_up();
    for _ in 0..3 {
        app.poll_edits(&ctx);
        app.sync_texture(&ctx);
        assert_eq!(app.editor.publications(), 0);
        assert!(app.editor.has_pending_edits());
        assert!(app.pending_frame.is_none());
        assert!(app.texture_key.is_none()); // Contention must not become a cached error.
    }
    release.send(()).unwrap();
    wait_done.recv_timeout(Duration::from_secs(5)).unwrap();
    // Completion alone cannot return admission while another consumer owns it.
    app.poll_edits(&ctx);
    app.sync_texture(&ctx);
    assert_eq!(app.editor.publications(), 0);
    assert_eq!(budget.usage().reserved, 3 << 30);
    assert_eq!(blocker.poll_for(handle).unwrap().unwrap(), 1);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.poll_edits(&ctx);
        app.sync_texture(&ctx);
        if !app.editor.has_pending_edits() && app.texture_key == Some(app.presentation_key()) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(app.editor.publications(), 1);
    assert_eq!(app.rendered, render_flat(app.editor.document()).unwrap());
    let layer = app.editor.target().unwrap();
    assert_eq!(
        app.editor.document().layers[&layer].raster.get(3, 3),
        Rgba([0, 0, 0, 255])
    );
    assert_eq!(budget.usage().reserved, 0);
    assert!(budget.usage().peak <= budget.usage().limit);
}
