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

#[test]
fn floating_slider_accent_centers_follow_egui_handles_in_the_same_frame() {
    use eframe::egui::epaint::Shape;
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("classic-slider-centers");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.get_by_label("Brush").click();
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for (size, opacity) in [(1.0, 0.0), (32.0, 50.0), (64.0, 100.0), (1024.0, 25.0)] {
            h.get_by_role_and_label(Role::SpinButton, "Size")
                .set_value_accesskit(size);
            h.get_by_role_and_label(Role::Slider, "Brush opacity")
                .set_value_accesskit(opacity);
            h.run();
            for label in ["Tool size", "Brush opacity"] {
                assert_thumb_centers(&h, label);
            }
            assert_eq!(h.state().editor().options.size, size as u32);
            assert_eq!(h.state().editor().options.brush_opacity, opacity as u8);
        }
        h.get_by_role_and_label(Role::Slider, "Brush opacity")
            .focus();
        h.run();
        h.key_press(Key::ArrowUp);
        h.run();
        assert!(h.state().editor().options.brush_opacity > 25);
        assert_thumb_centers(&h, "Brush opacity");
        let rail = h.get_by_role_and_label(Role::Slider, "Tool size").rect();
        let start = rail.center_bottom() - Vec2::new(0.0, 40.0);
        let end = rail.center_top() + Vec2::new(0.0, 40.0);
        h.input_mut().events.extend([
            Event::PointerMoved(start),
            press(start),
            Event::PointerMoved(end),
        ]);
        h.run();
        assert_thumb_centers(&h, "Tool size");
        h.event(release(end));
        h.run();
        assert_thumb_centers(&h, "Tool size");
        assert!(h.state().editor().options.size <= 64);
        assert!(!h.state().editor().gesture_in_progress());
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
    fn assert_thumb_centers(h: &Harness<App>, label: &str) {
        let rect = h.get_by_role_and_label(Role::Slider, label).rect();
        assert_eq!(rect.width(), 32.0);
        let circles: Vec<_> = h
            .output()
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                Shape::Circle(circle) if rect.contains(circle.center) => Some(circle),
                _ => None,
            })
            .collect();
        let shell = circles.iter().find(|circle| circle.radius > 10.0).unwrap();
        let accent = circles
            .iter()
            .find(|circle| (5.0..=7.0).contains(&circle.radius))
            .unwrap();
        assert!(
            shell.center.distance(accent.center) < 0.01,
            "{label}: {:?} != {:?}",
            shell.center,
            accent.center
        );
        assert!(
            accent.fill.b() > accent.fill.r(),
            "accent center must be blue"
        );
        assert_eq!(shell.fill.r(), shell.fill.g());
        assert_eq!(shell.fill.g(), shell.fill.b());
    }
}

#[test]
#[ignore = "requires GPU; exports floating slider thumb states"]
fn export_floating_slider_thumbs() {
    let output = std::env::temp_dir().join("paintcraft-slider-thumbs");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        let dir = scratch("slider-thumb-review");
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
        h.get_by_label("Brush").click();
        h.run();
        for (size, opacity, name) in [
            (1.0, 0.0, "minimum"),
            (32.0, 50.0, "middle"),
            (64.0, 100.0, "maximum"),
        ] {
            h.get_by_role_and_label(Role::SpinButton, "Size")
                .set_value_accesskit(size);
            h.get_by_role_and_label(Role::Slider, "Brush opacity")
                .set_value_accesskit(opacity);
            h.event(Event::PointerMoved(Pos2::new(1300.0, 500.0)));
            h.run();
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{name}.png")))
                .unwrap();
        }
        h.get_by_role_and_label(Role::Slider, "Brush opacity")
            .hover();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-hover.png")))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
