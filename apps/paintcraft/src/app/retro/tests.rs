//! Real native owner/profile/controls, not a synthetic third-key store.
use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::brush::BrushMedia;
use paintcraft_tools::SelectionMode;

fn app() -> App {
    let mut app = App::new(None);
    app.settings.classic_variant = Some(ClassicVariant::V11);
    app.picking = None;
    app
}

#[test]
fn three_experiences_restore_exact_real_editor_owners_and_preferences() {
    let mut app = app();
    let ctx = egui::Context::default();
    let mut owners = Vec::new();
    for (index, experience) in [Experience::Classic, Experience::Studio, Experience::Retro]
        .into_iter()
        .enumerate()
    {
        app.choose_experience(experience);
        assert_eq!(app.experience(), experience);
        app.publish_editor(Editor::with_document(
            paintcraft_document::new_document(
                2,
                1,
                Some(Rgba([
                    11 + index as u8 * 33,
                    22 + index as u8 * 33,
                    33 + index as u8 * 33,
                    255,
                ])),
                0,
                paintcraft_document::POLICY,
            )
            .unwrap(),
            None,
        ));
        app.editor.color = Rgba([index as u8, 0, 0, 255]);
        app.zoom = index as f32 + 1.0;
        owners.push((
            experience,
            app.editor.document_snapshot(),
            app.editor.session_owner_id(),
            app.active_tab_id(),
        ));
    }
    for _ in 0..2 {
        for (index, (experience, source, session, tab)) in owners.iter().enumerate() {
            app.choose_experience(*experience);
            app.pump_interface_switch(&ctx);
            assert_eq!(app.experience(), *experience);
            assert!(Arc::ptr_eq(source, &app.editor.document_snapshot()));
            assert_eq!(app.editor.session_owner_id(), *session);
            assert_eq!(app.active_tab_id(), *tab);
            assert_eq!(app.editor.color, Rgba([index as u8, 0, 0, 255]));
            assert_eq!(app.zoom, index as f32 + 1.0);
        }
    }
}

#[test]
fn real_retro_toolbox_semantic_choices_and_modal_refusal_preserve_source() {
    let mut app = app();
    app.choose_experience(Experience::Retro);
    let source = app.editor.document_snapshot();
    let history = app.editor.history_view();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1100.0, 800.0))
        .build_ui_state(
            |ui, app: &mut App| {
                app.apply_style(ui.ctx());
                app.retro_toolbar(ui);
                app.retro_panels(ui);
                app.confirm_dialog(ui);
            },
            app,
        );
    h.run();
    h.get_by_role_and_label(egui::Role::Button, "Airbrush")
        .click();
    h.run();
    assert_eq!(h.state().editor.tool, Tool::Brush);
    assert_eq!(h.state().editor.options.media, BrushMedia::Airbrush);
    h.get_by_role_and_label(egui::Role::Button, "Brush").click();
    h.run();
    assert_eq!(h.state().editor.options.media, BrushMedia::Plain);
    h.get_by_role_and_label(egui::Role::Button, "Free-form")
        .click();
    h.run();
    assert_eq!(
        h.state().editor.options.selection_mode,
        SelectionMode::FreeForm
    );
    h.get_by_role_and_label(egui::Role::Button, "Select")
        .click();
    h.run();
    assert_eq!(
        h.state().editor.options.selection_mode,
        SelectionMode::Rectangle
    );
    let ctx = h.ctx.clone();
    h.state_mut().open_about(&ctx);
    let before = h.state().editor.options;
    h.state_mut().retro_choose_tool(ToolChoice::Airbrush);
    assert_eq!(h.state().editor.options, before);
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
    assert_eq!(h.state().editor.history_view(), history);
}

#[test]
fn retro_has_an_explicit_profile_and_light_style_preserves_preferred_theme() {
    let mut app = app();
    app.settings.appearance = settings::Appearance::Dark;
    app.choose_experience(Experience::Retro);
    assert_eq!(
        app.format_frontend(),
        paintcraft_formats::registry::Frontend::RetroWin95
    );
    let ctx = egui::Context::default();
    app.apply_style(&ctx);
    assert_eq!(ctx.theme(), egui::Theme::Light);
    assert_eq!(app.settings.appearance, settings::Appearance::Dark);
    app.choose_experience(Experience::Studio);
    app.apply_style(&ctx);
    assert_eq!(ctx.theme(), egui::Theme::Dark);
    assert_eq!(app.settings.appearance, settings::Appearance::Dark);
}

#[test]
fn retro_settings_roundtrip_and_legacy_defaults_preserve_language_and_other_skin() {
    let path = std::env::temp_dir().join(format!("paintcraft-retro-settings-{}", Uuid::new_v4()));
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(path.clone());
    let settings = Settings {
        experience: Experience::Retro,
        retro_skin: paintcraft_retro::Skin::Win95,
        language: paintcraft_ui::i18n::Language::Japanese,
        classic_variant: Some(ClassicVariant::V10),
        appearance: settings::Appearance::Dark,
        ..Default::default()
    };
    settings.save(&path).unwrap();
    let (loaded, result) = Settings::load(&path);
    assert_eq!(result, Loaded::Ok);
    assert_eq!(loaded, settings);
    let reopened = App::new(None).with_settings(path);
    assert_eq!(reopened.experience(), Experience::Retro);
    assert_eq!(reopened.retro.skin, paintcraft_retro::Skin::Win95);
    assert_eq!(reopened.language(), paintcraft_ui::i18n::Language::Japanese);
}

#[test]
fn captured_retro_color_and_canvas_cancel_leave_source_history_unchanged() {
    let mut app = app();
    app.choose_experience(Experience::Retro);
    let source = app.editor.document_snapshot();
    let history = app.editor.history_view();
    let color = app.editor.color;
    let mut h = Harness::builder()
        .with_size(egui::vec2(900.0, 700.0))
        .build_ui_state(
            |ui, app: &mut App| {
                app.apply_style(ui.ctx());
                app.confirm_dialog(ui);
            },
            app,
        );
    let ctx = h.ctx.clone();
    h.state_mut().begin_color_dialog(&ctx, false);
    h.run();
    h.get_by_role_and_label(egui::Role::SpinButton, "Red")
        .set_value_accesskit(123.0);
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().editor.color, color);
    h.state_mut().begin_canvas_dialog(&ctx, false);
    h.run();
    h.get_by_role_and_label(egui::Role::SpinButton, "Width")
        .set_value_accesskit(123.0);
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
    assert_eq!(h.state().editor.history_view(), history);
}

#[test]
fn retro_actual_open_path_refuses_foreign_source_without_owner_or_output_changes() {
    let path = std::env::temp_dir().join(format!("paintcraft-retro-format-{}", Uuid::new_v4()));
    std::fs::create_dir(&path).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(path.clone());
    let mut app = app();
    app.choose_experience(Experience::Retro);
    let source = app.editor.document_snapshot();
    let history = app.editor.history_view();
    let png =
        paintcraft_formats::image::encode(ImageFormat::Png, 1, 1, &[2, 3, 4, 255], 90).unwrap();
    for name in ["foreign.webp", "foreign.svg", "foreign.pdn", "wrong.jpg"] {
        let input = path.join(name);
        std::fs::write(&input, &png).unwrap();
        assert!(app.open_path(&input).is_err());
        assert_eq!(std::fs::read(&input).unwrap(), png);
        assert!(Arc::ptr_eq(&source, &app.editor.document_snapshot()));
        assert_eq!(app.editor.history_view(), history);
        assert!(app.bound.is_none());
    }
    let input = path.join("actual.png");
    std::fs::write(&input, &png).unwrap();
    app.open_path(&input).unwrap();
    assert_eq!(app.editor.document().canvas, (1, 1));
    let layer = &app.editor.document().layers[&app.editor.document().roots[0]];
    assert_eq!(layer.raster.get(0, 0), Rgba([2, 3, 4, 255]));
}
