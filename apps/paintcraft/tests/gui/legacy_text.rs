use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Appearance, ClassicVariant, Settings};
use paintcraft_ui::i18n::{Language, translate};

#[test]
fn historical_text_tab_keeps_font_controls_compact_and_preserves_the_live_draft() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
                let dir = scratch("historical-context-text");
                Settings {
                    classic_variant: Some(variant),
                    language,
                    appearance,
                    ..Default::default()
                }
                .save(&dir)
                .unwrap();
                let mut h =
                    harness_with(App::new(None).with_settings(dir.clone()), ppp, ppp == 2.0);
                h.set_size(Vec2::new(1360.0, 900.0));
                h.run();
                let label = |s| translate(language, s);
                h.get_by_role_and_label(Role::Button, &label("Text"))
                    .click();
                h.run();
                assert_eq!(h.get_by_label(&label("Font family")).rect().height(), 24.0);
                assert_eq!(
                    h.get_by_label(&label("Bold")).rect().size(),
                    Vec2::splat(24.0)
                );
                h.get_by_label(&label("Font family")).click();
                h.run();
                h.get_by_label(&label("Search fonts")).type_text("Selawik");
                h.run();
                h.key_press(Key::Enter);
                h.run();
                assert_eq!(Settings::load(&dir).0.text.family, "Selawik");
                h.get_by_label(&label("Font size presets")).click();
                h.run();
                h.get_by_label("24 pt").scroll_to_me();
                h.run();
                h.get_by_label("24 pt").click();
                h.run();
                h.get_by_label(&label("Bold")).click();
                h.run();
                h.get_by_label(&label("Opaque")).click();
                h.run();
                let style = Settings::load(&dir).0.text;
                assert_eq!(style.points, 24);
                assert!(style.bold && style.background_fill);
                let snapshot = h.state().editor().document_snapshot();
                let ticket = h.state().editor().history_view().ticket;
                drag(&mut h, (50.0, 50.0), (50.0, 50.0));
                h.event(Event::Text("Classic".into()));
                h.run();
                assert!(h.state().editor().is_editing_text());
                for tab in ["Home", "View"] {
                    h.get_by_role_and_label(Role::Button, &label(tab)).click();
                    h.run();
                    assert!(h.query_by_label(&label("Font family")).is_none());
                    assert!(h.state().editor().is_editing_text());
                    h.get_by_role_and_label(Role::Tab, &label("Text")).click();
                    h.run();
                    assert!(h.query_by_label(&label("Font family")).is_some());
                }
                h.get_by_label(&label("Transparent")).click();
                h.run();
                assert!(!Settings::load(&dir).0.text.background_fill);
                assert!(h.state().editor().document().same_semantics(&snapshot));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
                h.event(key(Key::Enter, Modifiers::COMMAND));
                h.run();
                assert!(!h.state().editor().is_editing_text());
                assert!(h.state().editor().is_dirty());
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}

#[test]
#[ignore = "Explicit offscreen historical contextual Text ribbon captures"]
fn export_historical_text_ribbons() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("paintcraft-historical-text"));
    std::fs::create_dir_all(&output).unwrap();
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        let dir = scratch("historical-text-capture");
        Settings {
            classic_variant: Some(variant),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(1100.0, 720.0));
        h.run();
        h.get_by_role_and_label(Role::Button, "Text").click();
        h.run();
        h.get_by_label("Font size").set_value_accesskit(36.0);
        h.run();
        h.get_by_label("Bold").click();
        h.run();
        drag(&mut h, (70.0, 70.0), (70.0, 70.0));
        h.event(Event::Text("Classic text".into()));
        h.run();
        h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(output.join(format!("classic-{}-text.png", variant.id())))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
