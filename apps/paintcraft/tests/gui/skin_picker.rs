use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

#[test]
fn skin_picker_fits_small_windows_and_stages_choices_until_acceptance() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(700.0, 500.0), 2.0, Appearance::Dark),
        (Vec2::new(360.0, 360.0), 1.0, Appearance::Dark),
        (Vec2::new(480.0, 360.0), 2.0, Appearance::Light),
    ] {
        let dir = scratch("skin-picker-layout");
        Settings {
            appearance,
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        let build = || {
            Harness::builder()
                .with_size(size)
                .with_pixels_per_point(ppp)
                .build_ui_state(
                    |ui, app: &mut App| {
                        app.ui(ui);
                        if ui.ctx().current_pass_index() == 0 {
                            ui.ctx().request_discard("test: repeated skin layout");
                        }
                    },
                    App::new(None).with_settings(dir.clone()),
                )
        };
        let mut h = build();
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let bounds = h
            .get_by_role_and_label(Role::Window, "Choose your Classic skin")
            .rect();
        assert!(
            bounds.min.x >= 0.0
                && bounds.min.y >= 0.0
                && bounds.max.x <= size.x
                && bounds.max.y <= size.y,
            "{size:?}: {bounds:?}"
        );
        let footer = h.get_by_label("Continue").rect();
        for label in ["Paint v7", "Paint v10", "Paint v11"] {
            h.get_by_role_and_label(Role::RadioButton, label)
                .scroll_to_me();
            h.run();
            let card = h.get_by_role_and_label(Role::RadioButton, label).rect();
            assert!(card.min.x >= 0.0 && card.max.x <= size.x);
            assert!(card.max.y < footer.top());
            assert_eq!(h.get_by_label("Continue").rect(), footer);
        }
        h.get_by_role_and_label(Role::RadioButton, "Paint v7")
            .scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::RadioButton, "Paint v7")
            .click();
        h.run();
        assert_eq!(
            h.get_by_role_and_label(Role::RadioButton, "Paint v7")
                .accesskit_node()
                .toggled(),
            Some(eframe::egui::accesskit::Toggled::True)
        );
        assert_eq!(h.state().variant(), ClassicVariant::V11);
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        h.get_by_label("Cancel").click();
        h.run();
        assert!(!h.state().picker_open());
        assert_eq!(h.state().variant(), ClassicVariant::V11);
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        let mut h = build();
        h.run();
        assert!(
            h.state().picker_open(),
            "cancel leaves first-visit choice unsaved"
        );
        h.get_by_role_and_label(Role::RadioButton, "Paint v10")
            .scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::RadioButton, "Paint v10")
            .click();
        h.run();
        h.get_by_label("Continue").click();
        h.run();
        assert!(!h.state().picker_open());
        assert_eq!(h.state().variant(), ClassicVariant::V10);
        assert_eq!(
            Settings::load(&dir).0.classic_variant,
            Some(ClassicVariant::V10)
        );
        drop(h);
        let mut restarted = build();
        restarted.run();
        assert!(!restarted.state().picker_open());
        assert_eq!(restarted.state().variant(), ClassicVariant::V10);
        drop(restarted);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn changing_skin_and_escaping_preserves_unpublished_canvas_text() {
    let dir = scratch("skin-changer-apply");
    Settings {
        classic_variant: Some(ClassicVariant::V11),
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let saved = std::fs::read(dir.join("settings.json")).unwrap();
    let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, true);
    h.get_by_label("Text").click();
    h.run();
    drag(&mut h, (50.0, 50.0), (50.0, 50.0));
    h.event(Event::Text("Keep me".into()));
    h.run();
    let snapshot = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    h.get_by_role_and_label(Role::Button, "Interface").click();
    h.run();
    h.get_by_label("Change Classic skin…").click();
    h.run();
    assert!(h.state().picker_open());
    h.event(Event::Text("Do not append".into()));
    h.run();
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().picker_open());
    assert!(
        matches!(h.state().editor().preview(), Some(paintcraft_tools::Preview::Text { text, .. }) if text == "Keep me")
    );
    assert!(h.state().editor().document().same_semantics(&snapshot));
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
    h.get_by_role_and_label(Role::Button, "Interface").click();
    h.run();
    h.get_by_label("Change Classic skin…").click();
    h.run();
    h.get_by_role_and_label(Role::RadioButton, "Paint v10")
        .click();
    h.run();
    assert_eq!(h.state().variant(), ClassicVariant::V11);
    assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
    h.get_by_label("Apply").click();
    h.run();
    assert!(!h.state().picker_open());
    assert_eq!(h.state().variant(), ClassicVariant::V10);
    assert_eq!(
        Settings::load(&dir).0.classic_variant,
        Some(ClassicVariant::V10)
    );
    assert!(
        matches!(h.state().editor().preview(), Some(paintcraft_tools::Preview::Text { text, .. }) if text == "Keep me")
    );
    assert!(h.state().editor().document().same_semantics(&snapshot));
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
