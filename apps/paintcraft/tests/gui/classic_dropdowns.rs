use super::*;

#[test]
fn brush_and_selection_dropdowns_close_after_selection_without_editing_document() {
    let mut h = harness_with(App::new(None), 1.0, true);
    let base = h.state().editor().document_snapshot();
    h.get_by_role_and_label(eframe::egui::Role::Button, "Brushes")
        .click();
    h.run();
    h.get_by_label("Square brush").click();
    h.run();
    assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Brush);
    assert!(!h.state().editor().options.round);
    assert!(h.query_by_label("Square brush").is_none());
    h.get_by_role_and_label(eframe::egui::Role::Button, "Selection options")
        .click();
    h.run();
    h.get_by_label("Rectangular selection").click();
    h.run();
    assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Select);
    assert!(h.query_by_label("Rectangular selection").is_none());
    assert!(h.state().editor().document().same_semantics(&base));
}

#[test]
fn shape_outline_and_fill_dropdowns_change_real_tool_options() {
    let mut h = harness();
    h.get_by_label("Rectangle").click();
    h.run();
    h.get_by_label("Shape fill").click();
    h.run();
    h.get_by_label("Solid fill").click();
    h.run();
    assert_eq!(
        h.state().editor().options.style,
        paintcraft_tools::ShapeStyle::Both
    );
    h.get_by_label("Shape outline").click();
    h.run();
    h.get_by_label("No outline").click();
    h.run();
    assert_eq!(
        h.state().editor().options.style,
        paintcraft_tools::ShapeStyle::Filled
    );
    drag(&mut h, (50.0, 50.0), (70.0, 70.0));
    assert_eq!(top_pixel(&h, 60, 60), Rgba([0, 0, 0, 255]));
}

#[test]
fn classic_menus_switch_on_hover_and_zoom_submenu_closes_on_escape() {
    let mut h = harness();
    let base = h.state().editor().document_snapshot();
    h.get_by_role_and_label(eframe::egui::Role::Button, "File")
        .click();
    h.run();
    assert!(h.query_by_label_contains("Save as…").is_some());
    h.get_by_role_and_label(eframe::egui::Role::Button, "View")
        .hover();
    h.run();
    assert!(h.query_by_label_contains("Save as…").is_none());
    h.get_by_role_and_label(eframe::egui::Role::Button, "View")
        .get_by_label("Zoom")
        .hover();
    h.run();
    assert!(
        h.get_by_role_and_label(eframe::egui::Role::Button, "View")
            .query_by_label_contains("Zoom in")
            .is_some()
    );
    h.key_press(Key::Escape);
    h.run();
    assert!(
        h.get_by_role_and_label(eframe::egui::Role::Button, "View")
            .query_by_label_contains("Zoom in")
            .is_none()
    );
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state().editor().document().same_semantics(&base));
}

#[test]
#[ignore = "requires a native GPU; exports dropdown and cursor review images"]
fn render_classic_dropdowns_and_cursor_for_review() {
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    let output = PathBuf::from("/tmp/paintcraft-classic-dropdowns");
    std::fs::create_dir_all(&output).unwrap();
    for (name, appearance) in [("light", Appearance::Light), ("dark", Appearance::Dark)] {
        let dir = scratch(&format!("dropdowns-{name}"));
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.get_by_role_and_label(eframe::egui::Role::Button, "Brushes")
            .click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{name}-brushes.png")))
            .unwrap();
        h.key_press(Key::Escape);
        h.run();
        h.get_by_label("Rectangle").click();
        h.run();
        h.get_by_label("Shape fill").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{name}-shape-fill.png")))
            .unwrap();
        h.key_press(Key::Escape);
        h.run();
        h.get_by_label("Eraser").click();
        h.run();
        h.get_by_label("Tool size").set_value_accesskit(32.0);
        h.run();
        // egui_kittest's synthetic arrow ignores platform CursorIcon::None.
        h.set_render_cursor(false);
        h.event(Event::PointerMoved(at(&h, 100.0, 100.0)));
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{name}-eraser.png")))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
