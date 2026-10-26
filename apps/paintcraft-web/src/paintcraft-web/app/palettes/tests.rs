use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{self, Language, translate};
use std::sync::Arc;
const SOURCE: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/ase-fixtures/mixed.ase"
);
fn stage(app: &mut WebApp, ctx: &egui::Context) {
    app.frontend = Frontend::Studio;
    app.begin_palettes();
    app.accept_palette(
        ctx,
        app.palettes.epoch,
        Loaded::new(SOURCE, "colors.ase".into()).map(Some),
    );
}
#[test]
fn translated_browser_palette_controls_pick_secondary_without_publishing_or_changing_title() {
    for language in [Language::English, Language::Japanese] {
        let ctx = egui::Context::default();
        let mut app = WebApp::new(&ctx);
        stage(&mut app, &ctx);
        app.studio.edit_secondary = true;
        let before = app.editor.session.snapshot();
        let ticket = app.editor.session.ticket();
        let dirty = app.editor.session.is_dirty();
        let name = app.name.clone();
        let primary = app.controls.primary;
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
        assert!(
            h.get_by_role_and_label(egui::Role::Button, "Cyan")
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_role_and_label(egui::Role::Button, "赤 🚀").click();
        h.run();
        assert_eq!(
            h.state().controls.secondary,
            paintcraft_core::types::Rgba([64, 128, 255, 255])
        );
        assert_eq!(h.state().controls.primary, primary);
        assert_eq!(h.state().name, name);
        assert!(!h.state().palettes.open);
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert_eq!(h.state().editor.session.ticket(), ticket);
        assert_eq!(h.state().editor.session.is_dirty(), dirty);
        assert_eq!(
            ase::encode(
                &h.state().palettes.loaded.as_ref().unwrap().palette,
                &|| Ok(())
            )
            .unwrap(),
            SOURCE
        );
        h.state_mut().begin_palettes();
        h.run();
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Close"))
            .click();
        h.run();
        assert!(!h.state().palettes.open);
    }
}
#[test]
fn browser_palette_import_failure_cancellation_and_stale_picker_preserve_previous_resource() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    stage(&mut app, &ctx);
    let before = app.editor.session.snapshot();
    let bytes = ase::encode(&app.palettes.loaded.as_ref().unwrap().palette, &|| Ok(())).unwrap();
    app.accept_palette(&ctx, app.palettes.epoch, Ok(None));
    app.accept_palette(
        &ctx,
        app.palettes.epoch,
        Loaded::new(b"ASEF", "bad.ase".into()).map(Some),
    );
    assert!(app.palettes.error.is_some());
    let old = app.palettes.epoch;
    app.begin_palettes();
    app.accept_palette(&ctx, old, Loaded::new(SOURCE, "stale.ase".into()).map(Some));
    assert_eq!(
        app.palettes.loaded.as_ref().unwrap().view.name,
        "colors.ase"
    );
    app.frontend = Frontend::Classic;
    app.accept_palette(
        &ctx,
        app.palettes.epoch,
        Loaded::new(SOURCE, "classic.ase".into()).map(Some),
    );
    assert_eq!(
        ase::encode(&app.palettes.loaded.as_ref().unwrap().palette, &|| Ok(())).unwrap(),
        bytes
    );
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
}
#[test]
fn narrow_palette_dialog_keeps_file_actions_and_close_visible() {
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
    for label in ["Import palette…", "Export palette…", "Close"] {
        let rect = h.get_by_role_and_label(egui::Role::Button, label).rect();
        assert!(
            h.ctx.content_rect().contains_rect(rect),
            "{label}: {rect:?}"
        );
    }
    h.get_by_role_and_label(egui::Role::Button, "Close").click();
    h.run();
    assert!(!h.state().palettes.open);
}
