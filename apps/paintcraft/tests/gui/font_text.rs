use super::*;
use paintcraft::settings::{Appearance, ClassicVariant, Settings};
#[test]
#[ignore = "requires GPU; exports real font text and updated Classic controls"]
fn render_font_text_for_review() {
    let dir = scratch("font-text-preview");
    let output = std::env::temp_dir().join("paintcraft-font-previews");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, name) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.get_by_label("Text").click();
        h.run();
        h.get_by_label("Font size").set_value_accesskit(36.0);
        h.run();
        let p = at(&h, 60.0, 65.0);
        h.input_mut()
            .events
            .extend([Event::PointerMoved(p), press(p), release(p)]);
        h.run();
        h.event(Event::Text("PaintCraft — Selawik".into()));
        h.run();
        h.get_by_label("Bold").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("classic-{name}-font-text.png")))
            .unwrap();
        h.event(key(Key::Enter, Modifiers::COMMAND));
        h.run();
        assert!(h.state().editor().is_dirty());
        h.get_by_label("Pencil").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("classic-{name}-pencil-size.png")))
            .unwrap();
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn classic_text_size_presets_and_custom_values_persist_without_editing_pixels() {
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("text-size-presets");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.get_by_label("Text").click();
        h.run();
        let before = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for label in [
            "Font family",
            "Font size",
            "Font size presets",
            "Background fill",
        ] {
            let rect = h.get_by_label(label).rect();
            assert!(
                rect.min.x >= 0.0 && rect.max.x <= 1100.0,
                "{label}: {rect:?}"
            );
            assert!(rect.max.y < h.state().canvas_viewport().unwrap().min.y);
        }
        h.get_by_label("Font size presets").click();
        h.run();
        h.get_by_label("36 pt").scroll_to_me();
        h.run();
        h.get_by_label("36 pt").click();
        h.run();
        assert_eq!(Settings::load(&dir).0.text.points, 36);
        assert!(
            h.query_by_label("36 pt").is_none(),
            "choosing a preset closes its popup"
        );
        h.get_by_label("Font size").set_value_accesskit(37.0);
        h.run();
        assert_eq!(Settings::load(&dir).0.text.points, 37);
        h.get_by_label("Bold").click();
        h.run();
        assert!(
            Settings::load(&dir).0.text.bold,
            "multipass does not toggle twice"
        );
        h.get_by_label("Align center").click();
        h.run();
        assert_eq!(
            Settings::load(&dir).0.text.alignment,
            paintcraft_classic::TextAlignment::Center
        );
        assert!(h.state().editor().document().same_semantics(&before));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn classic_font_search_reports_no_matches_and_does_not_change_the_selected_font() {
    let dir = scratch("text-font-search");
    Settings {
        classic_variant: Some(ClassicVariant::V11),
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, true);
    h.get_by_label("Text").click();
    h.run();
    h.get_by_label("Font family").click();
    h.run();
    h.get_by_label("Search fonts").focus();
    h.run();
    h.get_by_label("Search fonts")
        .type_text("no-matching-family-982341");
    h.run();
    assert!(h.query_by_label("No matching fonts").is_some());
    assert_eq!(Settings::load(&dir).0.text.family, "Selawik");
    assert!(!h.state().editor().is_dirty());
    h.key_press(Key::Escape);
    h.run();
    assert!(h.query_by_label("Search fonts").is_none());
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
