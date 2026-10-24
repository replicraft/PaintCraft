use super::*;
use eframe::egui::{Role, accesskit::Toggled};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

fn choose(h: &mut Harness<App>, size: u32) {
    h.get_by_role_and_label(Role::Button, "Stroke size")
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(Role::Button, "Stroke size").click();
    h.run();
    h.get_by_role_and_label(Role::Button, &format!("{size} px"))
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(Role::Button, &format!("{size} px"))
        .click();
    h.run();
}

#[test]
fn classic_stroke_size_popup_uses_existing_options_and_keeps_custom_sizes_truthful() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(480.0, 360.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("stroke-size-popup");
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
        h.ctx.style_mut_of(h.ctx.theme(), |style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        let document = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        h.get_by_label("Stroke size").scroll_to_me();
        h.run();
        assert!(
            h.get_by_role_and_label(Role::Button, "Stroke size")
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_role_and_label(Role::Button, "Stroke size").hover();
        h.run();
        assert_eq!(
            h.query_all_by_role_and_label(
                Role::Label,
                "Select a line, rectangle or ellipse to change stroke size."
            )
            .count(),
            1
        );
        h.get_by_role_and_label(Role::Button, "Shape outline")
            .scroll_to_me();
        h.run();
        h.get_by_label("Rectangle").click();
        h.run();
        choose(&mut h, 3);
        assert_eq!(h.state().editor().options.size, 3);
        // The floating tool panel needs canvas height. Enter a custom width in
        // a tall window, then retain it when returning to the compact layout.
        h.set_size(Vec2::new(size.x, 900.0));
        h.run();
        assert_eq!(
            h.get_by_role_and_label(Role::SpinButton, "Size")
                .accesskit_node()
                .numeric_value(),
            Some(3.0)
        );
        h.get_by_role_and_label(Role::SpinButton, "Size")
            .set_value_accesskit(37.0);
        h.run();
        h.set_size(size);
        h.run();
        h.get_by_label("Stroke size").scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::Button, "Stroke size").hover();
        h.run();
        assert_eq!(
            h.query_all_by_role_and_label(Role::Label, "Stroke size")
                .count(),
            1
        );
        h.get_by_role_and_label(Role::Button, "Stroke size").click();
        h.run();
        assert!(h.query_by_label("Current: 37 px").is_some());
        for preset in [1, 3, 5, 8] {
            assert_eq!(
                h.get_by_role_and_label(Role::Button, &format!("{preset} px"))
                    .accesskit_node()
                    .toggled(),
                Some(Toggled::False)
            );
            let row = h
                .get_by_role_and_label(Role::Button, &format!("{preset} px"))
                .rect();
            assert_eq!(row.height(), 40.0);
            assert!(h.output().shapes.iter().any(|shape|matches!(&shape.shape,eframe::egui::epaint::Shape::Rect(sample)
                if sample.brush.is_none() && row.contains_rect(sample.rect) && sample.rect.size()==Vec2::new(112.0,preset as f32))));
        }
        h.get_by_role_and_label(Role::Button, "5 px").scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::Button, "5 px").focus();
        h.run();
        h.key_press(Key::Enter);
        h.run();
        assert_eq!(h.state().editor().options.size, 5);
        assert!(h.query_by_role_and_label(Role::Button, "5 px").is_none());
        h.get_by_role_and_label(Role::Button, "Stroke size").click();
        h.run();
        h.get_by_role_and_label(Role::Button, "5 px").scroll_to_me();
        h.run();
        assert_eq!(
            h.get_by_role_and_label(Role::Button, "5 px")
                .accesskit_node()
                .toggled(),
            Some(Toggled::True)
        );
        h.key_press(Key::Escape);
        h.run();
        assert_eq!(h.state().editor().options.size, 5);
        h.get_by_label("Settings…").click();
        h.run();
        assert!(
            h.get_by_role_and_label(Role::Button, "Stroke size")
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label("Cancel").click();
        h.run();
        for label in ["Line", "Ellipse"] {
            h.get_by_role_and_label(Role::Button, "Shape outline")
                .scroll_to_me();
            h.run();
            h.get_by_role_and_label(Role::Button, label).click();
            h.run();
            assert!(
                !h.get_by_role_and_label(Role::Button, "Stroke size")
                    .accesskit_node()
                    .is_disabled()
            );
            choose(&mut h, 3);
            assert_eq!(h.state().editor().options.size, 3);
        }
        h.get_by_role_and_label(Role::Button, "Brush")
            .scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::Button, "Brush").click();
        h.run();
        assert!(
            h.get_by_role_and_label(Role::Button, "Stroke size")
                .accesskit_node()
                .is_disabled()
        );
        assert!(h.state().editor().document().same_semantics(&document));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit GPU Classic stroke size dropdown review"]
fn export_classic_stroke_size() {
    let output = std::env::temp_dir().join("paintcraft-stroke-size");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        for (size, layout) in [
            (Vec2::new(1360.0, 900.0), "desktop"),
            (Vec2::new(1200.0, 900.0), "medium"),
            (Vec2::new(480.0, 360.0), "compact"),
        ] {
            let dir = scratch("stroke-size-review");
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
            let document = h.state().editor().document_snapshot();
            let ticket = h.state().editor().history_view().ticket;
            let capture = |h: &mut Harness<App>, state: &str| {
                h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
                h.run();
                h.render()
                    .unwrap()
                    .save(output.join(format!("{theme}-{layout}-{state}.png")))
                    .unwrap();
            };
            h.get_by_role_and_label(Role::Button, "Stroke size")
                .scroll_to_me();
            h.run();
            capture(&mut h, "disabled");
            h.get_by_role_and_label(Role::Button, "Shape outline")
                .scroll_to_me();
            h.run();
            h.get_by_role_and_label(Role::Button, "Rectangle").click();
            h.run();
            h.get_by_role_and_label(Role::Button, "Stroke size")
                .scroll_to_me();
            h.run();
            h.get_by_role_and_label(Role::Button, "Stroke size").click();
            h.run();
            capture(&mut h, "custom-current");
            h.get_by_role_and_label(Role::Button, "5 px").click();
            h.run();
            h.get_by_role_and_label(Role::Button, "Stroke size").click();
            h.run();
            assert_eq!(
                h.get_by_role_and_label(Role::Button, "5 px")
                    .accesskit_node()
                    .toggled(),
                Some(Toggled::True)
            );
            capture(&mut h, "selected");
            h.key_press(Key::Escape);
            h.run();
            assert_eq!(h.state().editor().options.size, 5);
            assert!(h.state().editor().document().same_semantics(&document));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn stroke_size_choice_reaches_existing_line_rendering_and_undo_redo() {
    let mut h = harness_with(App::new(None), 1.0, true);
    h.get_by_label("Line").click();
    h.run();
    let document = h.state().editor().document_snapshot();
    let depth = h.state().editor().undo_depth();
    choose(&mut h, 1);
    assert_eq!(h.state().editor().undo_depth(), depth);
    drag(&mut h, (80.0, 80.0), (140.0, 80.0));
    let thickness = |h: &Harness<App>| {
        (65..96)
            .filter(|y| top_pixel(h, 110, *y) != Rgba([255, 255, 255, 255]))
            .count()
    };
    let thin = thickness(&h);
    assert_eq!(thin, 1);
    assert_eq!(h.state().editor().undo_depth(), depth + 1);
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&document));
    choose(&mut h, 8);
    assert_eq!(h.state().editor().undo_depth(), depth);
    drag(&mut h, (80.0, 80.0), (140.0, 80.0));
    let thick = thickness(&h);
    assert!(thick >= 5 && thick > thin);
    let rendered = h.state().editor().document_snapshot();
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&document));
    h.get_by_label("Redo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&rendered));
}
