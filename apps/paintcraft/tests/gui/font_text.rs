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

#[test]
fn text_and_escape_in_popups_leave_unpublished_canvas_text_intact() {
    let mut h = harness_with(App::new(None), 1.0, true);
    h.get_by_label("Text").click();
    h.run();
    drag(&mut h, (50.0, 50.0), (50.0, 50.0));
    h.event(Event::Text("Keep me".into()));
    h.run();
    let snapshot = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    assert!(
        h.state().editor().is_editing_text(),
        "canvas typing should create a draft"
    );
    let text = |h: &Harness<App>| match h.state().editor().preview() {
        Some(paintcraft_tools::Preview::Text { text, .. }) => text,
        other => panic!("expected text preview, got {other:?}"),
    };
    assert_eq!(text(&h), "Keep me");
    for popup in ["Font family", "Font size presets", "File"] {
        h.get_by_label(popup).click();
        h.run();
        assert!(eframe::egui::Popup::is_any_open(&h.ctx));
        h.event(Event::Text("no-matching-family-90871".into()));
        h.run();
        if popup == "Font family" {
            assert!(h.query_by_label("No matching fonts").is_some());
        }
        assert!(
            h.state().editor().is_editing_text(),
            "{popup} typing must keep the canvas draft"
        );
        assert_eq!(
            text(&h),
            "Keep me",
            "{popup} cannot type into the canvas draft"
        );
        h.key_press(Key::Escape);
        h.run();
        assert!(!eframe::egui::Popup::is_any_open(&h.ctx));
        assert!(
            h.state().editor().is_editing_text(),
            "{popup} Escape must keep the canvas draft"
        );
        assert_eq!(
            text(&h),
            "Keep me",
            "Escape closes {popup} without discarding text"
        );
    }
    assert!(h.state().editor().document().same_semantics(&snapshot));
    assert_eq!(h.state().editor().history_view().ticket, ticket);
}

#[test]
fn typed_font_size_escape_preserves_preferences_and_unpublished_canvas_text() {
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("text-size-cancel");
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
        drag(&mut h, (50.0, 50.0), (50.0, 50.0));
        h.event(Event::Text("Keep me".into()));
        h.run();
        let original = Settings::load(&dir).0.text;
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let published = publications(&h);
        h.get_by_label("Font size").focus();
        h.run();
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
        h.run();
        h.get_by_label("Font size").type_text("40");
        h.run();
        assert_eq!(Settings::load(&dir).0.text, original);
        h.key_press(Key::Escape);
        h.run();
        assert_eq!(Settings::load(&dir).0.text, original);
        assert!(matches!(h.state().editor().preview(),
            Some(paintcraft_tools::Preview::Text { text, .. }) if text == "Keep me"));
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(publications(&h), published);
        h.get_by_label("Font size").focus();
        h.run();
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
        h.run();
        h.get_by_label("Font size").type_text("40");
        h.run();
        h.key_press(Key::Enter);
        h.run();
        assert_eq!(Settings::load(&dir).0.text.points, 40);
        assert!(h.state().editor().is_editing_text());
        assert_eq!(publications(&h), published);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
