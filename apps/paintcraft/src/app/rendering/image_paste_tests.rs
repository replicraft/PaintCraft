use super::*;
use paintcraft_core::types::{clipboard::Image, resources::SharedBudget};
use paintcraft_editor::clipboard::PasteCanvas;
const KEY: Rgba = Rgba([90, 80, 70, 255]);
fn setup() -> App {
    let mut app = App::new(None);
    app.editor = Editor::new_document(8, 6).unwrap();
    let image = Image::from_fn(3, 2, &SharedBudget::new(1024), &|| Ok(()), |x, y| {
        Ok(if x == 2 {
            KEY
        } else {
            Rgba([30 + x as u8, 40 + y as u8, 200, 128])
        })
    })
    .unwrap();
    let mut draft = app.editor.paste_draft(image).unwrap();
    draft.place((1, 1), PasteCanvas::Keep).unwrap();
    app.editor.begin_image_paste(draft, Some(KEY)).unwrap();
    app
}
fn oracle(origin: (usize, usize)) -> Vec<u8> {
    let mut out = vec![255; 8 * 6 * 4];
    for y in 0..2 {
        for x in 0..2 {
            let source = [30 + x as u8, 40 + y as u8, 200];
            let offset = ((origin.1 + y) * 8 + origin.0 + x) * 4;
            for c in 0..3 {
                out[offset + c] = ((u32::from(source[c]) * 128 + 255 * 127 + 127) / 255) as u8;
            }
        }
    }
    out
}
fn finish(app: &mut App, ctx: &egui::Context) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.sync_texture(ctx);
        if app.texture_key == Some(app.presentation_key()) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{}",
            app.editor.status()
        );
        std::thread::yield_now();
    }
}
#[test]
fn image_paste_display_matches_dense_alpha_key_oracle_and_cancel_retires_old_frames() {
    for asynchronous in [false, true] {
        let mut app = setup();
        let ctx = egui::Context::default();
        if asynchronous {
            app.renderer = Some(
                Executor::new(WorkerLimits {
                    workers: 1,
                    pending_jobs: 2,
                    transient_bytes: 512 << 20,
                })
                .unwrap(),
            );
        }
        finish(&mut app, &ctx);
        assert_eq!(app.rendered, oracle((1, 1)));
        app.editor.nudge_image_paste((2, 1)).unwrap();
        finish(&mut app, &ctx);
        assert_eq!(app.rendered, oracle((3, 2)));
        let old = app.presentation_key();
        app.editor.cancel_image_paste();
        finish(&mut app, &ctx);
        assert!(!app.accept_render_key(old));
        assert_eq!(app.rendered, vec![255; 8 * 6 * 4]);
        assert_eq!(app.editor.undo_depth(), 0);
    }
}
#[test]
fn moving_image_paste_keeps_admitted_render_progress_then_converges() {
    let mut app = setup();
    let ctx = egui::Context::default();
    finish(&mut app, &ctx);
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
                wait.recv_timeout(Duration::from_secs(5)).unwrap();
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
    for _ in 0..3 {
        app.editor.nudge_image_paste((1, 0)).unwrap();
        app.sync_texture(&ctx);
        assert_eq!(app.pending_frame, Some((old, handle)));
        assert!(app.accept_render_key(old));
    }
    send.send(()).unwrap();
    finish(&mut app, &ctx);
    assert_eq!(app.rendered, oracle((4, 1)));
    assert_eq!(app.editor.undo_depth(), 0);
}

#[test]
#[ignore = "Explicit offscreen GPU image-paste preview qualification"]
fn gpu_image_paste_preview_matches_dense_alpha_key_oracle() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("GPU required for this explicit qualification");
    eprintln!("Image-paste GPU adapter: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut app = setup();
    let ctx = egui::Context::default();
    app.replace_gpu(Some(Arc::new(GpuRenderer::new(device, queue).unwrap())));
    app.renderer = Some(
        Executor::new(WorkerLimits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 512 << 20,
        })
        .unwrap(),
    );
    finish(&mut app, &ctx);
    assert_eq!(app.rendered, oracle((1, 1)));
    app.editor.nudge_image_paste((2, 1)).unwrap();
    finish(&mut app, &ctx);
    assert_eq!(app.rendered, oracle((3, 2)));
    assert_eq!(app.editor.undo_depth(), 0);
}
