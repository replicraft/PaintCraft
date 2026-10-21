use super::*;
use paintcraft_render::gpu::DeviceEpoch;
use std::sync::mpsc;
use std::time::Instant;

#[test]
fn a_late_gpu_success_or_fallback_cannot_install_after_backend_replacement() {
    for fallback in [None, Some("stale device fallback".to_owned())] {
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        app.sync_texture(&ctx);
        let expected = app.rendered.clone();
        let key = app.presentation_key();
        let old = DeviceEpoch::default();
        let (completed, wait_completed) = mpsc::channel();
        let mut executor = Executor::new(WorkerLimits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 1 << 30,
        })
        .unwrap();
        let handle = executor
            .submit(
                1024,
                move |_| {
                    Ok(RenderedFrame {
                        key,
                        pixels: vec![0; expected.len()],
                        fallback,
                        device_epoch: Some(old),
                        gpu_texture: None,
                    })
                },
                move || {
                    completed.send(()).unwrap();
                },
            )
            .unwrap();
        wait_completed.recv_timeout(Duration::from_secs(5)).unwrap();
        app.renderer = Some(executor);
        app.pending_frame = Some((key, handle));
        app.texture_key = None;
        app.editor.set_status("current backend");
        // Deliberately keep the old handle registered: the epoch guard must
        // reject an otherwise current document/preview ticket independently.
        app.sync_texture(&ctx);
        assert_eq!(app.editor.status(), "current backend");
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.texture_key != Some(key) {
            assert!(Instant::now() < deadline);
            app.sync_texture(&ctx);
            std::thread::yield_now();
        }
        assert!(app.rendered.iter().all(|value| *value == 255));
    }
}
#[test]
fn replacing_a_backend_cancels_admitted_work_and_preserves_host_document() {
    let mut app = App::new(None);
    let ctx = egui::Context::default();
    app.editor.pointer_down((1, 1));
    app.editor.pointer_up();
    app.editor.run_pending();
    let ticket = app.editor.render_key();
    let expected = render_flat(app.editor.document()).unwrap();
    let (started, wait_started) = mpsc::channel();
    let (release, wait_release) = mpsc::channel();
    let mut executor = Executor::new(WorkerLimits {
        workers: 1,
        pending_jobs: 2,
        transient_bytes: 1 << 30,
    })
    .unwrap();
    let key = app.presentation_key();
    let length = expected.len();
    let handle = executor
        .submit(
            1024,
            move |_| {
                started.send(()).unwrap();
                wait_release.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok(RenderedFrame {
                    key,
                    pixels: vec![0; length],
                    fallback: None,
                    device_epoch: None,
                    gpu_texture: None,
                })
            },
            || {},
        )
        .unwrap();
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    app.renderer = Some(executor);
    app.pending_frame = Some((key, handle));
    app.replace_gpu(None);
    assert!(app.pending_frame.is_none());
    assert_eq!(app.renderer.as_ref().unwrap().pending(), 1);
    assert_eq!(app.renderer.as_ref().unwrap().reserved_bytes(), 1024);
    assert_eq!(app.editor.render_key(), ticket);
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.texture_key != Some(key) {
        assert!(Instant::now() < deadline);
        app.sync_texture(&ctx);
        std::thread::yield_now();
    }
    assert_eq!(app.rendered, expected);
    assert_eq!(app.renderer.as_ref().unwrap().reserved_bytes(), 0);
    assert_eq!(app.editor.render_key(), ticket);
}
