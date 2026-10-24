use crate::WebApp;
use eframe::egui::{Role, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::Language;
#[test]
fn browser_language_changes_real_chrome_settings_and_keeps_document_pixels() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    let original = h.state().editor.session.snapshot();
    h.get_by_label("Settings…").click();
    h.run();
    h.get_by_label("Language").click();
    h.run();
    h.get_by_label("日本語").click();
    h.run();
    h.run();
    assert_eq!(paintcraft_ui::i18n::language(&h.ctx), Language::Japanese);
    assert!(h.query_by_label("既定のフォント").is_some());
    assert!(h.query_by_label("ダークモード").is_some());
    assert!(h.query_by_label("クラシックのスキン").is_some());
    assert!(h.query_by_role_and_label(Role::Label, "図形").is_some());
    assert!(
        h.query_by_role_and_label(Role::Button, "ファイル")
            .is_some()
    );
    assert!(
        h.state()
            .editor
            .session
            .document()
            .same_semantics(&original)
    );
}

#[test]
fn browser_offers_every_language_and_translates_actual_settings_chrome() {
    use paintcraft_ui::i18n::{language, translate};
    for selected in Language::ALL {
        let mut h = Harness::builder()
            .with_size(Vec2::new(1360.0, 900.0))
            .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        h.run();
        let original = h.state().editor.session.snapshot();
        h.get_by_label("Settings…").click();
        h.run();
        h.get_by_label("Language").click();
        h.run();
        for choice in Language::ALL {
            assert!(
                h.query_by_label(choice.name()).is_some(),
                "Missing {}",
                choice.id()
            );
        }
        h.get_by_label(selected.name()).click();
        h.run();
        assert_eq!(language(&h.ctx), selected);
        assert!(
            h.query_by_label(&translate(selected, "Default font"))
                .is_some(),
            "{}",
            selected.id()
        );
        assert!(
            h.state()
                .editor
                .session
                .document()
                .same_semantics(&original)
        );
    }
}
