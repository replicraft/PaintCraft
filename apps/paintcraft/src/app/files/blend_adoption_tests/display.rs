use super::*;

#[test]
fn actual_sync_and_background_cpu_display_adopt_modes_with_correct_source_ownership() {
    for asynchronous in [false, true] {
        for mode in [Mode::Multiply, Mode::Screen] {
            let decorated = asynchronous; // Existing synchronous Noise refusal stays closed.
            let doc = source(mode, 2, 1, decorated);
            let mut app = App::new(None);
            app.install(Editor::with_document(doc.clone(), None))
                .unwrap();
            if asynchronous {
                executor(&mut app);
            }
            let before = app.editor.document_snapshot();
            let ticket = app.editor.history_view().ticket;
            display(&mut app, &egui::Context::default());
            assert_eq!(app.rendered, expected(mode, decorated).repeat(2));
            assert!(app.texture.is_some());
            assert!(app.gpu_texture.is_none());
            assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
            assert!(app.editor.document().same_semantics(&doc));
            assert_eq!(app.editor.history_view().ticket, ticket);
            assert_eq!(app.editor.undo_depth(), 0);
            if asynchronous {
                assert_eq!(app.renderer.as_ref().unwrap().reserved_bytes(), 0);
            }
        }
    }
}

#[test]
fn real_native_metal_texture_composes_mask_box_and_noise_before_blend() {
    let Some((target, gpu)) = crate::gpu_display::tests::target() else {
        return;
    };
    #[cfg(target_os = "macos")]
    assert_eq!(target.adapter_info().backend, eframe::wgpu::Backend::Metal);
    for mode in [Mode::Multiply, Mode::Screen] {
        let doc = source(mode, 2, 1, true);
        let mut app = App::new(None);
        app.install(Editor::with_document(doc.clone(), None))
            .unwrap();
        app.gpu_completion = Some(paintcraft_render::gpu::CompletionDriver::new(&gpu).unwrap());
        app.gpu = Some(gpu.clone());
        app.gpu_target = Some(target.clone());
        executor(&mut app);
        let before = app.editor.document_snapshot();
        display(&mut app, &egui::Context::default());
        let texture = app
            .gpu_texture
            .as_ref()
            .expect("CPU fallback is not Metal display proof");
        assert_eq!(
            texture.frame().readback(|| Ok(())).unwrap(),
            expected(mode, true).repeat(2)
        );
        assert!(app.rendered.is_empty());
        assert!(Arc::ptr_eq(app.gpu.as_ref().unwrap(), &gpu));
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert!(app.editor.document().same_semantics(&doc));
        assert_eq!(app.editor.undo_depth(), 0);
        assert_eq!(app.renderer.as_ref().unwrap().reserved_bytes(), 0);
    }
}

#[test]
fn actual_gpu_storage_limit_falls_back_to_cpu_then_native_small_roi_still_works() {
    let limits = eframe::wgpu::Limits {
        max_storage_buffer_binding_size: 16384,
        ..Default::default()
    };
    let Some((target, gpu)) = crate::gpu_display::tests::target_with_limits(0, 32, Some(limits))
    else {
        return;
    };
    let ctx = egui::Context::default();
    // The native egui texture upload includes its actual initialized font atlas.
    let mut initial = ctx.run_ui(Default::default(), |ui| {
        ui.label("blend fallback");
    });
    crate::gpu_display::benchmark::upload(&target, &mut initial.textures_delta);
    let doc = source(Mode::Multiply, 129, 65, true);
    let mut app = App::new(None);
    app.install(Editor::with_document(doc.clone(), None))
        .unwrap();
    app.gpu_completion = Some(paintcraft_render::gpu::CompletionDriver::new(&gpu).unwrap());
    app.gpu = Some(gpu.clone());
    app.gpu_target = Some(target.clone());
    app.render_level = 1;
    app.gpu_scaled_display = true;
    executor(&mut app);
    let before = app.editor.document_snapshot();
    display(&mut app, &ctx);
    assert!(
        app.gpu_texture.is_none(),
        "forced real device limit must exercise fallback"
    );
    assert!(app.texture.is_some());
    assert_eq!(app.rendered, expected(Mode::Multiply, true).repeat(65 * 33));
    assert!(Arc::ptr_eq(app.gpu.as_ref().unwrap(), &gpu));
    assert!(app.gpu_target.is_some());
    let mut delta = ctx.tex_manager().write().take_delta();
    crate::gpu_display::benchmark::upload(&target, &mut delta);
    app.render_region = Rect::new(1, 1, 17, 19);
    app.render_level = 0;
    display(&mut app, &ctx);
    let texture = app
        .gpu_texture
        .as_ref()
        .expect("native ROI backend must remain active");
    assert_eq!(
        texture.frame().readback(|| Ok(())).unwrap(),
        expected(Mode::Multiply, true).repeat(17 * 19)
    );
    assert!(app.rendered.is_empty());
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert!(app.editor.document().same_semantics(&doc));
    assert_eq!(app.editor.undo_depth(), 0);
    assert_eq!(app.renderer.as_ref().unwrap().reserved_bytes(), 0);
}

#[test]
fn actual_sync_budget_refusal_clears_current_display_without_mutating_source_or_counter() {
    use paintcraft_core::types::resources::AllocationClass;
    use paintcraft_render::spatial::{WorkspaceAdmission, with_workspace};
    let doc = source(Mode::Multiply, 2, 1, false);
    let mut app = App::new(None);
    app.install(Editor::with_document(doc.clone(), None))
        .unwrap();
    let ctx = egui::Context::default();
    display(&mut app, &ctx);
    assert_eq!(app.rendered, expected(Mode::Multiply, false).repeat(2));
    let before = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    // Change the ROI to force the real display path. Setup512 cannot fit in511.
    app.render_region = Rect::new(0, 0, 1, 1);
    let budget = SharedBudget::new(crate::spatial_workspace::BYTES);
    let parent = WorkspaceAdmission::new(
        budget.scope(AllocationClass::SharedBudget),
        crate::spatial_workspace::BYTES,
        511,
    );
    with_workspace(parent.clone(), || {
        app.sync_texture(&ctx);
        Ok(())
    })
    .unwrap();
    assert!(app.rendered.is_empty());
    assert!(app.texture.is_none());
    assert!(app.gpu_texture.is_none());
    assert!(app.editor.status().starts_with("Cannot render:"));
    assert_eq!(parent.work_used(), 0);
    assert_eq!(app.texture_key, Some(app.presentation_key()));
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(app.editor.history_view().ticket, ticket);
    assert!(app.editor.document().same_semantics(&doc));
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn cancelled_queued_old_source_cannot_publish_after_native_install() {
    use std::sync::mpsc;
    let ctx = egui::Context::default();
    let old = source(Mode::Multiply, 2, 1, true);
    let new = source(Mode::Screen, 3, 1, true);
    let mut app = App::new(None);
    app.install(Editor::with_document(old.clone(), None))
        .unwrap();
    let old_owner = app.editor.document_snapshot();
    executor(&mut app);
    let (started, waiting) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let blocker = app
        .renderer
        .as_mut()
        .unwrap()
        .submit(
            0,
            move |_| {
                started.send(()).unwrap();
                gate.recv_timeout(Duration::from_secs(20)).unwrap();
                Err(paintcraft_core::types::Error::new(
                    ErrorCode::OperationCancelled,
                    "test blocker",
                ))
            },
            || {},
        )
        .unwrap();
    waiting.recv_timeout(Duration::from_secs(20)).unwrap();
    app.sync_texture(&ctx);
    let old_pending = app.pending_frame.expect("actual queued old render");
    app.install(Editor::with_document(new.clone(), None))
        .unwrap();
    assert!(app.pending_frame.is_none());
    assert!(!app.accept_render_key(old_pending.0));
    app.renderer.as_ref().unwrap().cancel(blocker);
    release.send(()).unwrap();
    display(&mut app, &ctx);
    assert_eq!(app.rendered, expected(Mode::Screen, true).repeat(3));
    assert!(app.editor.document().same_semantics(&new));
    assert!(old_owner.same_semantics(&old));
    assert_eq!(app.editor.undo_depth(), 0);
    assert_eq!(app.renderer.as_ref().unwrap().reserved_bytes(), 0);
}
