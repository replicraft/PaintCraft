//! Real Studio controls, host raster frames and the native FIFO publication path.
use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::filters::BrightnessContrastV1;
use std::time::Instant;
#[path = "exposure_tests.rs"]
mod exposure_tests;
#[path = "levels_tests.rs"]
mod levels_tests;
const RAW: [u8; 8] = [10, 20, 30, 255, 100, 150, 200, 255];
const INVERTED: [u8; 8] = [245, 235, 225, 255, 155, 105, 55, 255];
fn app(asynchronous: bool) -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.editor = Editor::with_document(
        paintcraft_document::import_image(2, 1, &RAW, "native-studio-filters/v1").unwrap(),
        None,
    );
    if asynchronous {
        app.initialize_editing(app.context.clone());
        app.renderer = Some(
            Executor::new(WorkerLimits {
                workers: 1,
                pending_jobs: 2,
                transient_bytes: 1 << 30,
            })
            .unwrap(),
        );
    }
    app
}
fn harness(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1000., 800.))
        .build_ui_state(
            |ui, app: &mut App| {
                app.studio_filter_panel(ui);
                app.confirm_dialog(ui);
            },
            app,
        )
}
fn frame(app: &mut App, ctx: &egui::Context) {
    app.sync_texture(ctx);
    let until = Instant::now() + Duration::from_secs(10);
    while app.texture_key != Some(app.presentation_key()) {
        assert!(Instant::now() < until, "render: {}", app.editor.status());
        app.sync_texture(ctx);
        std::thread::yield_now();
    }
}
fn drain(app: &mut App) {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        app.editor.run_pending();
        app.pump_filter_edit(&app.context.clone());
        if !app.editor.has_pending_edits() {
            break;
        }
        assert!(Instant::now() < until, "edit: {}", app.editor.status());
        std::thread::yield_now();
    }
}
fn open_invert(h: &mut Harness<'_, App>) {
    h.get_by_label("Add filter").click();
    h.run();
    h.get_by_label("Invert").click();
    h.run();
    assert!(h.state().filter_editing.as_ref().unwrap().draft.is_some());
}
#[test]
fn actual_add_preview_cancel_apply_one_undo_and_pcnd3_keep_original_pixels() {
    for asynchronous in [false, true] {
        let mut h = harness(app(asynchronous));
        h.run();
        let source = h.state().editor.document_snapshot();
        let history = h.state().editor.history_view();
        let ctx = h.ctx.clone();
        open_invert(&mut h);
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, INVERTED);
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.history_view(), history);
        assert!(!h.state().editor.published_is_dirty());
        assert!(h.state().editor.export_snapshot().is_err());
        let preview_key = h.state().presentation_key();
        h.get_by_label("Cancel").click();
        h.run();
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, RAW);
        assert!(!h.state().accept_render_key(preview_key));
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        open_invert(&mut h);
        h.get_by_label("Apply").click();
        h.run_steps(1);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, INVERTED);
        assert_eq!(h.state().editor.undo_depth(), 1);
        let doc = h.state().editor.document();
        let layer = &doc.layers[&doc.roots[0]];
        assert_eq!(layer.raster.get(0, 0), Rgba([10, 20, 30, 255]));
        assert_eq!(layer.filters.len(), 1);
        let bytes = paintcraft_formats::pcnd::write_compact(doc).unwrap();
        assert_eq!(bytes[4], 3);
        let reopened = paintcraft_formats::pcnd::read(&bytes).unwrap();
        assert!(doc.same_semantics(&reopened));
        assert_eq!(render_flat(&reopened).unwrap(), INVERTED);
        h.state_mut().editor.request(900, Action::Undo);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, RAW);
        assert_eq!(h.state().editor.undo_depth(), 0);
        assert!(!h.state().editor.published_is_dirty());
    }
}
#[test]
fn preview_off_apply_uses_final_parameters_and_never_exports_private_draft() {
    let mut h = harness(app(true));
    h.run();
    let ctx = h.ctx.clone();
    h.get_by_label("Add filter").click();
    h.run();
    h.get_by_label("Brightness / Contrast").click();
    h.run();
    let source = h.state().editor.document_snapshot();
    // Change retained widget values as a slider does, then exercise actual checkbox and Apply.
    h.state_mut()
        .filter_editing
        .as_mut()
        .unwrap()
        .draft
        .as_mut()
        .unwrap()
        .parameters
        .brightness = 100;
    h.get_by_label("Preview").click();
    h.run();
    frame(h.state_mut(), &ctx);
    assert_eq!(h.state().rendered, RAW);
    assert!(h.state().editor.export_snapshot().is_err());
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
    h.get_by_label("Apply").click();
    h.run_steps(1);
    drain(h.state_mut());
    frame(h.state_mut(), &ctx);
    assert_eq!(h.state().rendered, [255; 8]);
    assert_eq!(h.state().editor.undo_depth(), 1);
    let f = &h.state().editor.document().layers[&source.roots[0]]
        .filters
        .as_slice()[0];
    assert_eq!(
        f.descriptor(),
        Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(100, 0).unwrap())
    );
}
#[test]
fn captured_fifo_base_rejects_changed_state_and_owner_without_filter_publication() {
    for asynchronous in [false, true] {
        let mut app = app(asynchronous);
        let layer = app.editor.target().unwrap();
        let op = operation(&app.editor, layer, Intent::Add(filters::Kind::Invert)).unwrap();
        let edit = app.editor.filter_capture(op);
        app.editor.request(1, Action::AddLayer);
        app.editor.request(2, Action::Filter(edit.clone()));
        drain(&mut app);
        assert_eq!(app.editor.undo_depth(), 1);
        assert!(
            app.editor
                .document()
                .layers
                .values()
                .all(|l| l.filters.is_empty())
        );
        let mut other = Editor::with_document(app.editor.document().clone(), None);
        other.request(3, Action::Filter(edit));
        other.run_pending();
        assert_eq!(other.undo_depth(), 0);
        assert!(
            other
                .document()
                .layers
                .values()
                .all(|l| l.filters.is_empty())
        );
    }
}

#[test]
fn stable_stack_actions_edit_toggle_duplicate_reorder_remove_keep_raw_samples() {
    let mut h = harness(app(false));
    h.run();
    open_invert(&mut h);
    h.get_by_label("Apply").click();
    h.run_steps(1);
    drain(h.state_mut());
    let first = h.state().editor.document().layers[&h.state().editor.target().unwrap()]
        .filters
        .as_slice()[0]
        .id();
    let ctx = h.ctx.clone();
    for intent in [
        Intent::Duplicate(first),
        Intent::SetEnabled {
            id: first,
            enabled: false,
        },
    ] {
        h.state_mut().begin_studio_filter(&ctx, intent);
        h.run();
        drain(h.state_mut());
    }
    let layer = h.state().editor.target().unwrap();
    let second = h.state().editor.document().layers[&layer]
        .filters
        .as_slice()[1]
        .id();
    assert_ne!(first, second);
    h.state_mut().begin_studio_filter(
        &ctx,
        Intent::Reorder {
            id: second,
            to_index: 0,
        },
    );
    h.run();
    drain(h.state_mut());
    assert_eq!(
        h.state().editor.document().layers[&layer]
            .filters
            .as_slice()[0]
            .id(),
        second
    );
    h.state_mut().begin_studio_filter(&ctx, Intent::Edit(first));
    h.run();
    h.get_by_label("Preview").click();
    h.run();
    h.get_by_label("Apply").click();
    h.run_steps(1);
    drain(h.state_mut());
    h.state_mut()
        .begin_studio_filter(&ctx, Intent::Remove(second));
    h.run();
    drain(h.state_mut());
    let stack = &h.state().editor.document().layers[&layer].filters;
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.as_slice()[0].id(), first);
    assert!(!stack.as_slice()[0].enabled());
    assert_eq!(
        h.state().editor.document().layers[&layer].raster.get(1, 0),
        Rgba([100, 150, 200, 255])
    );
    assert_eq!(render_flat(h.state().editor.document()).unwrap(), RAW);
}
#[test]
fn private_draft_fences_other_canvas_work_and_owner_change_retires_preview() {
    let mut h = harness(app(true));
    h.run();
    open_invert(&mut h);
    let ctx = h.ctx.clone();
    let before = h.state().editor.document_snapshot();
    assert!(!h.state_mut().editor.request(701, Action::Undo));
    h.state_mut().editor.pointer_down((0, 0));
    h.state_mut().editor.pointer_up();
    h.state_mut().editor.select_tool(Tool::Pencil);
    assert!(!h.state().editor.gesture_in_progress());
    assert!(h.state().editor.export_snapshot().is_err());
    h.state_mut().set_zoom(&ctx, 4., (1., 0.5));
    frame(h.state_mut(), &ctx);
    assert_eq!(h.state().rendered, INVERTED);
    assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
    // A host replacement with identical document IDs still has a different Session owner.
    h.state_mut().editor = Editor::with_document((*before).clone(), None);
    h.state_mut().pump_filter_edit(&ctx);
    assert!(h.state().filter_editing.is_none());
    assert!(h.state().confirm.is_none());
    assert!(h.state().editor.preview().is_none());
    assert_eq!(h.state().editor.undo_depth(), 0);
}
#[test]
fn opening_waits_real_worker_prefix_and_failed_retained_text_never_captures_old_base() {
    let ctx = egui::Context::default();
    let mut a = app(true);
    let service = a.editing.as_ref().unwrap().clone();
    let release = service.pause();
    a.editor.request(801, Action::AddLayer);
    a.editor.run_pending();
    a.begin_studio_filter(&ctx, Intent::Add(filters::Kind::Invert));
    assert!(a.filter_editing.as_ref().unwrap().draft.is_none());
    let old = a.editor.render_key();
    release.send(()).unwrap();
    drain(&mut a);
    a.pump_filter_edit(&ctx);
    // AddLayer changes the target, so the original requested layer must not be silently replaced.
    assert_ne!(a.editor.render_key(), old);
    assert!(a.filter_editing.is_none());
    let mut a = App::new(None);
    a.settings.experience = Experience::Studio;
    a.editor = Editor::with_document(editing::text_submission_tests::boundary(), None)
        .with_text_renderer(a.text_renderer.clone());
    a.editor.select_tool(Tool::Text);
    a.editor.pointer_down((2, 2));
    a.editor.pointer_up();
    a.editor.type_text("retain me");
    let before = a.editor.document_snapshot();
    a.begin_studio_filter(&ctx, Intent::Add(filters::Kind::Invert));
    a.editor.run_pending();
    a.pump_filter_edit(&ctx);
    assert!(a.editor.failed_text_retained());
    assert!(a.editor.is_editing_text());
    assert!(a.filter_editing.is_none());
    assert!(Arc::ptr_eq(&before, &a.editor.document_snapshot()));
    assert_eq!(a.editor.undo_depth(), 0);
}

#[test]
fn cancel_reopen_equal_parameters_revokes_prior_real_renderer_completion() {
    let mut a = app(false);
    let ctx = egui::Context::default();
    a.begin_studio_filter(&ctx, Intent::Add(filters::Kind::Invert));
    frame(&mut a, &ctx);
    assert_eq!(a.rendered, INVERTED);
    let source = a.editor.document_snapshot();
    let old = a.presentation_key();
    let (send, wait) = std::sync::mpsc::channel();
    let (started, ready) = std::sync::mpsc::sync_channel(1);
    let mut worker = Executor::new(WorkerLimits {
        workers: 1,
        pending_jobs: 2,
        transient_bytes: 1 << 30,
    })
    .unwrap();
    worker
        .submit(
            1024,
            move |_| {
                started.send(()).unwrap();
                wait.recv_timeout(Duration::from_secs(10)).unwrap();
                Ok(RenderedFrame {
                    key: old,
                    pixels: INVERTED.to_vec(),
                    fallback: None,
                    device_epoch: None,
                    gpu_texture: None,
                })
            },
            || {},
        )
        .unwrap();
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    a.renderer = Some(worker);
    a.texture_key = None;
    a.sync_texture(&ctx);
    assert!(a.pending_frame.is_some());
    a.cancel_filter_edit();
    // No intervening render: equality of descriptor/source must not revive the prior lifetime.
    a.begin_studio_filter(&ctx, Intent::Add(filters::Kind::Invert));
    a.sync_texture(&ctx);
    assert!(!a.accept_render_key(old));
    send.send(()).unwrap();
    frame(&mut a, &ctx);
    assert_eq!(a.rendered, INVERTED);
    assert!(Arc::ptr_eq(&source, &a.editor.document_snapshot()));
    assert_eq!(a.editor.undo_depth(), 0);
    a.cancel_filter_edit();
    frame(&mut a, &ctx);
    assert_eq!(a.rendered, RAW);
}

#[test]
fn actual_metadata_limit_apply_refusal_retains_parameters_and_cancel_or_retry_is_explicit() {
    for asynchronous in [false, true] {
        let mut a = App::new(None);
        a.settings.experience = Experience::Studio;
        a.editor = Editor::with_document(rename_boundary(), None);
        if asynchronous {
            a.initialize_editing(a.context.clone());
        }
        let mut h = harness(a);
        h.run();
        let ctx = h.ctx.clone();
        let source = h.state().editor.document_snapshot();
        let original = paintcraft_formats::pcnd::write_compact(&source).unwrap();
        h.state_mut()
            .begin_studio_filter(&ctx, Intent::Add(filters::Kind::BrightnessContrast));
        h.run();
        h.state_mut()
            .filter_editing
            .as_mut()
            .unwrap()
            .draft
            .as_mut()
            .unwrap()
            .parameters
            .brightness = 75;
        h.get_by_label("Apply").click();
        h.run_steps(1);
        drain(h.state_mut());
        h.run();
        let draft = h
            .state()
            .filter_editing
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap();
        assert_eq!(draft.parameters.brightness, 75);
        assert!(draft.attempt.is_none());
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.undo_depth(), 0);
        assert!(!h.state().editor.published_is_dirty());
        assert_eq!(
            paintcraft_formats::pcnd::write_compact(h.state().editor.document()).unwrap(),
            original
        );
        // Repainting never automatically retries a known failed attempt.
        for _ in 0..3 {
            h.run_steps(1);
            h.state_mut().editor.run_pending();
        }
        assert!(!h.state().editor.has_pending_edits());
        assert!(
            h.state()
                .filter_editing
                .as_ref()
                .unwrap()
                .draft
                .as_ref()
                .unwrap()
                .attempt
                .is_none()
        );
        h.get_by_label("Apply").click();
        h.run_steps(1);
        drain(h.state_mut());
        h.run();
        assert_eq!(
            h.state()
                .filter_editing
                .as_ref()
                .unwrap()
                .draft
                .as_ref()
                .unwrap()
                .parameters
                .brightness,
            75
        );
        h.get_by_label("Cancel").click();
        h.run();
        assert!(h.state().filter_editing.is_none());
        assert!(h.state().editor.preview().is_none());
        assert_eq!(
            paintcraft_formats::pcnd::write_compact(h.state().editor.document()).unwrap(),
            original
        );
        // A legitimate later edit frees exactly accounted metadata bytes; no profile is stripped.
        let layer = h.state().editor.target().unwrap();
        h.state_mut().editor.request(
            9901,
            Action::Rename {
                layer,
                name: "A".into(),
            },
        );
        drain(h.state_mut());
        h.run();
        h.state_mut()
            .begin_studio_filter(&ctx, Intent::Add(filters::Kind::BrightnessContrast));
        h.run();
        h.state_mut()
            .filter_editing
            .as_mut()
            .unwrap()
            .draft
            .as_mut()
            .unwrap()
            .parameters
            .brightness = 75;
        h.get_by_label("Apply").click();
        h.run_steps(1);
        drain(h.state_mut());
        h.run();
        assert!(h.state().filter_editing.is_none());
        let doc = h.state().editor.document();
        assert_eq!(doc.layers[&layer].filters.len(), 1);
        assert_eq!(doc.source_metadata, source.source_metadata);
        assert_eq!(doc.layers[&layer].raster, source.layers[&layer].raster);
        let saved = paintcraft_formats::pcnd::write_compact(doc).unwrap();
        assert!(doc.same_semantics(&paintcraft_formats::pcnd::read(&saved).unwrap()));
    }
}
fn rename_boundary() -> paintcraft_document::Document {
    use paintcraft_document::source_metadata::{BYTES_MAX, SourceMetadata};
    let mut doc = editing::text_submission_tests::boundary();
    let metadata = doc.source_metadata.take().unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    let delta = 96 - layer.name.len();
    layer.name = "x".repeat(96);
    let mut profile = metadata.profile().unwrap().to_vec();
    profile.truncate(profile.len() - delta);
    let len = profile.len() as u32;
    profile[..4].copy_from_slice(&len.to_be_bytes());
    doc.source_metadata = Some(
        SourceMetadata::capture(
            *metadata.descriptor(),
            Some(&profile),
            BYTES_MAX,
            &paintcraft_core::types::resources::SharedBudget::new(BYTES_MAX + 4096),
            &|| Ok(()),
        )
        .unwrap(),
    );
    assert_eq!(
        paintcraft_formats::pcnd::write_compact(&doc).unwrap().len(),
        (8 << 20) + 32
    );
    doc
}
#[test]
fn stale_filter_request_rejects_before_it_can_place_unrelated_private_text() {
    let mut a = app(false);
    let edit = a.editor.filter_capture(
        operation(
            &a.editor,
            a.editor.target().unwrap(),
            Intent::Add(filters::Kind::Invert),
        )
        .unwrap(),
    );
    a.editor.request(4001, Action::AddLayer);
    a.editor.run_pending();
    a.editor.select_tool(Tool::Text);
    a.editor.pointer_down((0, 0));
    a.editor.pointer_up();
    a.editor.type_text("private");
    let source = a.editor.document_snapshot();
    let history = a.editor.history_view();
    assert!(!a.editor.request(4002, Action::Filter(edit)));
    assert!(a.editor.is_editing_text());
    assert!(Arc::ptr_eq(&source, &a.editor.document_snapshot()));
    assert_eq!(a.editor.history_view(), history);
    assert!(!a.editor.has_pending_edits());
}

#[test]
fn cancelling_an_already_prepared_real_worker_apply_prevents_publication_until_retired() {
    let mut h = harness(app(true));
    h.run();
    open_invert(&mut h);
    let source = h.state().editor.document_snapshot();
    let service = h.state().editing.as_ref().unwrap().clone();
    let (prepared, release) = service.pause_next_completion();
    h.get_by_label("Apply").click();
    h.run_steps(1);
    h.state_mut().editor.run_pending();
    prepared.recv_timeout(Duration::from_secs(5)).unwrap();
    let attempt = h
        .state()
        .filter_editing
        .as_ref()
        .unwrap()
        .draft
        .as_ref()
        .unwrap()
        .attempt
        .unwrap();
    assert_eq!(
        h.state().editor.filter_feedback(attempt),
        Some(controller::FilterFeedback::Active)
    );
    for _ in 0..4 {
        h.run_steps(1);
        if h.query_by_label("Cancel").is_some() {
            break;
        }
        assert_eq!(h.state().confirm, Some(Confirm::FilterParameters));
    }
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
    h.get_by_label("Cancel").click();
    h.run_steps(1);
    assert!(h.state().filter_editing.is_none());
    assert!(
        h.state().editor.has_pending_edits(),
        "cancelled worker must actually retire"
    );
    assert!(h.state().editor.export_snapshot().is_err());
    release.send(()).unwrap();
    drain(h.state_mut());
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
    assert_eq!(h.state().editor.undo_depth(), 0);
    assert!(!h.state().editor.published_is_dirty());
    assert!(h.state().editor.export_snapshot().is_ok());
}

#[path = "mask_tests.rs"]
mod mask_tests;

#[path = "point_filter_tests.rs"]
mod point_filter_tests;

#[path = "box_tests.rs"]
mod box_tests;
