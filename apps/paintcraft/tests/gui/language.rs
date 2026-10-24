use super::*;
use paintcraft::settings::{ClassicVariant, Experience, Settings};
use paintcraft_ui::i18n::Language;
#[test]
fn japanese_settings_preview_cancel_and_apply_preserve_the_document_for_every_skin() {
    for (experience, variant) in [
        (Experience::Classic, ClassicVariant::V7),
        (Experience::Classic, ClassicVariant::V10),
        (Experience::Classic, ClassicVariant::V11),
        (Experience::Studio, ClassicVariant::V11),
    ] {
        let dir = scratch("japanese-settings");
        Settings {
            experience,
            classic_variant: Some(variant),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, true);
        h.set_size(Vec2::new(1360.0, 900.0));
        h.run();
        let doc = h.state().editor().document_snapshot();
        h.get_by_label("Settings…").click();
        h.run();
        h.get_by_label("Language").click();
        h.run();
        h.get_by_label("日本語").click();
        h.run();
        h.run();
        assert_eq!(paintcraft_ui::i18n::language(&h.ctx), Language::Japanese);
        h.get_by_label("テキストの設定").click();
        h.run();
        assert!(h.query_by_label("既定のフォント").is_some());
        h.get_by_label("キャンセル").click();
        h.run();
        assert_eq!(paintcraft_ui::i18n::language(&h.ctx), Language::English);
        assert_eq!(Settings::load(&dir).0.language, Language::English);
        h.get_by_label("Settings…").click();
        h.run();
        h.get_by_label("Language").click();
        h.run();
        h.get_by_label("日本語").click();
        h.run();
        h.run();
        h.get_by_label("適用").click();
        h.run();
        assert_eq!(Settings::load(&dir).0.language, Language::Japanese);
        assert!(h.state().editor().document().same_semantics(&doc));
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
