use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{self, Language, translate};
const SOURCE: &[u8] = include_bytes!(
    "../../../../../crates/services/paintcraft-formats/tests/abr-fixtures/native-presets.abr"
);
fn setup() -> (App, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("paintcraft-brush-ui-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("native.abr");
    std::fs::write(&source, SOURCE).unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.editor = Editor::with_document(
        paintcraft_document::new_document(16, 16, None, 0, paintcraft_document::POLICY).unwrap(),
        None,
    );
    app.begin_brushes();
    app.install_brushes(source.clone()).unwrap();
    (app, root, source)
}
#[test]
fn translated_brush_adoption_requires_conversion_consent_and_does_not_edit_until_pointer_release() {
    for language in [Language::English, Language::Japanese] {
        let (mut app, root, _) = setup();
        app.brushes.selected = 1;
        let before = app.editor.document_snapshot();
        assert!(app.adopt_brush().is_err());
        let mut h = Harness::builder()
            .with_max_steps(20)
            .with_size(egui::vec2(1000.0, 800.0))
            .build_ui_state(
                move |ui, app: &mut App| {
                    paintcraft_ui::fonts::install(ui.ctx());
                    ui.ctx()
                        .set_missing_glyph_policy(egui::epaint::text::MissingGlyphPolicy::Tofu);
                    i18n::set_language(ui.ctx(), language);
                    app.confirm_dialog(ui);
                },
                app,
            );
        h.run();
        h.get_by_role_and_label(
            egui::Role::CheckBox,
            &translate(language, "I accept the small change to brush spacing."),
        )
        .click();
        h.run();
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Use this brush"))
            .click();
        h.run();
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        assert!(!h.state().editor.is_dirty());
        let app = h.state_mut();
        app.editor.pointer_down((5, 5));
        app.editor.pointer_move((8, 5));
        let preview = app.editor.preview_batch().unwrap().unwrap();
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        app.editor.pointer_up();
        app.editor.run_pending();
        assert_eq!(app.editor.document().layers.len(), 1);
        assert_eq!(app.editor.undo_depth(), 1);
        let expected =
            paintcraft_editor::execute::execute(&before, app.editor.selection(), &preview)
                .unwrap()
                .doc;
        assert!(app.editor.document().same_semantics(&expected));
        app.editor.request(777, Action::Undo);
        app.editor.run_pending();
        assert!(app.editor.document().layers.is_empty());
        drop(h);
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn adopted_source_survives_library_replacement_rename_other_tabs_and_parked_workspace() {
    let (mut app, root, source) = setup();
    app.adopt_brush().unwrap();
    let adopted = app.editor.adopted_brush.clone().unwrap();
    app.begin_brushes();
    let replacement = root.join("replacement.abr");
    std::fs::write(&replacement, SOURCE).unwrap();
    app.install_brushes(replacement).unwrap();
    assert!(Arc::ptr_eq(
        adopted.source(),
        app.editor.adopted_brush.as_ref().unwrap().source()
    ));
    let moved = root.join("moved.abr");
    std::fs::rename(source, &moved).unwrap();
    let ctx = egui::Context::default();
    app.confirm = None;
    for place in 0..3 {
        if place == 1 {
            app.add_document_tab(Editor::new_document(2, 2).unwrap(), &ctx)
                .unwrap();
        }
        if place == 2 {
            let mut parked = tabs::WorkspaceDocuments::new((2, 2), false);
            parked.swap(&mut app);
            app.other_workspace = Some(Box::new(parked));
            app.settings.experience = Experience::Classic;
            assert!(app.editor.sampled_brush_name().is_none());
        }
        let alias = root.join(format!("image-{place}.png"));
        std::fs::hard_link(&moved, &alias).unwrap();
        app.do_export(&alias, ImageFormat::Png, &KNOWN_LOSSES.map(str::to_owned));
        assert_eq!(std::fs::read(alias).unwrap(), SOURCE);
        let alias = root.join(format!("project-{place}.pcnd"));
        std::fs::hard_link(&moved, &alias).unwrap();
        app.save_selected(&ctx, alias.clone());
        assert_eq!(std::fs::read(alias).unwrap(), SOURCE);
    }
    drop(app);
    drop(adopted);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn stale_cancelled_bad_and_unsupported_resource_choices_keep_prior_adoption_and_painting() {
    let (mut app, root, _) = setup();
    app.adopt_brush().unwrap();
    let before = app.editor.document_snapshot();
    let old = app.editor.adopted_brush.clone().unwrap();
    app.begin_brushes();
    app.brushes.pick = Some(app.brushes.epoch);
    app.complete_brush_picker(&egui::Context::default(), None);
    assert!(app.brushes.error.is_none());
    let bad = root.join("bad.abr");
    std::fs::write(&bad, b"bad").unwrap();
    app.brushes.pick = Some(app.brushes.epoch);
    app.complete_brush_picker(&egui::Context::default(), Some(bad));
    assert!(app.brushes.error.is_some());
    app.brushes.selected = 2;
    assert!(app.adopt_brush().is_err());
    app.brushes.pick = Some(Uuid::new_v4());
    app.complete_brush_picker(
        &egui::Context::default(),
        Some(root.join("must-not-read.abr")),
    );
    assert!(Arc::ptr_eq(
        old.source(),
        app.editor.adopted_brush.as_ref().unwrap().source()
    ));
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    drop(app);
    drop(old);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn sampled_native_worker_prepares_one_atomic_edit_and_cancelled_capture_never_publishes() {
    let (mut app, root, _) = setup();
    app.adopt_brush().unwrap();
    let before = app.editor.document_snapshot();
    let service = crate::controller::background::Service::new(egui::Context::default()).unwrap();
    app.editor.connect_editing(service);
    app.editor.pointer_down((5, 5));
    app.editor.pointer_move((8, 5));
    app.editor.pointer_up();
    app.editor.run_pending();
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.editor.has_pending_edits() {
        assert!(std::time::Instant::now() < end);
        app.editor.run_pending();
        std::thread::yield_now();
    }
    assert_eq!(app.editor.undo_depth(), 1);
    assert_eq!(app.editor.document().layers.len(), 1);
    assert!(!Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    let after = app.editor.document_snapshot();
    app.editor.pointer_down((8, 8));
    app.editor.pointer_move((9, 9));
    app.editor.cancel_gesture();
    app.editor.pointer_up();
    app.editor.run_pending();
    assert!(Arc::ptr_eq(&after, &app.editor.document_snapshot()));
    app.editor.request(778, Action::Undo);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.editor.undo_depth() != 0 || app.editor.has_pending_edits() {
        assert!(std::time::Instant::now() < end);
        app.editor.run_pending();
        std::thread::yield_now();
    }
    assert!(app.editor.document().layers.is_empty());
    app.editor.options.brush_opacity = 0;
    app.editor.pointer_down((4, 4));
    app.editor.pointer_up();
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        app.editor.run_pending();
        if !app.editor.has_pending_edits() {
            break;
        }
        assert!(std::time::Instant::now() < end);
        std::thread::yield_now();
    }
    assert_eq!(app.editor.undo_depth(), 0);
    assert!(app.editor.document().layers.is_empty());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
