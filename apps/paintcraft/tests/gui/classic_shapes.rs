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
        h.get_by_label("Lightning").scroll_to_me();
        h.run();
        let stars_and_callouts = [
            "Four-point star",
            "Five-point star",
            "Six-point star",
            "Rectangular callout",
            "Oval callout",
        ];
        let bottom = ["Cloud callout", "Heart", "Lightning"];
        let row_y = h.get_by_label(stars_and_callouts[0]).rect().top();
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
            assert!(button.accesskit_node().is_disabled());
            right = rect.right();
        }
        let bottom_y = h.get_by_label(bottom[0]).rect().top();
        assert!(bottom_y > row_y);
        for (column, label) in bottom.into_iter().enumerate() {
            let rect = h.get_by_label(label).rect();
            assert_eq!(rect.top(), bottom_y);
            assert_eq!(
                rect.left(),
                h.get_by_label(stars_and_callouts[column]).rect().left()
            );
            assert!(rect.bottom() < h.state().canvas_viewport().unwrap().top());
        }
        assert!(h.query_by_label("burst").is_none());
        assert!(h.query_by_label("rectangular-callout").is_none());
        h.get_by_label("Rectangular callout").hover();
        h.run();
        assert_eq!(
            h.query_all_by_role_and_label(Role::Label, "Rectangular callout is not available yet.")
                .count(),
            1
        );
        h.get_by_label("Rectangular callout").click();
        h.run();
        assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Pencil);
        for (label, tool) in [
            ("Line", paintcraft_tools::Tool::Line),
            ("Ellipse", paintcraft_tools::Tool::Ellipse),
            ("Rectangle", paintcraft_tools::Tool::Rect),
        ] {
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
