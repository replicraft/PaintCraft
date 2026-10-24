use super::*;
use eframe::egui::{Role, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};
use paintcraft_editor::execute::Selection;

fn open(h: &mut Harness<App>) {
    h.get_by_role_and_label(Role::Button, "Selection options")
        .click();
    h.run();
}

fn reveal(h: &mut Harness<App>, label: &str) {
    h.get_by_role_and_label(Role::Button, "Selection options")
        .get_by_role_and_label(Role::Button, label)
        .scroll_to_me();
    h.run();
}

#[test]
fn classic_selection_menu_has_aligned_rows_safe_previews_and_reachable_actions() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(1360.0, 900.0), 2.0, Appearance::Dark),
        (Vec2::new(360.0, 360.0), 1.0, Appearance::Dark),
        (Vec2::new(480.0, 360.0), 2.0, Appearance::Light),
    ] {
        let dir = scratch("classic-selection-menu");
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
                        ui.ctx().request_discard("test: repeated selection layout");
                    }
                },
                App::new(None).with_settings(dir.clone()),
            );
        h.run();
        h.ctx.style_mut_of(h.ctx.theme(), |style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        let viewport = h.state().canvas_viewport().unwrap();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        open(&mut h);
        assert!(
            h.query_by_label("Rectangular selection").is_some(),
            "selection popup did not open at {size:?}, scale {ppp}; tool={:?}, pubs={}, popups={}",
            h.state().editor().tool,
            publications(&h),
            eframe::egui::Popup::is_any_open(&h.ctx)
        );
        let labels = [
            "Rectangular selection",
            "Free-form selection",
            "Select all",
            "Invert selection",
            "Transparent selection",
            "Delete",
            "Deselect",
        ];
        let mut previous_bottom = 0.0;
        let left = h
            .get_by_role_and_label(Role::Button, "Selection options")
            .get_by_label(labels[0])
            .rect()
            .left();
        for label in labels {
            reveal(&mut h, label);
            let rect = h
                .get_by_role_and_label(Role::Button, "Selection options")
                .get_by_role_and_label(Role::Button, label)
                .rect();
            if size.y > 400.0 {
                assert!(rect.top() >= previous_bottom, "row ordering: {label}");
            }
            assert_eq!(rect.height(), 40.0);
            assert!(
                rect.width() >= 260.0 && rect.width() <= 276.0,
                "bounded {label} width: {rect:?}"
            );
            assert_eq!(rect.left(), left);
            previous_bottom = rect.bottom();
            let display = match label {
                "Rectangular selection" => "Rectangle",
                "Free-form selection" => "Free-form",
                other => other,
            };
            let text = h
                .output()
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    Shape::Text(text)
                        if text.galley.job.text == display && rect.contains(text.pos) =>
                    {
                        Some(text)
                    }
                    _ => None,
                })
                .expect("selection label is painted");
            assert_eq!(text.galley.job.sections[0].format.font_id.size, 16.0);
            if matches!(
                label,
                "Rectangular selection"
                    | "Free-form selection"
                    | "Select all"
                    | "Invert selection"
                    | "Delete"
            ) {
                assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
                    Shape::Rect(image) if image.brush.is_some() && rect.contains_rect(image.rect) && image.rect.size()==Vec2::splat(22.0)
                        && image.fill.r()==image.fill.g() && image.fill.g()==image.fill.b()
                )),"{label} paints its monochrome glyph");
            }
        }
        if size.y > 400.0 {
            let mut textures = std::collections::HashSet::new();
            for label in [
                "Rectangular selection",
                "Free-form selection",
                "Select all",
                "Invert selection",
                "Delete",
            ] {
                let rect = h.get_by_label(label).rect();
                let texture = h
                    .output()
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        Shape::Rect(image) if rect.contains_rect(image.rect) => {
                            image.brush.as_ref().map(|brush| brush.fill_texture_id)
                        }
                        _ => None,
                    })
                    .expect("selection glyph is present");
                assert!(textures.insert(texture), "{label} has distinct artwork");
            }
        }
        assert!(
            h.get_by_role_and_label(Role::Button, "Selection options")
                .get_by_label("Deselect")
                .accesskit_node()
                .is_disabled()
        );
        for (label, tooltip) in [
            (
                "Free-form selection",
                "Free-form selection is not available yet.",
            ),
            (
                "Invert selection",
                "Inverting selections is not available yet.",
            ),
            (
                "Transparent selection",
                "Transparent selection is not available yet.",
            ),
        ] {
            reveal(&mut h, label);
            assert!(
                h.get_by_role_and_label(Role::Button, "Selection options")
                    .get_by_label(label)
                    .accesskit_node()
                    .is_disabled()
            );
            h.get_by_role_and_label(Role::Button, "Selection options")
                .get_by_label(label)
                .hover();
            h.run();
            assert_eq!(
                h.query_all_by_role_and_label(Role::Label, tooltip).count(),
                1
            );
            h.get_by_role_and_label(Role::Button, "Selection options")
                .get_by_label(label)
                .click();
            h.run();
            assert!(h.query_by_label("Rectangular selection").is_some());
            assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Pencil);
            assert_eq!(h.state().editor().selection(), Selection::Inactive);
            assert_eq!(h.state().editor().history_view().ticket, ticket);
        }
        reveal(&mut h, "Rectangular selection");
        h.get_by_role_and_label(Role::Button, "Selection options")
            .get_by_label("Rectangular selection")
            .click();
        h.run();
        assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Select);
        assert_eq!(
            h.state().canvas_viewport().unwrap(),
            viewport,
            "Select must not add a second action row"
        );
        assert!(
            h.query_by_role_and_label(Role::Button, "Deselect")
                .is_none()
        );
        assert!(h.query_by_label("Rectangular selection").is_none());
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        open(&mut h);
        reveal(&mut h, "Rectangular selection");
        let rect = h
            .get_by_role_and_label(Role::Button, "Selection options")
            .get_by_label("Rectangular selection")
            .rect();
        assert_eq!(
            h.get_by_role_and_label(Role::Button, "Selection options")
                .get_by_label("Rectangular selection")
                .accesskit_node()
                .toggled(),
            Some(eframe::egui::accesskit::Toggled::True)
        );
        assert!(h.output().shapes.iter().any(|shape| {
            matches!(&shape.shape, Shape::Rect(marker)
                if marker.brush.is_none() && rect.contains_rect(marker.rect)
                && marker.rect.size() == Vec2::new(3.0, 22.0)
                && marker.fill.a() > 0)
        }));
        let initial = publications(&h);
        reveal(&mut h, "Select all");
        h.get_by_role_and_label(Role::Button, "Selection options")
            .get_by_label("Select all")
            .click();
        h.run();
        assert!(matches!(h.state().editor().selection(), Selection::Rect(_)));
        assert_eq!(publications(&h), initial + 1);
        open(&mut h);
        reveal(&mut h, "Deselect");
        let bottom = h
            .get_by_role_and_label(Role::Button, "Selection options")
            .get_by_label("Deselect")
            .rect();
        assert!(bottom.max.x <= size.x && bottom.max.y <= size.y);
        assert!(
            !h.get_by_role_and_label(Role::Button, "Selection options")
                .get_by_label("Deselect")
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_role_and_label(Role::Button, "Selection options")
            .get_by_label("Deselect")
            .click();
        h.run();
        assert_eq!(h.state().editor().selection(), Selection::Inactive);
        assert_eq!(publications(&h), initial + 2);
        assert!(h.state().editor().document().same_semantics(&snapshot));
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

fn selected_row_fill(h: &Harness<App>) -> eframe::egui::Color32 {
    let rect = h
        .get_by_role_and_label(Role::Button, "Rectangular selection")
        .rect();
    let surface = h
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            Shape::Rect(painted)
                if painted.rect.center().distance(rect.center()) < 0.1
                    && painted.rect.width() >= rect.width() - 2.0
                    && painted.rect.height() >= rect.height() - 2.0
                    && painted.fill.a() == 255 =>
            {
                Some(painted)
            }
            _ => None,
        })
        .expect("selected menu row has a painted surface");
    assert_eq!(
        surface.fill.r(),
        surface.fill.g(),
        "selected row uses a neutral fill"
    );
    assert_eq!(
        surface.fill.g(),
        surface.fill.b(),
        "selected row uses a neutral fill"
    );
    assert_eq!(
        surface.stroke.width, 0.0,
        "the accent belongs to the leading indicator"
    );
    let accent = h.ctx.style_of(h.ctx.theme()).visuals.selection.stroke.color;
    assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
        Shape::Rect(marker) if rect.contains_rect(marker.rect) && marker.rect.size()==Vec2::new(3.0,22.0)
            && marker.fill==accent)),"selected row retains its accent indicator");
    surface.fill
}

#[test]
fn selection_popup_uses_neutral_selected_rows_with_accent_indicators_and_native_input() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(480.0, 360.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("selection-row-surfaces");
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
        open(&mut h);
        assert_eq!(
            h.state().editor().tool,
            paintcraft_tools::Tool::Pencil,
            "opening the mode menu must not activate Select"
        );
        assert_eq!(
            h.get_by_role_and_label(Role::Button, "Rectangular selection")
                .accesskit_node()
                .toggled(),
            Some(eframe::egui::accesskit::Toggled::True),
            "Rectangle remains the selected mode while Pencil is active"
        );
        h.event(Event::PointerMoved(Pos2::new(4.0, 4.0)));
        h.run();
        let idle = selected_row_fill(&h);
        h.get_by_role_and_label(Role::Button, "Rectangular selection")
            .hover();
        h.run();
        assert_ne!(selected_row_fill(&h), idle);
        h.get_by_role_and_label(Role::Button, "Rectangular selection")
            .focus();
        h.run();
        selected_row_fill(&h);
        h.key_press(Key::Enter);
        h.run();
        assert!(
            h.query_by_role_and_label(Role::Button, "Rectangular selection")
                .is_none()
        );
        assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Select);
        assert_eq!(h.state().editor().selection(), Selection::Inactive);
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit GPU Classic Selection mode row review"]
fn export_selection_row_surfaces() {
    let output = std::env::temp_dir().join("paintcraft-selection-row-surfaces");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        for (size, layout) in [
            (Vec2::new(1360.0, 900.0), "desktop"),
            (Vec2::new(480.0, 360.0), "compact"),
        ] {
            let dir = scratch("selection-row-review");
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
            open(&mut h);
            h.event(Event::PointerMoved(Pos2::new(4.0, 4.0)));
            h.run();
            selected_row_fill(&h);
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{layout}-idle.png")))
                .unwrap();
            h.get_by_role_and_label(Role::Button, "Rectangular selection")
                .hover();
            h.run();
            selected_row_fill(&h);
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{layout}-hover.png")))
                .unwrap();
            h.key_press(Key::Escape);
            h.run();
            assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Pencil);
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
