use super::*;
use eframe::egui::{Role, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

fn has_full_icon(h: &Harness<App>, row: eframe::egui::Rect) -> bool {
    h.output().shapes.iter().any(|shape| matches!(&shape.shape, Shape::Rect(image)
        if image.brush.is_some() && image.rect.size() == Vec2::splat(22.0) && row.contains_rect(image.rect)))
}

#[test]
fn classic_main_menu_rows_use_full_icons_and_keep_lower_actions_reachable() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(700.0, 500.0), 2.0, Appearance::Dark),
        (Vec2::new(360.0, 360.0), 2.0, Appearance::Light),
    ] {
        let dir = scratch("classic-menu-row-metrics");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = Harness::builder()
            .with_size(size)
            .with_pixels_per_point(ppp)
            .build_ui_state(
                |ui, app: &mut App| {
                    app.ui(ui);
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx().request_discard("test: repeated menu metrics");
                    }
                },
                App::new(None).with_settings(dir.clone()),
            );
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        h.get_by_role_and_label(Role::Button, "File").click();
        h.run();
        for label in ["New", "Open", "Import to canvas"] {
            let rect = h
                .get_by_role_and_label(Role::Button, "File")
                .get_by_label_contains(label)
                .rect();
            assert_eq!(rect.height(), 40.0);
            assert!(
                has_full_icon(&h, rect),
                "{label} needs a full 22-point icon"
            );
        }
        h.get_by_label("Exit").scroll_to_me();
        h.run();
        let exit = h.get_by_label("Exit").rect();
        assert!(exit.min.y >= 0.0 && exit.max.y <= size.y);
        h.key_press(Key::Escape);
        h.run();
        h.get_by_role_and_label(Role::Button, "Edit").click();
        h.run();
        for label in ["Cut", "Copy", "Paste"] {
            let row = if label == "Copy" {
                h.query_all_by_label_contains(label)
                    .find(|node| {
                        node.accesskit_node().role() == Role::Button
                            && !node.accesskit_node().is_disabled()
                    })
                    .expect("enabled Copy row with its shortcut")
                    .rect()
            } else {
                h.get_by_role_and_label(Role::Button, label).rect()
            };
            assert_eq!(row.height(), 40.0);
            assert!(has_full_icon(&h, row));
        }
        h.get_by_role_and_label(Role::Button, "Edit")
            .get_by_label("Clear Canvas")
            .scroll_to_me();
        h.run();
        let clear = h
            .get_by_role_and_label(Role::Button, "Edit")
            .get_by_label("Clear Canvas")
            .rect();
        assert!(clear.min.y >= 0.0 && clear.max.y <= size.y);
        h.key_press(Key::Escape);
        h.run();
        h.get_by_role_and_label(Role::Button, "View").click();
        h.run();
        let zoom = h
            .get_by_role_and_label(Role::Button, "View")
            .get_by_label("Zoom")
            .rect();
        assert_eq!(zoom.height(), 40.0);
        assert!(has_full_icon(&h, zoom));
        h.get_by_role_and_label(Role::Button, "View")
            .get_by_label("Zoom")
            .hover();
        h.run();
        let actual = h
            .get_by_role_and_label(Role::Button, "View")
            .get_by_label_contains("Actual size")
            .rect();
        assert_eq!(actual.height(), 40.0);
        assert!(
            actual.min.y >= 0.0
                && actual.max.y <= size.y
                && actual.min.x >= 0.0
                && actual.max.x <= size.x
        );
        assert!(has_full_icon(&h, actual));
        h.key_press(Key::Escape);
        h.run();
        h.get_by_role_and_label(Role::Button, "View")
            .get_by_label("Zoom")
            .focus();
        h.run();
        h.key_press(Key::ArrowRight);
        h.run();
        assert!(
            h.get_by_role_and_label(Role::Button, "View")
                .query_by_label_contains("Actual size")
                .is_some(),
            "Right enters the focused submenu"
        );
        h.key_press(Key::ArrowLeft);
        h.run();
        assert!(
            h.get_by_role_and_label(Role::Button, "View")
                .query_by_label_contains("Actual size")
                .is_none(),
            "Left returns to the parent menu"
        );
        h.key_press(Key::Escape);
        h.run();
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
