use super::*;

#[test]
fn resizing_shapes_preserves_inflight_frames_and_converges_to_exact_latest_pixels() {
    for tool in [Tool::Line, Tool::Rect, Tool::Ellipse]
        .into_iter()
        .chain(paintcraft_tools::ShapePreset::ALL.map(Tool::Preset))
    {
        let mut app = App::new(None);
        app.editor = Editor::new_document(128, 96).unwrap();
        let ctx = egui::Context::default();
        app.editor.select_tool(tool);
        app.editor.pointer_down((20, 20));
        app.editor.pointer_move((50, 40));
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
        for point in [(80, 60), (30, 25), (10, 10), (70, 50)] {
            app.editor.pointer_move(point);
            app.sync_texture(&ctx);
            assert_eq!(
                app.pending_frame,
                Some((old, handle)),
                "{tool:?}: resizing must not starve rendering"
            );
            assert!(app.accept_render_key(old));
        }
        send.send(()).unwrap();
        let current = app.presentation_key();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while app.texture_key != Some(current) {
            assert!(std::time::Instant::now() < deadline, "{tool:?}");
            app.sync_texture(&ctx);
            std::thread::yield_now();
        }
        let staged = paintcraft_editor::execute::execute(
            app.editor.document(),
            app.editor.selection(),
            &app.editor.preview_batch().unwrap().unwrap(),
        )
        .unwrap();
        assert_eq!(app.rendered, render_flat(&staged.doc).unwrap(), "{tool:?}");
        app.editor.cancel_gesture();
        app.sync_texture(&ctx);
        assert!(
            !app.accept_render_key(old),
            "cancel must retire every frame of the old gesture"
        );
    }
}

#[test]
fn shape_progress_rejects_new_origins_layers_tools_and_styles() {
    let shape = Preview::Shape {
        layer: Uuid::nil(),
        tool: Tool::Rect,
        from: (10, 10),
        to: (40, 50),
        stroke: Some((paintcraft_core::types::Rgba([0, 0, 0, 255]), 2)),
        fill: None,
    };
    let mut changed = shape.clone();
    if let Preview::Shape { to, .. } = &mut changed {
        *to = (5, 5);
    }
    assert!(preserves_preview_progress(Some(&shape), Some(&changed)));
    for field in 0..4 {
        let mut changed = shape.clone();
        if let Preview::Shape {
            layer,
            tool,
            from,
            stroke,
            ..
        } = &mut changed
        {
            match field {
                0 => *layer = Uuid::new_v4(),
                1 => *tool = Tool::Ellipse,
                2 => *from = (9, 10),
                _ => *stroke = None,
            }
        }
        assert!(!preserves_preview_progress(Some(&shape), Some(&changed)));
    }
}
