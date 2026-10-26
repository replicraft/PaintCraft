use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{self, Language, translate};
use std::sync::Arc;
const SOURCE: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/csh-fixtures/paths.csh"
);
fn stage(app: &mut WebApp, ctx: &egui::Context) {
    app.frontend = Frontend::Studio;
    app.begin_shapes();
    app.accept_shape(
        ctx,
        app.shapes.epoch,
        Loaded::new(SOURCE, "colors.csh".into()).map(Some),
    );
}
#[test]
fn translated_browser_shape_selection_preserves_document_history_and_title() {
    for language in [Language::English, Language::Japanese] {
        let ctx = egui::Context::default();
        let mut app = WebApp::new(&ctx);
        stage(&mut app, &ctx);
        let before = app.editor.session.snapshot();
        let ticket = app.editor.session.ticket();
        let dirty = app.editor.session.is_dirty();
        let name = app.name.clone();
        assert_eq!(
            app.shapes.loaded.as_ref().unwrap().view.shapes[0].paths[0].knots[0],
            [1.0, -1.0 / 16_777_216.0, 1.0 / 16_777_216.0, 0.5, 0.0, -1.0]
        );
        let mut h = Harness::builder()
            .with_max_steps(20)
            .with_size(egui::vec2(1360.0, 900.0))
            .build_ui_state(
                move |ui, app: &mut WebApp| {
                    if ui.ctx().missing_glyph_policy()
                        != egui::epaint::text::MissingGlyphPolicy::Tofu
                    {
                        paintcraft_ui::fonts::install(ui.ctx());
                        ui.ctx()
                            .set_missing_glyph_policy(egui::epaint::text::MissingGlyphPolicy::Tofu);
                    }
                    i18n::set_language(ui.ctx(), language);
                    app.dialogs(ui.ctx());
                },
                app,
            );
        h.run();
        h.get_by_role_and_label(egui::Role::Button, "Shape 2")
            .click();
        h.run();
        assert_eq!(h.state().shapes.selected, 2);
        assert_eq!(h.state().name, name);
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert_eq!(h.state().editor.session.ticket(), ticket);
        assert_eq!(h.state().editor.session.is_dirty(), dirty);
        assert_eq!(
            h.state().shapes.loaded.as_ref().unwrap().library.as_ref(),
            &csh::decode(SOURCE, &|| Ok(())).unwrap()
        );
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Close"))
            .click();
        h.run();
        assert!(!h.state().shapes.open);
    }
}
#[test]
fn browser_shape_import_failure_cancellation_and_stale_picker_preserve_previous_resource() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    stage(&mut app, &ctx);
    let before = app.editor.session.snapshot();
    let bytes = csh::encode(&app.shapes.loaded.as_ref().unwrap().library, &|| Ok(())).unwrap();
    app.accept_shape(&ctx, app.shapes.epoch, Ok(None));
    app.accept_shape(
        &ctx,
        app.shapes.epoch,
        Loaded::new(b"cush", "bad.csh".into()).map(Some),
    );
    assert!(app.shapes.error.is_some());
    let old = app.shapes.epoch;
    app.begin_shapes();
    app.accept_shape(&ctx, old, Loaded::new(SOURCE, "stale.csh".into()).map(Some));
    assert_eq!(app.shapes.loaded.as_ref().unwrap().view.name, "colors.csh");
    app.frontend = Frontend::Classic;
    app.accept_shape(
        &ctx,
        app.shapes.epoch,
        Loaded::new(SOURCE, "classic.csh".into()).map(Some),
    );
    assert_eq!(
        csh::encode(&app.shapes.loaded.as_ref().unwrap().library, &|| Ok(())).unwrap(),
        bytes
    );
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
}
#[test]
fn narrow_shape_dialog_keeps_file_actions_and_close_visible() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    stage(&mut app, &ctx);
    let mut h = Harness::builder()
        .with_max_steps(20)
        .with_size(egui::vec2(390.0, 640.0))
        .build_ui_state(
            |ui, app: &mut WebApp| {
                if ui.ctx().missing_glyph_policy() != egui::epaint::text::MissingGlyphPolicy::Tofu {
                    paintcraft_ui::fonts::install(ui.ctx());
                    ui.ctx()
                        .set_missing_glyph_policy(egui::epaint::text::MissingGlyphPolicy::Tofu);
                }
                app.dialogs(ui.ctx());
            },
            app,
        );
    h.run();
    for label in ["Import shapes…", "Export shapes…", "Close"] {
        let rect = h.get_by_role_and_label(egui::Role::Button, label).rect();
        assert!(
            h.ctx.content_rect().contains_rect(rect),
            "{label}: {rect:?}"
        );
    }
    h.get_by_role_and_label(egui::Role::Button, "Close").click();
    h.run();
    assert!(!h.state().shapes.open);
}
