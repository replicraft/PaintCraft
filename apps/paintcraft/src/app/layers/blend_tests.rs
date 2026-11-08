use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::{LayerBlendMode as Mode, Rgba};
use paintcraft_ui::i18n::{Language, translate};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
fn app(asynchronous: bool) -> App {
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([31, 127, 241, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let mut top = doc.layers[&doc.roots[0]].clone();
    top.id = Uuid::from_u128(0x62c0);
    top.name = "Top".into();
    top.raster = paintcraft_raster::RasterLayer::new(2, 1, Rgba([204, 77, 13, 255]));
    doc.roots.push(top.id);
    doc.layers.insert(top.id, top);
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.install(Editor::with_document(doc, None)).unwrap();
    if asynchronous {
        app.initialize_editing(app.context.clone());
    }
    app
}
fn harness(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(900.0, 850.0))
        .build_ui_state(
            |ui, app: &mut App| {
                if ui.ctx().missing_glyph_policy() != egui::epaint::text::MissingGlyphPolicy::Tofu {
                    paintcraft_ui::fonts::install(ui.ctx());
                    ui.ctx()
                        .set_missing_glyph_policy(egui::epaint::text::MissingGlyphPolicy::Tofu);
                }
                paintcraft_ui::i18n::set_language(ui.ctx(), app.settings.language);
                // Isolate the actual modal controls while covered, avoiding duplicate
                // background opacity/name accessibility nodes; no production test hook.
                if app.confirm.is_none() {
                    app.layer_panel(ui);
                }
                app.confirm_dialog(ui);
            },
            app,
        )
}
fn open(h: &mut Harness<'_, App>, language: Language) {
    // A closing modal changes state after this harness's background pass.
    // Present the uncovered panel before querying its actual button.
    h.run();
    h.get_by_label(&translate(language, "Layer properties…"))
        .click();
    h.run();
    assert!(
        h.state()
            .confirm
            .as_ref()
            .is_some_and(|c| matches!(c, Confirm::LayerProperties(_)))
    );
}
fn mode(h: &mut Harness<'_, App>, language: Language, name: &str) {
    h.get_by_role_and_label(
        egui::Role::ComboBox,
        &translate(language, "Layer blend mode"),
    )
    .click();
    h.run();
    h.get_by_label(&translate(language, name)).click();
    h.run();
}
fn name(h: &mut Harness<'_, App>, value: &str) {
    h.get_by_role_and_label(egui::Role::TextInput, "Name")
        .focus();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run();
    h.get_by_role_and_label(egui::Role::TextInput, "Name")
        .type_text(value);
    h.run();
}
fn drain(app: &mut App) {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        app.editor.run_pending();
        if !app.editor.has_pending_edits() {
            break;
        }
        assert!(Instant::now() < until, "{}", app.editor.status());
        std::thread::yield_now();
    }
}
fn frame(app: &mut App, ctx: &egui::Context) {
    app.sync_texture(ctx);
    assert_eq!(app.texture_key, Some(app.presentation_key()));
}
fn save(app: &mut App, ctx: &egui::Context, path: &Path) {
    app.save_selected(ctx, path.to_path_buf());
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        app.editor.run_pending();
        app.poll_edits(ctx);
        app.finish_save();
        app.poll_native_saves(ctx);
        if path.exists()
            && app.bound.as_deref() == Some(path)
            && app.deferred_files.is_empty()
            && !app.editor.has_pending_edits()
        {
            break;
        }
        assert!(Instant::now() < until, "{}", app.editor.status());
        std::thread::yield_now();
    }
}
#[test]
fn real_studio_combined_fields_cancel_then_apply_one_undo_with_paused_native_fifo() {
    for asynchronous in [false, true] {
        let mut h = harness(app(asynchronous));
        h.run();
        let ctx = h.ctx.clone();
        let source = h.state().editor.document_snapshot();
        let ticket = h.state().editor.history_view().ticket;
        let layer = h.state().editor.target().unwrap();
        for apply in [false, true] {
            open(&mut h, Language::English);
            name(&mut h, "Combined");
            h.get_by_label("Visible").click();
            h.run();
            h.get_by_role_and_label(egui::Role::Slider, "Layer opacity")
                .set_value_accesskit(127.0);
            h.run();
            mode(&mut h, Language::English, "Screen");
            assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
            assert_eq!(h.state().editor.history_view().ticket, ticket);
            let release =
                (apply && asynchronous).then(|| h.state().editing.as_ref().unwrap().pause());
            h.get_by_label(if apply { "Apply" } else { "Cancel" })
                .click();
            h.run();
            if !apply {
                assert!(h.state().confirm.is_none());
                assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
            } else {
                assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
                if let Some(release) = release {
                    h.state_mut().editor.run_pending();
                    assert!(h.state().editor.has_pending_edits());
                    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
                    release.send(()).unwrap();
                }
                drain(h.state_mut());
            }
        }
        let applied = h.state().editor.document_snapshot();
        let value = &applied.layers[&layer];
        assert_eq!(value.name, "Combined");
        assert!(!value.visible);
        assert_eq!(value.opacity, 127);
        assert_eq!(value.blend_mode, Mode::Screen);
        assert_eq!(h.state().editor.undo_depth(), 1);
        assert_eq!(value.raster, source.layers[&layer].raster);
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, [31, 127, 241, 255].repeat(2));
        h.state_mut().editor.request(0x62c1, Action::Undo);
        drain(h.state_mut());
        assert!(h.state().editor.document().same_semantics(&source));
        h.state_mut().editor.request(0x62c2, Action::Redo);
        drain(h.state_mut());
        assert!(h.state().editor.document().same_semantics(&applied));
    }
}
#[test]
fn real_mode_choices_save_reopen_pcnd8_then_normal_reset_returns_old_minimum() {
    for asynchronous in [false, true] {
        for (label, selected, pixels) in [
            ("Multiply", Mode::Multiply, [25, 38, 12, 255]),
            ("Screen", Mode::Screen, [210, 166, 242, 255]),
        ] {
            let mut h = harness(app(asynchronous));
            h.run();
            let ctx = h.ctx.clone();
            let source = h.state().editor.document_snapshot();
            open(&mut h, Language::English);
            h.get_by_label("Apply").click();
            h.run();
            drain(h.state_mut());
            assert_eq!(
                h.state().editor.undo_depth(),
                0,
                "unchanged real dialog is a no-op"
            );
            assert!(h.state().editor.document().same_semantics(&source));
            open(&mut h, Language::English);
            mode(&mut h, Language::English, label);
            h.get_by_label("Apply").click();
            h.run();
            drain(h.state_mut());
            assert_eq!(
                h.state().editor.document().layers
                    [h.state().editor.document().roots.last().unwrap()]
                .blend_mode,
                selected
            );
            frame(h.state_mut(), &ctx);
            assert_eq!(h.state().rendered, pixels.repeat(2));
            let path = std::env::temp_dir()
                .join(format!("paintcraft-mode-controls-{}.pcnd", Uuid::new_v4()));
            save(h.state_mut(), &ctx, &path);
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(bytes[4], 8);
            assert!(
                Editor::open(&path)
                    .unwrap()
                    .document()
                    .same_semantics(h.state().editor.document())
            );
            open(&mut h, Language::English);
            mode(&mut h, Language::English, "Normal");
            h.get_by_label("Apply").click();
            h.run();
            drain(h.state_mut());
            frame(h.state_mut(), &ctx);
            assert_eq!(h.state().rendered, [204, 77, 13, 255].repeat(2));
            let normal = h.state().editor.document_snapshot();
            for id in &source.roots {
                assert_eq!(normal.layers[id].raster, source.layers[id].raster);
            }
            // Existing bound destination, fresh actual save token on next egui frame.
            h.run();
            save(h.state_mut(), &ctx, &path);
            assert_eq!(std::fs::read(&path).unwrap()[4], 1);
            assert!(
                Editor::open(&path)
                    .unwrap()
                    .document()
                    .same_semantics(&normal)
            );
            h.state_mut().editor.request(0x62c3, Action::Undo);
            drain(h.state_mut());
            frame(h.state_mut(), &ctx);
            assert_eq!(h.state().rendered, pixels.repeat(2));
            h.state_mut().editor.request(0x62c4, Action::Redo);
            drain(h.state_mut());
            assert!(h.state().editor.document().same_semantics(&normal));
            std::fs::remove_file(path).unwrap();
        }
    }
}
#[test]
fn real_localized_three_choices_and_old_presentation_entry_have_separate_controls() {
    for language in Language::ALL {
        let mut a = app(false);
        a.settings.language = language;
        let mut h = harness(a);
        h.run();
        let source = h.state().editor.document_snapshot();
        open(&mut h, language);
        for label in ["Multiply", "Screen", "Normal"] {
            mode(&mut h, language, label);
        }
        h.get_by_label(&translate(language, "Cancel")).click();
        h.run();
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.undo_depth(), 0);
    }
    let values = paintcraft_studio::LayerProperties {
        name: "Old entry".into(),
        visible: true,
        opacity: 255,
    };
    let mut old = Harness::builder().build_ui_state(
        |ui, values: &mut paintcraft_studio::LayerProperties| {
            paintcraft_studio::layer_properties_dialog(ui.ctx(), values);
        },
        values,
    );
    old.run();
    assert!(old.query_by_label("Layer blend mode").is_none());
}
#[test]
fn pending_capture_waits_actual_accepted_prefix_and_escape_or_stale_target_never_applies() {
    let mut a = app(true);
    let ctx = egui::Context::default();
    let layer = a.editor.target().unwrap();
    let release = a.editing.as_ref().unwrap().pause();
    a.editor.request(
        0x62c5,
        Action::Rename {
            layer,
            name: "Earlier prefix".into(),
        },
    );
    a.editor.run_pending();
    a.dispatch_layer_action(&ctx, LayerAction::Properties);
    let Some(Confirm::LayerProperties(draft)) = &a.confirm else {
        panic!("pending capture")
    };
    assert!(draft.ready.is_none());
    release.send(()).unwrap();
    drain(&mut a);
    let mut h = harness(a);
    h.run();
    let Some(Confirm::LayerProperties(draft)) = &h.state().confirm else {
        panic!("ready capture")
    };
    assert_eq!(draft.ready.as_ref().unwrap().values.name, "Earlier prefix");
    let before = h.state().editor.document_snapshot();
    let ticket = h.state().editor.history_view().ticket;
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.state().confirm.is_none());
    assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
    open(&mut h, Language::English);
    let bottom = h.state().editor.document().roots[0];
    h.state_mut().editor.set_target(bottom);
    h.run();
    assert!(h.state().confirm.is_none());
    assert_eq!(h.state().editor.history_view().ticket, ticket);
    open(&mut h, Language::English);
    let prior = h.state().tabs[0].id;
    assert_ne!(prior, h.state().active_tab_id());
    assert!(h.state_mut().swap_tab_state(prior));
    h.run();
    assert!(
        h.state().confirm.is_none(),
        "actual tab change retires draft"
    );
}

#[test]
fn new_properties_failure_envelopes_localize_all_seven_languages_and_keep_diagnostics() {
    fn status(language: Language, raw: &str) {
        let ctx = egui::Context::default();
        paintcraft_ui::i18n::set_language(&ctx, language);
        let detail = raw
            .strip_prefix("Edit failed: ")
            .expect("recognized existing envelope");
        assert!(!detail.is_empty());
        let translated = paintcraft_ui::i18n::tr_status(&ctx, raw);
        assert!(
            translated.contains(detail),
            "raw diagnostic retained: {language:?}"
        );
        if language == Language::English {
            assert_eq!(translated, raw);
        } else {
            assert_ne!(translated, raw, "envelope must localize: {language:?}");
        }
    }
    for language in Language::ALL {
        let mut a = app(false);
        let layer = a.editor.target().unwrap();
        let source = a.editor.document_snapshot();
        // Actual new acceptance validation branch, before callbacks/publication.
        let capture = a.editor.capture_properties(layer).unwrap();
        assert!(!a.editor.request_layer_properties(
            0x62d0,
            controller::PropertiesRequest {
                capture,
                desired_mode: Mode::Multiply,
                values: paintcraft_studio::LayerProperties {
                    name: "x".repeat(1025),
                    visible: false,
                    opacity: 1
                },
            }
        ));
        status(language, a.editor.status());
        assert!(Arc::ptr_eq(&source, &a.editor.document_snapshot()));
        // Actual queue-head revalidation branch; a target change is not rebased.
        let capture = a.editor.capture_properties(layer).unwrap();
        assert!(a.editor.request_layer_properties(
            0x62d1,
            controller::PropertiesRequest {
                capture,
                desired_mode: Mode::Screen,
                values: paintcraft_studio::LayerProperties {
                    name: "Must not apply".into(),
                    visible: false,
                    opacity: 1
                },
            }
        ));
        let bottom = a.editor.document().roots[0];
        a.editor.set_target(bottom);
        drain(&mut a);
        status(language, a.editor.status());
        assert!(Arc::ptr_eq(&source, &a.editor.document_snapshot()));
        assert_eq!(a.editor.undo_depth(), 0);
        // Actual pending-capture error branch: a real active file boundary cannot
        // be bypassed merely because its barrier is not an ordinary pending edit.
        let barrier = a.editor.queue_file_barrier();
        a.editor.run_pending();
        let ctx = egui::Context::default();
        a.dispatch_layer_action(&ctx, LayerAction::Properties);
        let Some(Confirm::LayerProperties(draft)) = a.confirm.take() else {
            panic!("capture intent")
        };
        assert!(draft.ready.is_none());
        a.layer_properties_dialog(&ctx, draft);
        status(language, a.editor.status());
        assert!(a.confirm.is_none());
        assert!(Arc::ptr_eq(&source, &a.editor.document_snapshot()));
        assert_eq!(a.editor.undo_depth(), 0);
        a.editor.release_file_barrier(barrier);
    }
}

#[test]
fn closing_real_last_studio_tab_disables_f4_and_refuses_placeholder_properties() {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    let ctx = egui::Context::default();
    // Keep a genuine previous dialog to exercise a late dispatch after close.
    app.dispatch_layer_action(&ctx, LayerAction::Properties);
    let Some(Confirm::LayerProperties(draft)) = app.confirm.take() else {
        panic!("real document properties")
    };
    app.request_close_tab(app.active_tab_id(), &ctx);
    assert_eq!(app.document_count(), 0);
    assert!(
        app.tabs.is_empty(),
        "actual close path, not a fabricated Vec state"
    );
    let source = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    let stamp = app.editor.preview_stamp();
    let owner = app.editor.properties_owner();
    // Real F4 input dispatch follows the normal enabled-state chrome boundary.
    let mut output = ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::F4,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ui| app.handle_global_input(ui),
    );
    output.textures_delta.clear();
    assert!(app.confirm.is_none());
    assert!(
        !paintcraft_ui::actions::Action::Layer(LayerAction::Properties)
            .enabled(app.action_context())
    );
    // Explicit properties chrome dispatch also remains closed.
    app.dispatch_chrome(
        &ctx,
        vec![paintcraft_ui::actions::Action::Layer(
            LayerAction::Properties,
        )],
    );
    assert!(app.confirm.is_none());
    // Private handler must retain the old App::request no-document fence too.
    app.dispatch_layer_action(&ctx, LayerAction::Properties);
    assert!(
        app.confirm.is_none(),
        "placeholder cannot become a properties modal"
    );
    app.layer_properties_dialog(&ctx, draft);
    assert!(app.confirm.is_none());
    app.editor.run_pending();
    assert_eq!(app.document_count(), 0);
    assert_eq!(app.editor.properties_owner(), owner);
    assert!(Arc::ptr_eq(&source, &app.editor.document_snapshot()));
    assert_eq!(app.editor.history_view().ticket, ticket);
    assert_eq!(app.editor.preview_stamp(), stamp);
    assert_eq!(app.editor.undo_depth(), 0);
    assert!(!app.editor.has_pending_edits());
    assert!(!app.editor.published_is_dirty());
}
