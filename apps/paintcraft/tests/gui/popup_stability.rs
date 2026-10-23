use super::*;
use eframe::egui::{Role, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

const MENUS: [(&str, &str, &str); 6] = [
    ("File", "New", "Exit"),
    ("Edit", "Cut", "Clear Canvas"),
    ("View", "Rulers", "Change Classic skin…"),
    ("Selection options", "Rectangular selection", "Deselect"),
    ("Rotate", "Rotate right 90°", "Rotate custom"),
    ("Flip", "Flip vertical", "Flip horizontal"),
];

fn visible(h: &Harness<App>, menu: &str, label: &str) -> bool {
    let rect = h
        .get_by_role_and_label(Role::Button, menu)
        .get_by_label_contains(label)
        .rect();
    let painted = if label == "Rectangular selection" {
        "Rectangle"
    } else {
        label
    };
    h.output().shapes.iter().any(|shape| {
        matches!(&shape.shape, Shape::Text(text) if text.galley.job.text == painted)
            && shape.clip_rect.contains_rect(rect.shrink(0.5))
    })
}

#[test]
fn classic_menus_keep_stable_bounds_and_fully_visible_scrolled_rows() {
    for (size, ppp, appearance) in [
        (Vec2::new(480.0, 360.0), 1.0, Appearance::Light),
        (Vec2::new(480.0, 260.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("popup-stability");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.set_size(size);
        h.run();
        let document = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        for (menu, first, last) in MENUS {
            h.get_by_role_and_label(Role::Button, menu).scroll_to_me();
            h.run();
            h.get_by_role_and_label(Role::Button, menu).click();
            h.run();
            for label in [first, last, first] {
                h.get_by_role_and_label(Role::Button, menu)
                    .get_by_label_contains(label)
                    .scroll_to_me();
                h.run();
                let rect = h
                    .get_by_role_and_label(Role::Button, menu)
                    .get_by_label_contains(label)
                    .rect();
                assert!(
                    visible(&h, menu, label),
                    "{menu}/{label} clipped at {size:?}: {rect:?}"
                );
                h.run_steps(12);
                assert_eq!(
                    h.get_by_role_and_label(Role::Button, menu)
                        .get_by_label_contains(label)
                        .rect(),
                    rect,
                    "{menu}/{label} moved at {size:?}"
                );
                assert!(visible(&h, menu, label));
            }
            h.key_press(Key::Escape);
            h.run();
        }
        assert!(h.state().editor().document().same_semantics(&document));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit GPU Classic compact scrolling popup review"]
fn export_compact_popup_bounds() {
    let output = std::env::temp_dir().join("paintcraft-compact-popup-bounds");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        let dir = scratch("compact-popup-review");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(480.0, 260.0));
        h.run();
        let document = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for (menu, first, last) in MENUS {
            h.get_by_role_and_label(Role::Button, menu).scroll_to_me();
            h.run();
            h.get_by_role_and_label(Role::Button, menu).click();
            h.run();
            for (position, label) in [("top", first), ("bottom", last)] {
                h.get_by_role_and_label(Role::Button, menu)
                    .get_by_label_contains(label)
                    .scroll_to_me();
                h.run();
                h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
                h.run_steps(12);
                assert!(visible(&h, menu, label));
                h.render()
                    .unwrap()
                    .save(output.join(format!(
                        "{theme}-{}-{position}.png",
                        menu.replace(' ', "-").to_lowercase()
                    )))
                    .unwrap();
            }
            h.key_press(Key::Escape);
            h.run();
        }
        assert!(h.state().editor().document().same_semantics(&document));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
