use super::*;
use eframe::egui::{Role, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

#[test]
fn classic_split_controls_join_their_targets_and_keep_tool_and_popup_actions_separate() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(480.0, 360.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("classic-split-controls");
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
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for (main, arrow, entry, tool) in [
            (
                "Select",
                "Selection options",
                "Rectangular selection",
                paintcraft_tools::Tool::Select,
            ),
            (
                "Brush",
                "Brushes",
                "Round brush",
                paintcraft_tools::Tool::Brush,
            ),
        ] {
            h.get_by_role_and_label(Role::Button, main).scroll_to_me();
            h.run();
            let top = h.get_by_role_and_label(Role::Button, main).rect();
            let bottom = h.get_by_role_and_label(Role::Button, arrow).rect();
            assert_eq!(top.size(), Vec2::splat(48.0));
            assert_eq!(bottom.size(), Vec2::new(48.0, 20.0));
            assert_eq!(top.bottom(), bottom.top());
            assert_eq!(top.left(), bottom.left());
            h.get_by_role_and_label(Role::Button, main).click();
            h.run();
            assert_eq!(h.state().editor().tool, tool);
            assert!(
                h.query_by_label(entry).is_none(),
                "main button must not open the menu"
            );
            let body = top.union(bottom);
            assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
                Shape::Rect(painted) if painted.rect.center().distance(body.center())<0.1
                    && painted.rect.width()>=body.width()-2.0 && painted.rect.height()>=body.height()-2.0
                    && painted.stroke.width>0.0 && painted.stroke.color.b()>painted.stroke.color.r()
                    && painted.fill.r()==painted.fill.g() && painted.fill.g()==painted.fill.b())),
                "selected split control has one neutral outer surface and accent boundary");
            h.get_by_role_and_label(Role::Button, arrow).click();
            h.run();
            assert!(h.query_by_label(entry).is_some());
            assert_eq!(h.state().editor().tool, tool);
            assert_eq!(
                h.get_by_role_and_label(Role::Button, arrow)
                    .accesskit_node()
                    .toggled(),
                Some(eframe::egui::accesskit::Toggled::True)
            );
            h.key_press(Key::Escape);
            h.run();
            h.get_by_role_and_label(Role::Button, arrow).focus();
            h.run();
            h.key_press(Key::Space);
            h.run();
            assert!(
                h.query_by_label(entry).is_some(),
                "keyboard opens the arrow's popup"
            );
            h.key_press(Key::Escape);
            h.run();
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
        }
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit GPU Classic joined tool/menu control review"]
fn export_classic_split_controls() {
    let output = std::env::temp_dir().join("paintcraft-split-controls");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        for (size, layout) in [
            (Vec2::new(1360.0, 900.0), "desktop"),
            (Vec2::new(480.0, 360.0), "compact"),
        ] {
            let dir = scratch("split-control-review");
            Settings {
                appearance,
                classic_variant: Some(ClassicVariant::V11),
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
            h.set_size(size);
            h.run();
            let snapshot = h.state().editor().document_snapshot();
            let ticket = h.state().editor().history_view().ticket;
            for (tool, arrow, name) in [
                ("Select", "Selection options", "selection"),
                ("Brush", "Brushes", "brush"),
            ] {
                h.get_by_role_and_label(Role::Button, tool).scroll_to_me();
                h.run();
                h.get_by_role_and_label(Role::Button, tool).click();
                h.run();
                h.event(Event::PointerMoved(Pos2::new(4.0, 4.0)));
                h.run();
                h.render()
                    .unwrap()
                    .save(output.join(format!("{theme}-{layout}-{name}-selected.png")))
                    .unwrap();
                h.get_by_role_and_label(Role::Button, arrow).click();
                h.run();
                h.render()
                    .unwrap()
                    .save(output.join(format!("{theme}-{layout}-{name}-open.png")))
                    .unwrap();
                h.key_press(Key::Escape);
                h.run();
                assert!(h.state().editor().document().same_semantics(&snapshot));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
            }
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
