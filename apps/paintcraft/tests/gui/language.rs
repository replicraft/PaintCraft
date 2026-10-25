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

#[test]
fn japanese_classic_dialog_footers_and_tooltips_use_the_selected_language() {
    use eframe::egui::{Role, epaint::Shape};
    for variant in ClassicVariant::ALL {
        let dir = scratch(&format!("japanese-dialogs-{}", variant.id()));
        Settings {
            classic_variant: Some(variant),
            language: Language::Japanese,
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        let source = h.state().editor().document_snapshot();
        h.event(key(Key::E, Modifiers::COMMAND));
        h.run();
        assert!(
            h.query_by_role_and_label(Role::Button, "キャンセル")
                .is_some()
        );
        assert!(h.query_by_role_and_label(Role::Button, "Cancel").is_none());
        assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
            Shape::Text(text) if text.galley.job.text == "画像のプロパティ")));
        assert!(h.query_by_label("幅").is_some());
        h.get_by_label("キャンセル").click();
        h.run();
        if variant == ClassicVariant::V11 {
            h.ctx.style_mut_of(h.ctx.theme(), |style| {
                style.interaction.tooltip_delay = 0.0;
                style.interaction.show_tooltips_only_when_still = false;
            });
            h.get_by_label("線の太さ").hover();
            h.run();
            assert!(
                h.query_by_role_and_label(
                    Role::Label,
                    "線や図形を選ぶと、線の太さを変更できます。"
                )
                .is_some()
            );
        }
        assert!(h.state().editor().document().same_semantics(&source));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn japanese_classic_translates_tool_sizes_view_and_selection_menus() {
    use eframe::egui::Role;
    let dir = scratch("japanese-menu-sweep");
    Settings {
        classic_variant: Some(ClassicVariant::V11),
        language: Language::Japanese,
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
    h.set_size(Vec2::new(1360.0, 900.0));
    h.run();
    assert!(!h.state().picker_open());
    h.get_by_role_and_label(Role::Button, "鉛筆").click();
    h.run();
    assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Pencil);
    assert!(
        h.query_by_role_and_label(Role::Image, "鉛筆のサイズ")
            .is_some()
    );
    assert!(
        h.query_by_role_and_label(Role::Slider, "ツールのサイズ")
            .is_some()
    );
    h.get_by_label("表示").click();
    h.run();
    assert!(h.query_by_label("ウィンドウに合わせる").is_some());
    assert!(h.query_by_label("ツールバーを自動的に隠す").is_some());
    assert!(h.query_by_label("Automatically hide toolbar").is_none());
    h.event(key(Key::Escape, Modifiers::NONE));
    h.run();
    h.get_by_label("選択のオプション").click();
    h.run();
    assert!(h.query_by_label("選択範囲の反転").is_some());
    assert!(h.query_by_label("Invert selection").is_none());
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn all_language_choices_apply_persist_and_restore_in_classic_and_studio() {
    use paintcraft_ui::i18n::{language, translate};
    for experience in [Experience::Classic, Experience::Studio] {
        for selected in Language::ALL {
            let dir = scratch(&format!("locale-{}-{experience:?}", selected.id()));
            Settings {
                experience,
                classic_variant: Some(ClassicVariant::V11),
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
            h.set_size(Vec2::new(1360.0, 900.0));
            h.run();
            let original = h.state().editor().document_snapshot();
            h.get_by_label("Settings…").click();
            h.run();
            h.get_by_label("Language").click();
            h.run();
            h.get_by_label(selected.name()).click();
            h.run();
            assert_eq!(language(&h.ctx), selected);
            h.get_by_label(&translate(selected, "Apply")).click();
            h.run();
            assert_eq!(Settings::load(&dir).0.language, selected);
            assert!(h.state().editor().document().same_semantics(&original));
            drop(h);
            let restored = App::new(None).with_settings(dir.clone());
            let mut h = harness_with(restored, 1.0, false);
            h.run();
            assert_eq!(language(&h.ctx), selected);
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
