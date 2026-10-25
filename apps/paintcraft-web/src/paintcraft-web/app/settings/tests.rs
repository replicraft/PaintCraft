use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{Language, language};
use std::sync::Arc;

fn harness(variant: ClassicVariant, size: egui::Vec2) -> Harness<'static, WebApp> {
    let mut h = Harness::builder()
        .with_size(size)
        .with_theme(egui::Theme::Light)
        .build_eframe(move |cc| {
            let mut app = WebApp::new(&cc.egui_ctx);
            app.variant = variant;
            app.preferences.variant = variant;
            app
        });
    h.run();
    h.get_by_label("Settings…").click();
    h.run();
    h
}
#[test]
fn settings_match_desktop_navigation_and_cancel_restores_theme_for_every_skin_and_size() {
    for skin in ClassicVariant::ALL {
        for size in [egui::vec2(1360.0, 900.0), egui::vec2(480.0, 360.0)] {
            let mut h = harness(skin, size);
            let before = h.state().preferences.clone();
            let document = h.state().editor.session.snapshot();
            for label in [
                "Appearance",
                "Canvas",
                "Text preferences",
                "About",
                "Apply",
                "Cancel",
                "Close settings",
            ] {
                let r = h.get_by_role_and_label(egui::Role::Button, label).rect();
                assert_eq!(r.height(), 40.0, "{label}");
                assert!(h.ctx.content_rect().contains_rect(r), "{label}: {r:?}");
            }
            if skin != ClassicVariant::V7 {
                h.get_by_label("Dark").scroll_to_me();
                h.run();
                h.get_by_label("Dark").click();
                h.run();
                assert_eq!(h.ctx.theme(), egui::Theme::Light);
            } else {
                assert!(h.query_by_label("Dark").is_none());
                assert_eq!(h.ctx.theme(), egui::Theme::Light);
            }
            assert_eq!(h.state().preferences, before);
            h.get_by_label("Close settings").click();
            h.run();
            assert_eq!(h.ctx.theme(), egui::Theme::Light);
            assert_eq!(h.state().preferences, before);
            assert!(Arc::ptr_eq(&document, &h.state().editor.session.snapshot()));
            assert!(!h.state().settings);
        }
    }
}
#[test]
fn apply_adopts_theme_and_skin_without_document_history_or_pixels() {
    let mut h = harness(ClassicVariant::V11, egui::vec2(1360.0, 900.0));
    let before = h.state().editor.session.snapshot();
    h.get_by_label("Dark").click();
    h.run();
    h.get_by_label(ClassicVariant::V7.name()).click();
    h.run();
    assert_eq!(h.state().variant, ClassicVariant::V11);
    h.get_by_label("Apply").click();
    h.run();
    assert_eq!(h.state().preferences.theme, Some(egui::Theme::Dark));
    assert_eq!(h.state().variant, ClassicVariant::V7);
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    assert_eq!(h.state().editor.session.undo_depth(), 0);
    let ctx = h.ctx.clone();
    h.state_mut().open_settings(&ctx);
    h.run();
    assert_eq!(h.ctx.theme(), egui::Theme::Light);
    assert!(h.query_by_label("Dark").is_none());
    h.get_by_label(ClassicVariant::V10.name()).click();
    h.run();
    assert_eq!(h.ctx.theme(), egui::Theme::Light);
    h.get_by_label("Apply").click();
    h.run();
    assert_eq!(h.state().preferences.theme, Some(egui::Theme::Dark));
    assert_eq!(h.state().variant, ClassicVariant::V10);
    assert_eq!(h.ctx.theme(), egui::Theme::Dark);
}
#[test]
fn canvas_defaults_are_staged_bounded_and_only_apply_to_new_documents() {
    let mut h = harness(ClassicVariant::V11, egui::vec2(1360.0, 900.0));
    let before = h.state().editor.session.ticket();
    h.get_by_role_and_label(egui::Role::Button, "Canvas")
        .click();
    h.run();
    for label in ["Default width", "Default height"] {
        h.get_by_label(label).set_value_accesskit(4096.0);
        h.run();
    }
    h.get_by_label("Apply").click();
    h.run();
    assert!(h.state().settings);
    assert_eq!(h.state().preferences.canvas.configured, (1000, 486));
    assert_eq!(h.state().editor.session.ticket(), before);
    h.get_by_label("Default width").set_value_accesskit(64.0);
    h.run();
    h.get_by_label("Default height").set_value_accesskit(32.0);
    h.run();
    h.get_by_label("Use configured default size").click();
    h.run();
    h.get_by_label("Apply").click();
    h.run();
    assert_eq!(h.state().preferences.canvas.configured, (64, 32));
    assert_eq!(h.state().editor.session.document().canvas, (1000, 486));
    assert_eq!(h.state().editor.session.ticket(), before);
    h.state_mut().new_document();
    h.run();
    assert_eq!(h.state().editor.session.document().canvas, (64, 32));
}
#[test]
fn text_preferences_and_about_share_desktop_controls_and_preserve_pixels() {
    let mut h = harness(ClassicVariant::V11, egui::vec2(1360.0, 900.0));
    let original = h.state().editor.session.snapshot();
    h.get_by_label("Text preferences").click();
    h.run();
    assert!(h.query_by_label("Default font").is_some());
    h.get_by_label("Keep text editable until save (prototype)")
        .click();
    h.run();
    assert!(!h.state().preferences.editable_until_save);
    h.get_by_label("About").click();
    h.run();
    assert!(h.query_by_label("PaintCraft by Replicraft").is_some());
    assert!(h.query_by_label("GitHub: replicraft/paintcraft").is_some());
    assert!(
        h.query_by_label(&format!("v{}", env!("CARGO_PKG_VERSION")))
            .is_some()
    );
    h.get_by_label("Apply").click();
    h.run();
    assert!(h.state().preferences.editable_until_save);
    assert!(Arc::ptr_eq(&original, &h.state().editor.session.snapshot()));
}
#[test]
fn cancelling_staged_language_keeps_saved_choices_and_real_chrome() {
    let mut h = harness(ClassicVariant::V11, egui::vec2(1360.0, 900.0));
    h.get_by_label("Language").click();
    h.run();
    h.get_by_label("日本語").click();
    h.run();
    assert_eq!(language(&h.ctx), Language::English);
    assert_eq!(h.state().preferences.language, Language::English);
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(language(&h.ctx), Language::English);
    assert_eq!(h.state().preferences.language, Language::English);
    assert!(
        h.query_by_role_and_label(egui::Role::Button, "File")
            .is_some()
    );
}

#[test]
fn skin_and_theme_are_staged_and_paint7_restores_the_preferred_theme_only_after_apply() {
    let mut h = super::super::chrome_tests::harness(Frontend::Classic, ClassicVariant::V11, 1360.0);
    let ticket = h.state().editor.session.ticket();
    h.get_by_label("Settings…").click();
    h.run();
    h.get_by_label(ClassicVariant::V7.name()).click();
    h.run();
    assert_eq!(h.ctx.theme(), egui::Theme::Dark);
    assert_eq!(h.state().variant, ClassicVariant::V11);
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.ctx.theme(), egui::Theme::Dark);
    h.get_by_label("Settings…").click();
    h.run();
    h.get_by_label(ClassicVariant::V7.name()).click();
    h.run();
    h.get_by_label("Apply").click();
    h.run();
    assert_eq!(h.state().variant, ClassicVariant::V7);
    assert_eq!(h.ctx.theme(), egui::Theme::Light);
    h.get_by_label("Settings…").click();
    h.run();
    h.get_by_label(ClassicVariant::V11.name()).click();
    h.run();
    assert_eq!(h.ctx.theme(), egui::Theme::Light);
    assert_eq!(h.state().variant, ClassicVariant::V7);
    h.get_by_label("Apply").click();
    h.run();
    assert_eq!(h.ctx.theme(), egui::Theme::Dark);
    assert_eq!(h.state().variant, ClassicVariant::V11);
    assert_eq!(h.state().editor.session.ticket(), ticket);
}

