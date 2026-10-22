use super::*;
use eframe::egui::Role;

#[test]
fn classic_layers_scroll_without_moving_properties_and_close_without_history() {
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(1100.0, 500.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("classic-layer-panel");
        Settings {
            classic_variant: Some(ClassicVariant::V11),
            appearance,
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
                        ui.ctx()
                            .request_discard("test: repeated layer-panel layout");
                    }
                },
                App::new(None).with_settings(dir.clone()),
            );
        h.run();
        with_layer(&mut h);
        for _ in 0..16 {
            h.get_by_label("Add layer").click();
            h.run();
        }
        assert_eq!(h.state().editor().document().roots.len(), 18);
        let selected = h.state().editor().target();
        let bottom = h.state().editor().document().roots[0];
        let before = publications(&h);
        let field = h.get_by_label("Layer name").rect();
        assert!(field.max.y <= size.y && field.max.x <= size.x, "{field:?}");
        h.get_by_role_and_label(Role::CheckBox, "Show Background")
            .scroll_to_me();
        h.run();
        assert_eq!(h.get_by_label("Layer name").rect(), field);
        h.get_by_role_and_label(Role::CheckBox, "Show Background")
            .click();
        h.run();
        assert!(!h.state().editor().document().layers[&bottom].visible);
        assert_eq!(h.state().editor().target(), selected);
        assert_eq!(publications(&h), before + 1);
        h.get_by_role_and_label(Role::Button, "Background").click();
        h.run();
        assert_eq!(h.state().editor().target(), Some(bottom));
        assert_eq!(publications(&h), before + 1, "selection is view state");
        assert!(h.get_by_label("Move down").accesskit_node().is_disabled());
        h.get_by_label("Move up").click();
        h.run();
        assert_eq!(h.state().editor().document().roots[1], bottom);
        assert!(!h.state().editor().document().layers[&bottom].visible);
        assert_eq!(publications(&h), before + 2);
        h.get_by_label("Close Layers").click();
        h.run();
        assert!(h.query_by_label("Add layer").is_none());
        assert_eq!(publications(&h), before + 2);
        h.get_by_role_and_label(Role::Button, "Layers").click();
        h.run();
        assert_eq!(h.state().editor().target(), Some(bottom));
        assert!(h.query_by_label("Add layer").is_some());
        assert_eq!(publications(&h), before + 2);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn selecting_a_different_classic_layer_commits_the_old_name_to_its_owner() {
    let mut h = harness();
    with_layer(&mut h);
    let top = h.state().editor().target().unwrap();
    let bottom = h.state().editor().document().roots[0];
    let before = publications(&h);
    h.get_by_label("Layer name").focus();
    h.run();
    h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
    h.run();
    h.get_by_label("Layer name").type_text("Sky");
    h.run();
    h.get_by_role_and_label(Role::Button, "Background").click();
    h.run();
    assert_eq!(h.state().editor().target(), Some(bottom));
    assert_eq!(&*h.state().editor().document().layers[&top].name, "Sky");
    assert_eq!(
        &*h.state().editor().document().layers[&bottom].name,
        "Background"
    );
    assert_eq!(publications(&h), before + 1);
    h.get_by_label("Undo").click();
    h.run();
    assert_eq!(&*h.state().editor().document().layers[&top].name, "Layer 2");
}

#[test]
fn long_layer_names_keep_classic_properties_fixed_and_have_one_full_name_tooltip() {
    use eframe::egui::epaint::Shape;
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(1100.0, 500.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("classic-layer-long-name");
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
                        ui.ctx().request_discard("test: repeated layer properties");
                    }
                },
                App::new(None).with_settings(dir.clone()),
            );
        h.run();
        with_layer(&mut h);
        h.ctx.style_mut_of(h.ctx.theme(), |style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        let field = h.get_by_label("Layer name").rect();
        let opacity = h.get_by_label("Opacity").rect();
        let name = "Long_layer_name_".repeat(60);
        h.get_by_label("Layer name").focus();
        h.run();
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
        h.run();
        h.get_by_label("Layer name").type_text(&name);
        h.run();
        h.key_press(Key::Enter);
        h.run();
        let layer = h.state().editor().target().unwrap();
        assert_eq!(&*h.state().editor().document().layers[&layer].name, name);
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        assert_eq!(
            h.get_by_label("Layer name").rect(),
            field,
            "long names must not move the editor below the panel"
        );
        assert_eq!(h.get_by_label("Opacity").rect(), opacity);
        assert!(field.max.y <= size.y && field.max.x <= size.x);
        let status_label = format!("Drawing on: {name}");
        let status = h.get_by_role_and_label(Role::Label, &status_label);
        assert!(status.rect().height() <= 24.0);
        let selected = h.get_by_role_and_label(Role::Button, &name);
        let row = selected.rect();
        assert!(row.right() <= size.x);
        assert_eq!(
            selected.accesskit_node().toggled(),
            Some(eframe::egui::accesskit::Toggled::True)
        );
        assert!(
            h.output()
                .shapes
                .iter()
                .any(|shape| matches!(&shape.shape, Shape::Rect(outline)
            if outline.brush.is_none() && outline.rect.center().distance(row.center()) < 1.0
            && outline.rect.width() >= row.width() - 2.0 && outline.stroke.width > 0.0
            && outline.stroke.color == h.ctx.style_of(h.ctx.theme()).visuals.selection.stroke.color))
        );
        h.get_by_role_and_label(Role::Button, &name).hover();
        h.run();
        assert_eq!(h.query_all_by_role_and_label(Role::Label, &name).count(), 1);
        let tooltip = h.get_by_role_and_label(Role::Label, &name).rect();
        assert!(
            tooltip.min.x >= 0.0 && tooltip.max.x <= size.x,
            "full name tooltip must fit horizontally: {tooltip:?}"
        );
        h.get_by_role_and_label(Role::Label, &status_label).hover();
        h.run();
        assert_eq!(
            h.query_all_by_role_and_label(Role::Label, &name).count(),
            1,
            "the truncated status must not add a second automatic tooltip"
        );
        h.run_steps(8);
        assert_eq!(h.get_by_label("Layer name").rect(), field);
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
