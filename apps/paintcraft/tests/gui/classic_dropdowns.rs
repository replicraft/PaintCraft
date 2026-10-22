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
    assert!(h.query_by_label_contains("Save as project").is_some());
    h.get_by_role_and_label(eframe::egui::Role::Button, "View")
        .hover();
    h.run();
    assert!(h.query_by_label_contains("Save as project").is_none());
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
        for (menu, filename) in [
            ("Rotate", "rotate"),
            ("Flip", "flip"),
            ("Selection options", "selection"),
        ] {
            h.get_by_role_and_label(eframe::egui::Role::Button, menu)
                .click();
            h.run();
            h.render()
                .unwrap()
                .save(output.join(format!("{name}-{filename}.png")))
                .unwrap();
            h.key_press(Key::Escape);
            h.run();
        }
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

#[test]
fn media_gallery_preserves_unavailable_presets_and_has_reachable_working_tips() {
    let mut h = harness();
    let base = h.state().editor().document_snapshot();
    h.get_by_role_and_label(eframe::egui::Role::Button, "Brushes")
        .click();
    h.run();
    for label in [
        "Calligraphy brush",
        "Calligraphy pen",
        "Airbrush",
        "Oil brush",
        "Crayon",
        "Marker",
        "Natural pencil",
        "Watercolor brush",
    ] {
        assert!(h.get_by_label(label).accesskit_node().is_disabled());
    }
    h.get_by_label("Calligraphy brush").click();
    h.run();
    assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Pencil);
    assert!(h.query_by_label("Watercolor brush").is_some());
    h.get_by_label("Square brush").scroll_to_me();
    h.run();
    h.get_by_label("Square brush").click();
    h.run();
    assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Brush);
    assert!(!h.state().editor().options.round);
    h.get_by_role_and_label(eframe::egui::Role::Button, "Brushes")
        .click();
    h.run();
    h.get_by_label("Round brush").scroll_to_me();
    h.run();
    h.get_by_label("Round brush").click();
    h.run();
    assert!(h.state().editor().options.round);
    assert!(h.state().editor().document().same_semantics(&base));
}

#[test]
fn image_menus_show_reference_entries_without_executing_unavailable_operations() {
    let mut h = harness_with(App::new(None), 1.0, true);
    let base = h.state().editor().document_snapshot();
    for (menu, entries) in [
        (
            "Rotate",
            vec![
                "Rotate right 90°",
                "Rotate left 90°",
                "Rotate 180°",
                "Rotate custom",
            ],
        ),
        ("Flip", vec!["Flip vertical", "Flip horizontal"]),
    ] {
        h.get_by_role_and_label(eframe::egui::Role::Button, menu)
            .click();
        h.run();
        for entry in &entries {
            assert!(h.get_by_label(entry).accesskit_node().is_disabled());
        }
        h.get_by_label(entries[0]).click();
        h.run();
        assert!(h.query_by_label(entries[0]).is_some());
        assert!(h.state().editor().document().same_semantics(&base));
        h.key_press(Key::Escape);
        h.run();
        assert!(h.query_by_label(entries[0]).is_none());
    }
}

#[test]
fn image_transform_popups_keep_rows_icons_and_single_tooltips_reachable_on_short_displays() {
    use eframe::egui::{Role, epaint::Shape};
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(360.0, 360.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("image-menu-layout");
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
                        ui.ctx().request_discard("test: repeated image-menu layout");
                    }
                },
                App::new(None).with_settings(dir.clone()),
            );
        h.run();
        h.ctx.style_mut_of(h.ctx.theme(), |style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        for (menu, entries) in [
            (
                "Rotate",
                vec![
                    "Rotate right 90°",
                    "Rotate left 90°",
                    "Rotate 180°",
                    "Rotate custom",
                ],
            ),
            ("Flip", vec!["Flip vertical", "Flip horizontal"]),
        ] {
            h.get_by_role_and_label(Role::Button, menu).hover();
            h.run();
            assert_eq!(h.query_all_by_role_and_label(Role::Label, menu).count(), 1);
            h.get_by_role_and_label(Role::Button, menu).click();
            h.run();
            for entry in &entries {
                h.get_by_label(entry).scroll_to_me();
                h.run();
                let row = h.get_by_label(entry).rect();
                assert!(
                    row.height() >= 40.0
                        && row.min.x >= 0.0
                        && row.min.y >= 0.0
                        && row.max.x <= size.x
                        && row.max.y <= size.y,
                    "{entry}: {row:?} outside {size:?}"
                );
                assert!(h.get_by_label(entry).accesskit_node().is_disabled());
                assert!(
                    h.output().shapes.iter().any(|shape| {
                        let Shape::Rect(icon) = &shape.shape else {
                            return false;
                        };
                        icon.brush.is_some()
                            && row.contains_rect(icon.rect)
                            && icon.rect.size() == Vec2::splat(22.0)
                    }),
                    "{entry} must draw its full 22-point icon"
                );
            }
            h.get_by_label(entries.last().unwrap()).hover();
            h.run();
            assert_eq!(
                h.query_all_by_role_and_label(
                    Role::Label,
                    "This image transformation is not available yet."
                )
                .count(),
                1
            );
            h.get_by_label(entries.last().unwrap()).click();
            h.run();
            assert!(
                h.query_by_label(entries[0]).is_some(),
                "a disabled entry keeps the popup open"
            );
            h.key_press(Key::Escape);
            h.run();
            assert!(h.query_by_label(entries[0]).is_none());
        }
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn brush_gallery_scrolls_lower_choices_into_a_short_window() {
    let mut h = harness();
    h.set_size(Vec2::new(1100.0, 500.0));
    h.run();
    h.get_by_role_and_label(eframe::egui::Role::Button, "Brushes")
        .click();
    h.run();
    h.get_by_label("Square brush").scroll_to_me();
    h.run();
    assert!(h.get_by_label("Square brush").rect().max.y <= 500.0);
    h.get_by_label("Square brush").click();
    h.run();
    assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Brush);
    assert!(!h.state().editor().options.round);
}

#[test]
fn classic_image_and_shape_previews_are_present_without_enabling_unimplemented_edits() {
    let mut h = harness();
    let before = h.state().editor().document_snapshot();
    assert!(
        h.get_by_label("Remove background")
            .accesskit_node()
            .is_disabled()
    );
    for label in [
        "polygon",
        "right-triangle",
        "diamond",
        "pentagon",
        "right-arrow",
        "left-arrow",
        "up-arrow",
        "down-arrow",
        "four-point-star",
        "six-point-star",
        "rectangular-callout",
        "rounded-speech-bubble",
        "cloud-callout",
    ] {
        h.get_by_label(label).scroll_to_me();
        h.run();
        assert!(
            h.get_by_label(label).accesskit_node().is_disabled(),
            "{label}"
        );
    }
    assert!(h.state().editor().document().same_semantics(&before));
}
