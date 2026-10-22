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
            let rect = h
                .get_by_role_and_label(Role::Button, "Selection options")
                .get_by_role_and_label(Role::Button, label)
                .rect();
            assert!(rect.top() >= previous_bottom, "row ordering: {label}");
            assert_eq!(rect.height(), 40.0);
            assert!(rect.width() >= 260.0);
            assert_eq!(rect.left(), left);
            previous_bottom = rect.bottom();
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
            ("Delete", "Deleting selected pixels is not available yet."),
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
