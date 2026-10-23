use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

#[test]
fn classic_tool_control_headers_identify_size_and_opacity_without_painting() {
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("classic-tool-headers");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.ctx.style_mut_of(h.ctx.theme(), |style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let canvas = h.state().canvas_rect();
        for tool in ["Pencil", "Brush", "Eraser", "Line", "Rectangle", "Ellipse"] {
            h.get_by_role_and_label(Role::Button, tool).click();
            h.run();
            let label = format!("{tool} size");
            let header = h.get_by_role_and_label(Role::Image, &label);
            assert_eq!(header.rect().size(), Vec2::splat(22.0));
            header.hover();
            h.run();
            assert_eq!(
                h.query_all_by_role_and_label(Role::Label, &label).count(),
                1
            );
            h.get_by_role_and_label(Role::Image, &label).click();
            h.run();
            h.get_by_role_and_label(Role::Slider, "Tool size")
                .set_value_accesskit(12.0);
            h.run();
            assert_eq!(
                if tool == "Pencil" {
                    h.state().editor().options.pencil_size
                } else {
                    h.state().editor().options.size
                },
                12
            );
            if tool == "Brush" {
                let header = h.get_by_role_and_label(Role::Image, "Brush opacity");
                assert_eq!(header.rect().size(), Vec2::splat(22.0));
                header.hover();
                h.run();
                assert_eq!(
                    h.query_all_by_role_and_label(Role::Label, "Brush opacity")
                        .count(),
                    1
                );
                h.get_by_role_and_label(Role::Image, "Brush opacity")
                    .click();
                h.run();
                h.get_by_role_and_label(Role::Slider, "Brush opacity")
                    .set_value_accesskit(65.0);
                h.run();
                assert_eq!(h.state().editor().options.brush_opacity, 65);
            }
            assert_eq!(h.state().canvas_rect(), canvas);
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            assert!(!h.state().editor().gesture_in_progress());
        }
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
