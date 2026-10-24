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
        for label in [
            "Close Layers",
            "Add layer",
            "Move up",
            "Move down",
            "Delete layer",
        ] {
            let rect = h.get_by_role_and_label(Role::Button, label).rect();
            assert!(
                h.output().shapes.iter().any(|shape| matches!(&shape.shape,
                    eframe::egui::epaint::Shape::Rect(image)
                        if image.brush.is_some() && image.rect.size() == Vec2::splat(22.0)
                        && image.rect.center() == rect.center()
                        && image.fill.r() == image.fill.g() && image.fill.g() == image.fill.b()
                )),
                "{label} must use centered theme-tinted artwork"
            );
        }
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
        let height = if size.y < 650.0 { 32.0 } else { 40.0 };
        assert_eq!(field.height(), height);
        assert_eq!(opacity.height(), height);
        assert!((field.left() - opacity.left()).abs() < 0.1);
        assert!(
            opacity.bottom() <= size.y - 40.0,
            "properties stay above the status bar: {opacity:?}"
        );
        fn caption_font(shape: &Shape, caption: &str) -> Option<f32> {
            match shape {
                Shape::Text(text) if text.galley.job.text == caption => {
                    Some(text.galley.job.sections[0].format.font_id.size)
                }
                Shape::Vec(children) => children
                    .iter()
                    .find_map(|shape| caption_font(shape, caption)),
                _ => None,
            }
        }
        for caption in ["Layer name", "Opacity"] {
            assert_eq!(
                h.output()
                    .shapes
                    .iter()
                    .find_map(|shape| caption_font(&shape.shape, caption)),
                Some(16.0),
                "{caption} uses the Classic body font"
            );
        }
        let global = h.ctx.style_of(h.ctx.theme());
        assert_ne!(
            global.text_styles[&eframe::egui::TextStyle::Body].size,
            16.0,
            "panel metrics must stay scoped"
        );

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

#[test]
#[ignore = "requires GPU; exports Classic Layers glyphs and panel actions"]
fn export_classic_layer_controls() {
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    let output = std::env::temp_dir().join("paintcraft-layer-controls");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        let dir = scratch("classic-layer-controls-review");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(1360.0, 900.0));
        h.run();
        h.get_by_role_and_label(Role::Button, "Layers").hover();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-ribbon-hover.png")))
            .unwrap();
        h.get_by_role_and_label(Role::Button, "Layers").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-panel.png")))
            .unwrap();
        h.get_by_label("Add layer").click();
        h.run();
        h.get_by_label("Add layer").click();
        h.run();
        h.get_by_label("Move down").hover();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-panel-actions.png")))
            .unwrap();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        h.event(Event::PointerMoved(Pos2::new(4.0, 4.0)));
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-row-idle.png")))
            .unwrap();
        h.get_by_role_and_label(Role::Button, "Layer 3").hover();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-row-hover.png")))
            .unwrap();
        let point = h
            .get_by_role_and_label(Role::Button, "Layer 3")
            .rect()
            .center();
        h.event(press(point));
        h.step();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-row-pressed.png")))
            .unwrap();
        h.event(release(point));
        h.run();
        h.set_size(Vec2::new(1100.0, 500.0));
        h.event(Event::PointerMoved(Pos2::new(4.0, 4.0)));
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-rows-compact.png")))
            .unwrap();
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn classic_layer_rows_use_neutral_selected_surfaces_and_keep_native_input() {
    use super::control_surfaces::selected_surface;
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(1100.0, 500.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("classic-layer-row-surfaces");
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
        with_layer(&mut h);
        let top = h.state().editor().target();
        let bottom = h.state().editor().document().roots[0];
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        h.event(Event::PointerMoved(Pos2::new(4.0, 4.0)));
        h.run();
        let idle = selected_surface(&h, "Layer 2");
        let row = h.get_by_role_and_label(Role::Button, "Layer 2").rect();
        assert_eq!(row.height(), 40.0);
        h.get_by_role_and_label(Role::Button, "Layer 2").hover();
        h.run();
        let hovered = selected_surface(&h, "Layer 2");
        assert_ne!(idle, hovered);
        h.event(press(row.center()));
        h.step();
        assert_ne!(selected_surface(&h, "Layer 2"), hovered);
        h.event(release(row.center()));
        h.run();
        assert_eq!(h.state().editor().target(), top);
        assert_eq!(h.get_by_role_and_label(Role::Button, "Layer 2").rect(), row);
        h.get_by_role_and_label(Role::Button, "Background")
            .scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::Button, "Background").click();
        h.run();
        assert_eq!(h.state().editor().target(), Some(bottom));
        selected_surface(&h, "Background");
        assert_eq!(
            h.get_by_role_and_label(Role::Button, "Layer 2")
                .accesskit_node()
                .toggled(),
            Some(eframe::egui::accesskit::Toggled::False)
        );
        h.get_by_role_and_label(Role::Button, "Layer 2")
            .scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::Button, "Layer 2").focus();
        h.run();
        h.key_press(Key::Space);
        h.run();
        assert_eq!(h.state().editor().target(), top);
        selected_surface(&h, "Layer 2");
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
