use super::*;

#[test]
fn tool_hover_shows_one_tooltip() {
    let mut h = harness();
    h.ctx.style_mut_of(h.ctx.theme(), |s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h.get_by_role_and_label(eframe::egui::Role::Button, "Pencil")
        .hover();
    h.run();
    assert_eq!(
        h.query_all_by_role_and_label(eframe::egui::Role::Label, "Pencil")
            .count(),
        1
    );
}

#[test]
fn classic_color_wells_use_reference_proportions_and_show_one_named_color_tooltip() {
    use eframe::egui::Role;
    for ppp in [1.0, 2.0] {
        let mut h = harness_with(App::new(None), ppp, true);
        h.ctx.style_mut_of(h.ctx.theme(), |style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let primary = h
            .get_by_role_and_label(Role::ColorWell, "Pencil color")
            .rect();
        let secondary = h
            .get_by_role_and_label(Role::ColorWell, "Fill color")
            .rect();
        assert_eq!(primary.size(), Vec2::splat(40.0));
        assert_eq!(secondary.size(), Vec2::splat(32.0));
        assert_eq!(primary.center().x, secondary.center().x);
        assert!(secondary.top() > primary.bottom());
        for (label, tip) in [
            ("Pencil color", "Pencil color\nBlack\n#000000"),
            ("Fill color", "Fill color\nWhite\n#FFFFFF"),
        ] {
            h.get_by_role_and_label(Role::ColorWell, label).hover();
            h.run();
            assert_eq!(h.query_all_by_role_and_label(Role::Label, tip).count(), 1);
        }
        h.get_by_label("Color #ED1C24").click();
        h.run();
        h.get_by_label("Color #B5E61D").click_secondary();
        h.run();
        assert_eq!(h.state().editor().color, Rgba([237, 28, 36, 255]));
        assert_eq!(h.state().editor().secondary, Rgba([181, 230, 29, 255]));
        for (label, tip) in [
            ("Pencil color", "Pencil color\nRed\n#ED1C24"),
            ("Fill color", "Fill color\nLime\n#B5E61D"),
        ] {
            h.get_by_role_and_label(Role::ColorWell, label).hover();
            h.run();
            assert_eq!(h.query_all_by_role_and_label(Role::Label, tip).count(), 1);
        }
        h.get_by_role_and_label(Role::ColorWell, "Fill color")
            .click();
        h.run();
        h.get_by_label("Hex RGB").focus();
        h.run();
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
        h.run();
        h.get_by_label("Hex RGB").type_text("#123456");
        h.run();
        h.get_by_label("OK").click();
        h.run();
        h.get_by_role_and_label(Role::ColorWell, "Fill color")
            .hover();
        h.run();
        assert_eq!(
            h.query_all_by_role_and_label(Role::Label, "Fill color\nCustom color\n#123456")
                .count(),
            1
        );
        assert_eq!(h.state().editor().color, Rgba([237, 28, 36, 255]));
        assert_eq!(h.state().editor().secondary, Rgba([18, 52, 86, 255]));
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
    }
}

#[test]
fn classic_ribbon_dropdowns_are_compact_and_show_one_tooltip_at_each_display_scale() {
    use eframe::egui::Role;
    for ppp in [1.0, 2.0] {
        let mut h = harness_with(App::new(None), ppp, true);
        h.ctx.style_mut_of(h.ctx.theme(), |style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        let before = h.state().editor().document_snapshot();
        for (label, tooltip) in [
            ("Crop", "Select an area of the canvas to crop"),
            (
                "Remove background",
                "Background removal is not available yet.",
            ),
        ] {
            let button = h.get_by_role_and_label(Role::Button, label);
            assert!(button.accesskit_node().is_disabled());
            button.hover();
            h.run();
            assert_eq!(
                h.query_all_by_role_and_label(Role::Label, tooltip).count(),
                1,
                "{label} needs one availability tooltip"
            );
        }
        let picker = h.get_by_role_and_label(Role::Button, "Color picker");
        assert!(!picker.accesskit_node().is_disabled());
        picker.hover();
        h.run();
        assert_eq!(
            h.query_all_by_role_and_label(Role::Label, "Color picker")
                .count(),
            1
        );
        for label in ["Selection options", "Brushes"] {
            let button = h.get_by_role_and_label(Role::Button, label);
            assert!(
                button.rect().height() <= 24.0,
                "{label}: {:?}",
                button.rect()
            );
            // Brushes also has a persistent group caption; count only the
            // additional label introduced by hovering this control.
            let baseline = h.query_all_by_role_and_label(Role::Label, label).count();
            button.hover();
            h.run();
            assert_eq!(
                h.query_all_by_role_and_label(Role::Label, label).count(),
                baseline + 1
            );
        }
        h.get_by_role_and_label(Role::Button, "Rectangle").click();
        h.run();
        for label in ["Shape outline", "Shape fill"] {
            let button = h.get_by_role_and_label(Role::Button, label);
            assert!(
                button.rect().width() <= 64.0,
                "{label}: {:?}",
                button.rect()
            );
            assert!(!button.accesskit_node().is_disabled());
            button.hover();
            h.run();
            assert_eq!(h.query_all_by_role_and_label(Role::Label, label).count(), 1);
        }
        assert!(h.state().editor().document().same_semantics(&before));
    }
}
