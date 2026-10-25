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

#[test]
fn skin_picker_blocks_background_edits_history_and_shortcuts_until_dismissed() {
    for ppp in [1.0, 2.0] {
        let dir = scratch("skin-picker-input-isolation");
        Settings {
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.set_size(Vec2::new(1360.0, 900.0));
        h.run();
        h.get_by_label("Pencil").click();
        h.run();
        let start = at(&h, 10.5, 10.5);
        h.input_mut()
            .events
            .extend([Event::PointerMoved(start), press(start), release(start)]);
        h.run();
        assert!(h.state().editor().is_dirty());
        h.get_by_label("Canvas zoom").set_value_accesskit(200.0);
        h.run();
        let canvas = h.state().canvas_rect().unwrap();
        let scale = canvas.width() / 1000.0;
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let before = publications(&h);
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        h.get_by_label("Interface").click();
        h.run();
        h.get_by_label("Change Classic skin…").click();
        h.run();
        assert!(h.state().picker_open());
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::Z]);
        h.run();
        assert!(
            h.state().editor().document().same_semantics(&snapshot),
            "skin selection must block background undo"
        );
        h.get_by_role_and_label(Role::Button, "Brush").click();
        h.run();
        assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Pencil);
        let right = canvas
            .right()
            .min(h.state().canvas_viewport().unwrap().right())
            - 12.0;
        let x = ((right - canvas.left()) / scale).floor() as i64;
        let y = ((canvas.top().max(h.state().canvas_viewport().unwrap().top()) + 20.0
            - canvas.top())
            / scale)
            .floor() as i64;
        let point = canvas.min + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * scale;
        h.input_mut()
            .events
            .extend([Event::PointerMoved(point), press(point), release(point)]);
        h.run();
        assert!(h.state().picker_open());
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        h.key_press(Key::Escape);
        h.run();
        assert!(!h.state().picker_open());
        h.input_mut()
            .events
            .extend([Event::PointerMoved(point), press(point), release(point)]);
        h.run();
        assert_eq!(top_pixel(&h, x, y), Rgba([0, 0, 0, 255]));
        assert_eq!(publications(&h), before + 1);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit GPU skin chooser review"]
fn export_skin_chooser_surfaces() {
    let output = std::env::temp_dir().join("paintcraft-skin-chooser-surfaces");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        for (size, name) in [
            (Vec2::new(1360.0, 900.0), "desktop"),
            (Vec2::new(360.0, 360.0), "compact"),
        ] {
            for first_visit in [true, false] {
                let dir = scratch("skin-chooser-review");
                Settings {
                    appearance,
                    classic_variant: (!first_visit).then_some(ClassicVariant::V11),
                    ..Default::default()
                }
                .save(&dir)
                .unwrap();
                let saved = std::fs::read(dir.join("settings.json")).unwrap();
                let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
                h.set_size(size);
                h.run();
                if !first_visit {
                    h.get_by_label("Interface").click();
                    h.run();
                    h.get_by_label("Change Classic skin…").click();
                    h.run();
                }
                let mode = if first_visit { "first-visit" } else { "change" };
                h.render()
                    .unwrap()
                    .save(output.join(format!("{theme}-{name}-{mode}.png")))
                    .unwrap();
                h.get_by_role_and_label(Role::RadioButton, "Paint v7")
                    .scroll_to_me();
                h.run();
                h.get_by_role_and_label(Role::RadioButton, "Paint v7")
                    .click();
                h.run();
                assert_eq!(h.state().variant(), ClassicVariant::V11);
                h.get_by_label("Close skin chooser").click();
                h.run();
                assert!(!h.state().picker_open());
                assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}
