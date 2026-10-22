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
