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
            assert_eq!(rect.height(), 38.0);
            assert!(
                has_full_icon(&h, rect),
                "{label} needs a full 22-point icon"
            );
            let icon = h
                .output()
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    Shape::Rect(image)
                        if image.brush.is_some()
                            && image.rect.size() == Vec2::splat(22.0)
                            && rect.contains_rect(image.rect) =>
                    {
                        Some(image.rect)
                    }
                    _ => None,
                })
                .min_by(|a, b| a.left().total_cmp(&b.left()))
                .unwrap();
            let text = h
                .output()
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    Shape::Text(text) if text.galley.text() == label && rect.contains(text.pos) => {
                        Some(text.pos)
                    }
                    _ => None,
                })
                .unwrap();
            assert!(
                text.x - icon.right() >= 13.5,
                "{label}: gap {} at {ppp}ppp row{rect:?} icon{icon:?} text{text:?}",
                text.x - icon.right()
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
        for (label, key) in [("Cut", Key::X), ("Copy", Key::C), ("Paste", Key::V)] {
            let shortcut = h.ctx.format_shortcut(&eframe::egui::KeyboardShortcut::new(
                eframe::egui::Modifiers::COMMAND,
                key,
            ));
            let label = format!("{label} {shortcut}");
            let row = h
                .get_by_role_and_label(Role::Button, "Edit")
                .get_by_label(&label)
                .rect();
            assert_eq!(row.height(), 38.0);
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
        assert_eq!(zoom.height(), 38.0);
        assert!(has_full_icon(&h, zoom));
        h.get_by_role_and_label(Role::Button, "View")
            .get_by_label("Zoom")
            .hover();
        h.run();
        let actual = h
            .get_by_role_and_label(Role::Button, "View")
            .get_by_label_contains("Actual size")
            .rect();
        assert_eq!(actual.height(), 38.0);
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

#[test]
#[ignore = "requires GPU; exports headless Classic layout references"]
fn export_compact_classic_layout_previews() {
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/validation/classic-layout-about-2026-10-24");
    std::fs::create_dir_all(&output).unwrap();
    for (name, appearance) in [("light", Appearance::Light), ("dark", Appearance::Dark)] {
        let dir = scratch("classic-layout-preview");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(1600.0, 1000.0));
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("after-{name}.png")))
            .unwrap();
        h.get_by_role_and_label(Role::Button, "File").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("after-{name}-file.png")))
            .unwrap();
        h.key_press(Key::Escape);
        h.run();
        h.get_by_label("Settings…").click();
        h.run();
        h.get_by_role_and_label(Role::Button, "About").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("after-{name}-about.png")))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
