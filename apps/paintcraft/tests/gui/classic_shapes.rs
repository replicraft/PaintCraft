use super::*;
use eframe::egui::Role;

#[test]
fn classic_shape_gallery_matches_reference_rows_and_preserves_tool_and_document_contracts() {
    for ppp in [1.0, 2.0] {
        let mut h = harness_with(App::new(None), ppp, true);
        h.ctx.style_mut_of(h.ctx.theme(), |style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        h.get_by_role_and_label(Role::Button, "Lightning")
            .scroll_to_me();
        h.run();
        let stars_and_callouts = [
            "Four-point star",
            "Five-point star",
            "Six-point star",
            "Rectangular callout",
            "Oval callout",
        ];
        let bottom = ["Cloud callout", "Heart", "Lightning"];
        let row_y = h
            .get_by_role_and_label(Role::Button, stars_and_callouts[0])
            .rect()
            .top();
        let mut right = 0.0;
        for label in stars_and_callouts {
            let button = h.get_by_role_and_label(Role::Button, label);
            let rect = button.rect();
            assert_eq!(
                rect.top(),
                row_y,
                "{label} must be in the same reference row"
            );
            assert!(rect.left() >= right);
            assert_eq!(rect.size(), Vec2::splat(24.0));
            assert!(!button.accesskit_node().is_disabled());
            right = rect.right();
        }
        let bottom_y = h
            .get_by_role_and_label(Role::Button, bottom[0])
            .rect()
            .top();
        assert!(bottom_y > row_y);
        for (column, label) in bottom.into_iter().enumerate() {
            let rect = h.get_by_role_and_label(Role::Button, label).rect();
            assert_eq!(rect.top(), bottom_y);
            assert_eq!(
                rect.left(),
                h.get_by_role_and_label(Role::Button, stars_and_callouts[column])
                    .rect()
                    .left()
            );
            assert!(rect.bottom() < h.state().canvas_viewport().unwrap().top());
        }
        assert!(h.query_by_label("burst").is_none());
        assert!(h.query_by_label("rectangular-callout").is_none());
        h.get_by_role_and_label(Role::Button, "Rectangular callout")
            .hover();
        h.run();
        assert_eq!(
            h.query_all_by_role_and_label(Role::Label, "Rectangular callout")
                .count(),
            1
        );
        h.get_by_role_and_label(Role::Button, "Rectangular callout")
            .click();
        h.run();
        assert_eq!(
            h.state().editor().tool,
            paintcraft_tools::Tool::Preset(paintcraft_tools::ShapePreset::RectangularCallout)
        );
        for (label, tool) in [
            ("Line", paintcraft_tools::Tool::Line),
            ("Ellipse", paintcraft_tools::Tool::Ellipse),
            ("Rectangle", paintcraft_tools::Tool::Rect),
            ("Curve", paintcraft_tools::Tool::Curve),
            ("Polygon", paintcraft_tools::Tool::Polygon),
        ]
        .into_iter()
        .chain(
            paintcraft_tools::ShapePreset::ALL
                .map(|preset| (preset.name(), paintcraft_tools::Tool::Preset(preset))),
        ) {
            h.get_by_role_and_label(Role::Button, label).scroll_to_me();
            h.run();
            h.get_by_role_and_label(Role::Button, label).click();
            h.run();
            assert_eq!(h.state().editor().tool, tool);
            assert_eq!(
                h.get_by_role_and_label(Role::Button, label)
                    .accesskit_node()
                    .toggled(),
                Some(eframe::egui::accesskit::Toggled::True)
            );
        }
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
    }
}

#[test]
fn desktop_galleries_resize_without_losing_actions_colors_or_history() {
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("desktop-gallery-density");
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
        for width in [1360.0, 1200.0, 1199.0, 1100.0, 1360.0] {
            h.set_size(Vec2::new(width, 900.0));
            h.run();
            let desktop = width >= 1200.0;
            for label in ["Line", "Ellipse", "Rectangle"] {
                assert_eq!(
                    h.get_by_role_and_label(Role::Button, label).rect().size(),
                    Vec2::splat(if desktop { 28.0 } else { 24.0 })
                );
            }
            let swatch = h.get_by_label("Color #ED1C24").rect();
            assert_eq!(
                swatch.size(),
                Vec2::splat(if desktop { 24.0 } else { 20.0 })
            );
            if desktop {
                assert_eq!(
                    h.get_by_label("Edit colors…").rect().size(),
                    Vec2::splat(40.0)
                );
            }
            let layers = h.get_by_role_and_label(Role::Button, "Layers").rect();
            assert!(
                layers.max.x <= width,
                "Layers must remain visible at {width}: {layers:?}"
            );
            let captions: Vec<_> = h
                .output()
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    eframe::egui::epaint::Shape::Text(text)
                        if ["Tools", "Shapes", "Colors", "Layers"]
                            .contains(&text.galley.job.text.as_str()) =>
                    {
                        Some(text.pos.y)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(captions.len(), 4);
            assert!(
                captions.iter().all(|y| (*y - captions[0]).abs() < 0.5),
                "captions must align at {width}: {captions:?}"
            );
            h.get_by_label("Color #ED1C24").click();
            h.run();
            h.get_by_label("Color #00A2E8").click_secondary();
            h.run();
            assert_eq!(h.state().editor().color, Rgba([237, 28, 36, 255]));
            assert_eq!(h.state().editor().secondary, Rgba([0, 162, 232, 255]));
            h.get_by_role_and_label(Role::Button, "Rectangle").click();
            h.run();
            assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Rect);
            h.get_by_label("Lightning").scroll_to_me();
            h.run();
            let last = h.get_by_label("Lightning").rect();
            assert!(last.max.y < h.state().canvas_viewport().unwrap().min.y);
            h.get_by_label("Line").scroll_to_me();
            h.run();
            h.get_by_role_and_label(Role::Button, "Layers").click();
            h.run();
            assert!(h.query_by_label("Add layer").is_some());
            h.get_by_role_and_label(Role::Button, "Layers").click();
            h.run();
            h.get_by_label("Edit colors…").click();
            h.run();
            assert!(h.query_by_label("Hex RGB").is_some());
            h.get_by_label("Cancel").click();
            h.run();
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
        }
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "requires GPU; exports desktop and compact gallery review scenes"]
fn export_gallery_density_for_review() {
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    let output = std::env::temp_dir().join("paintcraft-gallery-density");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        let dir = scratch("gallery-density-review");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        for (width, name) in [(1360.0, "desktop"), (1100.0, "compact")] {
            h.set_size(Vec2::new(width, 900.0));
            h.run();
            h.get_by_label("Line").scroll_to_me();
            h.run();
            h.get_by_label("Rectangle").click();
            h.run();
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{name}-shapes.png")))
                .unwrap();
            h.get_by_label("Lightning").scroll_to_me();
            h.run();
            h.get_by_label("Color #ED1C24").hover();
            h.run();
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{name}-scrolled.png")))
                .unwrap();
        }
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
