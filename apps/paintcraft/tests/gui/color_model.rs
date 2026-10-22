use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

#[test]
fn color_model_popup_stages_choices_and_escape_dismisses_only_the_popup() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(700.0, 500.0), 2.0, Appearance::Dark),
        (Vec2::new(360.0, 360.0), 1.0, Appearance::Dark),
        (Vec2::new(480.0, 360.0), 2.0, Appearance::Light),
    ] {
        let dir = scratch("classic-color-model");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        let mut h = Harness::builder()
            .with_size(size)
            .with_pixels_per_point(ppp)
            .build_ui_state(
                |ui, app: &mut App| {
                    app.ui(ui);
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx()
                            .request_discard("test: repeated color model layout");
                    }
                },
                App::new(None).with_settings(dir.clone()),
            );
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let color = h.state().editor().color;
        h.get_by_role_and_label(Role::ColorWell, "Pencil color")
            .scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::ColorWell, "Pencil color")
            .click();
        h.run();
        h.get_by_label("Hex RGB").scroll_to_me();
        h.run();
        h.get_by_label("Hex RGB").focus();
        h.run();
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
        h.run();
        h.get_by_label("Hex RGB").type_text("#2080FF");
        h.run();
        let footer = h.get_by_label("OK").rect();
        h.get_by_label("Color model").scroll_to_me();
        h.run();
        h.get_by_label("Color model").click();
        h.run();
        for (label, selected) in [("RGB", true), ("HSL", false)] {
            let row = h.get_by_role_and_label(Role::Button, label);
            let rect = row.rect();
            assert_eq!(rect.height(), 40.0);
            assert!(
                rect.min.x >= 0.0
                    && rect.max.x <= size.x
                    && rect.min.y >= 0.0
                    && rect.max.y <= size.y,
                "{size:?}: {rect:?}"
            );
            assert_eq!(
                row.accesskit_node().toggled(),
                Some(if selected {
                    eframe::egui::accesskit::Toggled::True
                } else {
                    eframe::egui::accesskit::Toggled::False
                })
            );
        }
        h.key_press(Key::Escape);
        h.run();
        assert!(h.query_by_role_and_label(Role::Button, "HSL").is_none());
        assert!(
            h.query_by_label("Hex RGB").is_some(),
            "Escape must leave the parent color dialog open"
        );
        assert_eq!(
            h.get_by_label("Hex RGB")
                .accesskit_node()
                .value()
                .as_deref(),
            Some("#2080FF")
        );
        h.get_by_label("Color model").click();
        h.run();
        h.get_by_role_and_label(Role::Button, "HSL").click();
        h.run();
        assert!(h.query_by_label("Hue").is_some());
        assert!(h.query_by_label("Lightness").is_some());
        assert_eq!(h.get_by_label("OK").rect(), footer);
        assert_eq!(
            h.get_by_label("Hex RGB")
                .accesskit_node()
                .value()
                .as_deref(),
            Some("#2080FF")
        );
        assert_eq!(h.state().editor().color, color);
        h.get_by_label("Color model").click();
        h.run();
        assert_eq!(
            h.get_by_role_and_label(Role::Button, "HSL")
                .accesskit_node()
                .toggled(),
            Some(eframe::egui::accesskit::Toggled::True)
        );
        h.get_by_role_and_label(Role::Button, "RGB").click();
        h.run();
        assert!(h.query_by_label("Red").is_some());
        h.get_by_label("Cancel").click();
        h.run();
        assert!(h.query_by_label("Hex RGB").is_none());
        assert_eq!(h.state().editor().color, color);
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
