use super::*;
use eframe::egui::{Role, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

#[test]
fn classic_main_menu_rows_have_contiguous_hit_targets_and_readable_text() {
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("classic-menu-density");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for (menu, labels) in [
            ("File", ["New", "Open"]),
            ("Edit", ["Cut", "Copy"]),
            ("View", ["Rulers", "Gridlines"]),
        ] {
            h.get_by_role_and_label(Role::Button, menu).click();
            h.run();
            let first = h
                .get_by_role_and_label(Role::Button, menu)
                .get_by_label_contains(labels[0])
                .rect();
            let second = if menu == "Edit" {
                h.get_by_role_and_label(Role::Button, menu)
                    .query_all_by_label_contains(labels[1])
                    .find(|node| {
                        node.accesskit_node().role() == Role::Button
                            && !node.accesskit_node().is_disabled()
                    })
                    .expect("enabled Copy row with its shortcut")
                    .rect()
            } else {
                h.get_by_role_and_label(Role::Button, menu)
                    .get_by_label_contains(labels[1])
                    .rect()
            };
            assert_eq!(first.height(), 40.0);
            assert_eq!(second.height(), 40.0);
            assert_eq!(
                second.top(),
                first.bottom(),
                "{menu} must not leave dead space between ordinary rows"
            );
            let text = h
                .output()
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    Shape::Text(text) if text.galley.job.text.trim() == labels[0] => Some(text),
                    _ => None,
                })
                .expect("menu row text is painted");
            assert_eq!(text.galley.job.sections[0].format.font_id.size, 16.0);
            if menu == "File" {
                let shortcut = h.ctx.format_shortcut(&eframe::egui::KeyboardShortcut::new(
                    Modifiers::COMMAND,
                    Key::N,
                ));
                let text = h
                    .output()
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        Shape::Text(text) if text.galley.job.text == shortcut => Some(text),
                        _ => None,
                    })
                    .expect("New shortcut is painted");
                let ink = text
                    .override_text_color
                    .unwrap_or(text.galley.job.sections[0].format.color);
                assert!(
                    if appearance == Appearance::Dark {
                        ink.r() >= 180
                    } else {
                        ink.r() <= 110
                    },
                    "shortcut text must remain legible in either theme"
                );
            }
            if menu == "View" {
                // This point lay in the former inter-row gap. It must now
                // activate Gridlines through the real pointer/action path.
                let point = Pos2::new(second.center().x, first.bottom() + 2.0);
                h.input_mut().events.extend([
                    Event::PointerMoved(point),
                    press(point),
                    release(point),
                ]);
                h.run();
                assert!(!eframe::egui::Popup::is_any_open(&h.ctx));
                h.get_by_role_and_label(Role::Button, "View").click();
                h.run();
                assert_eq!(
                    h.get_by_label_contains("Gridlines")
                        .accesskit_node()
                        .toggled(),
                    Some(eframe::egui::accesskit::Toggled::True)
                );
            }
            h.key_press(Key::Escape);
            h.run();
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
        }
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
