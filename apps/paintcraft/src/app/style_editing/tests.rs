//! Actual native modal/FIFO/preview; independent full-opacity RGB literals.
use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::styles::EffectBlendMode;
use std::time::Instant;
const RAW: [u8; 8] = [10, 20, 30, 255, 100, 150, 200, 255];
const RED: [u8; 8] = [255, 0, 0, 255, 255, 0, 0, 255];
fn app() -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.editor = Editor::with_document(
        paintcraft_document::import_image(2, 1, &RAW, "native-color-overlay/v1").unwrap(),
        None,
    );
    app.editor.color = Rgba([255, 0, 0, 255]);
    app.initialize_editing(app.context.clone());
    app.renderer = Some(
        Executor::new(WorkerLimits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 1 << 30,
        })
        .unwrap(),
    );
    app
}
fn harness(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1000., 800.))
        .build_ui_state(
            |ui, app: &mut App| {
                app.studio_style_panel(ui);
                app.confirm_dialog(ui);
            },
            app,
        )
}
fn drain(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.editor.run_pending();
        app.pump_style_edit(&app.context.clone());
        if !app.editor.has_pending_edits() {
            break;
        }
        assert!(Instant::now() < deadline, "{}", app.editor.status());
        std::thread::yield_now();
    }
}
fn frame(app: &mut App, ctx: &egui::Context) {
    let deadline = Instant::now() + Duration::from_secs(10);
    app.sync_texture(ctx);
    while app.texture_key != Some(app.presentation_key()) {
        assert!(Instant::now() < deadline, "{}", app.editor.status());
        app.sync_texture(ctx);
        std::thread::yield_now();
    }
}
fn open(h: &mut Harness<'_, App>) {
    h.get_by_label("Add Color Overlay").click();
    h.run();
    assert!(h.state().style_editing.as_ref().unwrap().draft.is_some());
}
fn apply(h: &mut Harness<'_, App>) {
    h.get_by_label("Apply").click();
    h.run_steps(1);
    drain(h.state_mut());
}
#[test]
fn actual_modal_private_preview_cancel_apply_pcnd9_and_undo_redo() {
    let mut h = harness(app());
    h.run();
    let source = h.state().editor.document_snapshot();
    let history = h.state().editor.history_view();
    let ctx = h.ctx.clone();
    open(&mut h);
    frame(h.state_mut(), &ctx);
    assert_eq!(h.state().rendered, RED);
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
    assert_eq!(h.state().editor.history_view(), history);
    assert!(!h.state().editor.published_is_dirty());
    assert!(h.state().editor.export_snapshot().is_err());
    let preview = h.state().presentation_key();
    h.get_by_label("Cancel").click();
    h.run();
    frame(h.state_mut(), &ctx);
    assert_eq!(h.state().rendered, RAW);
    assert!(!h.state().accept_render_key(preview));
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
    open(&mut h);
    apply(&mut h);
    frame(h.state_mut(), &ctx);
    assert_eq!(h.state().rendered, RED);
    assert_eq!(h.state().editor.undo_depth(), 1);
    let doc = h.state().editor.document();
    let layer = &doc.layers[&doc.roots[0]];
    assert_eq!(layer.raster.get(0, 0), Rgba([10, 20, 30, 255]));
    assert_eq!(layer.styles.len(), 1);
    let bytes = paintcraft_formats::pcnd::write_compact(doc).unwrap();
    assert_eq!(bytes[4], 9);
    let budget = paintcraft_core::types::resources::SharedBudget::new(1 << 20);
    let reopened = paintcraft_formats::pcnd::read_with_admission_checked(
        &bytes,
        2,
        65536,
        1,
        paintcraft_formats::pcnd::ReadAdmission {
            metadata: paintcraft_formats::pcnd::MetadataAdmission {
                bytes: 8 << 20,
                budget: &budget,
            },
            coverage: paintcraft_formats::pcnd::CoverageAdmission {
                bytes: 1 << 20,
                budget: &budget,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap();
    assert!(doc.same_semantics(&reopened));
    assert!(spatial_workspace::needed(&reopened));
    let pixels = spatial_workspace::with_job(spatial_workspace::needed(&reopened), || {
        render_flat(&reopened)
    })
    .unwrap();
    assert_eq!(pixels, RED);
    h.state_mut().editor.request(900, Action::Undo);
    drain(h.state_mut());
    frame(h.state_mut(), &ctx);
    assert_eq!(h.state().rendered, RAW);
    assert!(!h.state().editor.published_is_dirty());
    h.state_mut().editor.request(901, Action::Redo);
    drain(h.state_mut());
    frame(h.state_mut(), &ctx);
    assert_eq!(h.state().rendered, RED);
}
#[test]
fn actual_named_rgb_opacity_effect_and_preview_widgets_store_complete_zero_intent() {
    let mut h = harness(app());
    h.run();
    open(&mut h);
    h.get_by_role_and_label(egui::Role::SpinButton, "Red")
        .set_value_accesskit(7.);
    h.run();
    h.get_by_role_and_label(egui::Role::SpinButton, "Green")
        .set_value_accesskit(9.);
    h.run();
    h.get_by_role_and_label(egui::Role::SpinButton, "Blue")
        .set_value_accesskit(11.);
    h.run();
    h.get_by_role_and_label(egui::Role::Slider, "Style opacity (0–255)")
        .set_value_accesskit(0.);
    h.run();
    h.get_by_role_and_label(egui::Role::ComboBox, "Effect blend mode")
        .click();
    h.run();
    h.get_by_label("Screen").click();
    h.run();
    h.get_by_label("Preview").click();
    h.run();
    let draft = h
        .state()
        .style_editing
        .as_ref()
        .unwrap()
        .draft
        .as_ref()
        .unwrap();
    assert_eq!(draft.parameters.rgb, [7, 9, 11]);
    assert_eq!(draft.parameters.opacity, 0);
    assert_eq!(draft.parameters.blend, EffectBlendMode::Screen);
    assert!(!draft.parameters.preview);
    let ctx = h.ctx.clone();
    frame(h.state_mut(), &ctx);
    assert_eq!(h.state().rendered, RAW);
    apply(&mut h);
    frame(h.state_mut(), &ctx);
    assert_eq!(h.state().rendered, RAW);
    let doc = h.state().editor.document();
    assert_eq!(
        doc.layers[&doc.roots[0]].styles.as_slice()[0]
            .descriptor()
            .semantic_parameter_bytes(),
        [7, 9, 11, 0, 2]
    );
    assert_eq!(h.state().editor.undo_depth(), 1);
    assert!(h.state().editor.published_is_dirty());
}
#[test]
fn no_worker_is_explicit_refusal_and_no_synchronous_edit_fallback() {
    let mut app = app();
    app.editor = Editor::with_document((*app.editor.document_snapshot()).clone(), None);
    let before = app.editor.document_snapshot();
    app.begin_studio_style(&app.context.clone(), Intent::Add);
    assert!(app.style_editing.is_none());
    assert!(!app.editor.style_worker_available());
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(app.editor.undo_depth(), 0);
}
#[test]
fn duplicate_attempt_and_cancel_before_fifo_activation_never_publish() {
    let mut app = app();
    let layer = app.editor.target().unwrap();
    let edit = app
        .editor
        .style_capture(operation(&app.editor, layer, Intent::Add).unwrap());
    let id = app.editor.queue_style_apply(810, edit.clone()).unwrap();
    assert_eq!(app.editor.queue_style_apply(810, edit.clone()).unwrap(), id);
    assert!(app.editor.queue_style_apply(811, edit).is_err());
    let source = app.editor.document_snapshot();
    app.editor.cancel_style_attempt(id);
    drain(&mut app);
    assert_eq!(
        app.editor.style_feedback(id),
        Some(controller::StyleFeedback::Cancelled)
    );
    assert!(Arc::ptr_eq(&source, &app.editor.document_snapshot()));
    assert_eq!(app.editor.undo_depth(), 0);
}
#[test]
fn edit_noop_disable_duplicate_move_remove_use_same_captured_fifo() {
    let mut h = harness(app());
    h.run();
    open(&mut h);
    apply(&mut h);
    let layer = h.state().editor.target().unwrap();
    let id = h.state().editor.document().layers[&layer].styles.as_slice()[0].id();
    let depth = h.state().editor.undo_depth();
    let ctx = h.ctx.clone();
    h.state_mut().begin_studio_style(&ctx, Intent::Edit(id));
    h.run();
    apply(&mut h);
    assert_eq!(
        h.state().editor.undo_depth(),
        depth,
        "same descriptor is NoOp"
    );
    for intent in [
        Intent::SetEnabled { id, enabled: false },
        Intent::Duplicate(id),
    ] {
        let ctx = h.ctx.clone();
        h.state_mut().begin_studio_style(&ctx, intent);
        // A separate real GUI frame gives each activation its own token; layout
        // passes within one frame must continue to deduplicate the same token.
        h.run();
        drain(h.state_mut());
    }
    let layer_ref = &h.state().editor.document().layers[&layer];
    assert_eq!(layer_ref.styles.len(), 2);
    assert!(!layer_ref.styles.as_slice()[0].enabled());
    assert_eq!(h.state().editor.undo_depth(), depth + 2);
    let duplicate = layer_ref.styles.as_slice()[1].id();
    for intent in [
        Intent::Move {
            id: duplicate,
            to_index: 0,
        },
        Intent::Remove(id),
    ] {
        let ctx = h.ctx.clone();
        h.state_mut().begin_studio_style(&ctx, intent);
        // A separate real GUI frame gives each activation its own token; layout
        // passes within one frame must continue to deduplicate the same token.
        h.run();
        drain(h.state_mut());
    }
    assert_eq!(
        h.state().editor.document().layers[&layer].styles.as_slice()[0].id(),
        duplicate
    );
    assert_eq!(h.state().editor.document().layers[&layer].styles.len(), 1);
}

#[test]
fn stale_base_and_stale_target_are_refused_before_queuing_or_preview() {
    let mut app = app();
    let layer = app.editor.target().unwrap();
    let edit = app
        .editor
        .style_capture(operation(&app.editor, layer, Intent::Add).unwrap());
    app.editor.request(920, Action::AddLayer);
    drain(&mut app);
    let current = app.editor.document_snapshot();
    let history = app.editor.history_view();
    assert!(app.editor.queue_style_apply(921, edit).is_err());
    assert!(Arc::ptr_eq(&current, &app.editor.document_snapshot()));
    assert_eq!(app.editor.history_view(), history);
}
#[test]
fn real_modal_controls_and_statuses_resolve_in_all_seven_languages() {
    for language in paintcraft_ui::i18n::Language::ALL {
        let mut h = harness(app());
        paintcraft_ui::i18n::set_language(&h.ctx, language);
        h.run();
        let ctx = h.ctx.clone();
        h.state_mut().begin_studio_style(&ctx, Intent::Add);
        h.run();
        for label in ["Red", "Green", "Blue"] {
            h.get_by_role_and_label(egui::Role::SpinButton, &paintcraft_ui::t!(&h.ctx, label));
        }
        h.get_by_role_and_label(
            egui::Role::Slider,
            &paintcraft_ui::t!(&h.ctx, "Style opacity (0–255)"),
        );
        h.get_by_role_and_label(
            egui::Role::ComboBox,
            &paintcraft_ui::t!(&h.ctx, "Effect blend mode"),
        );
        let detail = "resource.limit: 原稿/{literal}.pcnd";
        for prefix in ["Style not opened: ", "Style not applied: "] {
            let raw = format!("{prefix}{detail}");
            let translated = paintcraft_ui::i18n::tr_status(&h.ctx, &raw);
            assert!(translated.ends_with(detail));
            if language != paintcraft_ui::i18n::Language::English {
                assert_ne!(raw, translated);
            }
        }
        let cancel = paintcraft_ui::t!(&h.ctx, "Cancel");
        h.get_by_label(&cancel).click();
        h.run();
        assert!(h.state().style_editing.is_none());
        assert_eq!(h.state().editor.undo_depth(), 0);
    }
}

#[test]
fn admitted_style_preview_without_render_executor_uses_existing_base_context() {
    let mut a = app();
    a.renderer = None;
    let source = a.editor.document_snapshot();
    let history = a.editor.history_view();
    let mut h = harness(a);
    h.run();
    open(&mut h);
    let ctx = h.ctx.clone();
    frame(h.state_mut(), &ctx);
    assert_eq!(h.state().rendered, RED);
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
    assert_eq!(h.state().editor.history_view(), history);
    h.get_by_label("Cancel").click();
    h.run();
    frame(h.state_mut(), &ctx);
    assert_eq!(h.state().rendered, RAW);
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
}

#[path = "gpu_tests.rs"]
mod gpu_tests;
