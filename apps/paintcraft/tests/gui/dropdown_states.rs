use super::*;
use eframe::egui::{Rect, Role, epaint::Shape};
use paintcraft::settings::{Appearance, Settings};

fn has_surface(h: &Harness<App>, bounds: Rect, outline: bool) -> bool {
    h.output().shapes.iter().any(|shape| {
        let Shape::Rect(rect) = &shape.shape else {
            return false;
        };
        rect.brush.is_none()
            && rect.rect.center().distance(bounds.center()) < 1.0
            && rect.rect.width() >= bounds.width() - 3.0
            && rect.rect.height() >= bounds.height() - 3.0
            && if outline {
                rect.stroke.width > 0.0 && rect.stroke.color.a() > 0
            } else {
                rect.fill.a() > 0 || (rect.stroke.width > 0.0 && rect.stroke.color.a() > 0)
            }
    })
}

#[test]
fn classic_dropdowns_highlight_hover_and_keep_an_outline_while_open() {
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("dropdown-states");
        Settings {
            appearance,
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.get_by_role_and_label(Role::Button, "Rectangle").click();
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for (trigger, entry) in [
            ("Selection options", "Rectangular selection"),
            ("Brushes", "Round brush"),
            ("Shape outline", "Solid outline"),
            ("Shape fill", "No fill"),
            ("Rotate", "Rotate right 90°"),
            ("Flip", "Flip vertical"),
        ] {
            h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
            h.run();
            let rect = h.get_by_role_and_label(Role::Button, trigger).rect();
            assert!(!has_surface(&h, rect, false), "{trigger} idle surface");
            h.get_by_role_and_label(Role::Button, trigger).hover();
            h.run();
            assert!(has_surface(&h, rect, false), "{trigger} hover surface");
            h.get_by_role_and_label(Role::Button, trigger).click();
            h.run();
            h.get_by_label(entry).hover();
            h.run();
            assert!(has_surface(&h, rect, true), "{trigger} open outline");
            assert_eq!(
                h.get_by_role_and_label(Role::Button, trigger)
                    .accesskit_node()
                    .toggled(),
                Some(eframe::egui::accesskit::Toggled::True)
            );
            h.key_press(Key::Escape);
            h.run();
            assert!(h.query_by_label(entry).is_none(), "{trigger} must close");
            assert_eq!(
                h.get_by_role_and_label(Role::Button, trigger)
                    .accesskit_node()
                    .toggled(),
                Some(eframe::egui::accesskit::Toggled::False)
            );
        }
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn shape_paint_choices_have_large_rows_and_selected_outlines() {
    let mut h = harness_with(App::new(None), 2.0, true);
    h.get_by_role_and_label(Role::Button, "Rectangle").click();
    h.run();
    for (trigger, labels, selected) in [
        (
            "Shape outline",
            ["No outline", "Solid outline"],
            "Solid outline",
        ),
        ("Shape fill", ["No fill", "Solid fill"], "No fill"),
    ] {
        h.get_by_role_and_label(Role::Button, trigger).click();
        h.run();
        for label in labels {
            let node = h.get_by_role_and_label(Role::Button, label);
            assert!(node.rect().width() >= 220.0 && node.rect().height() >= 40.0);
            assert_eq!(
                node.accesskit_node().toggled(),
                Some(if label == selected {
                    eframe::egui::accesskit::Toggled::True
                } else {
                    eframe::egui::accesskit::Toggled::False
                })
            );
        }
        assert!(has_surface(&h, h.get_by_label(selected).rect(), true));
        h.key_press(Key::Escape);
        h.run();
    }
}
