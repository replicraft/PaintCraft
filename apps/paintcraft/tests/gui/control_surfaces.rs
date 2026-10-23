use super::*;
use eframe::egui::{Color32, Role, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

fn selected_surface(h: &Harness<App>, label: &str) -> Color32 {
    let button = h.get_by_role_and_label(Role::Button, label);
    assert_eq!(
        button.accesskit_node().toggled(),
        Some(eframe::egui::accesskit::Toggled::True)
    );
    let rect = button.rect();
    let fill = h
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
                Some(painted.fill)
            }
            _ => None,
        })
        .expect("selected control has an opaque surface");
    assert_eq!(
        fill.r(),
        fill.g(),
        "{label} selected fill should be neutral: {fill:?}"
    );
    assert_eq!(
        fill.g(),
        fill.b(),
        "{label} selected fill should be neutral: {fill:?}"
    );
    assert!(
        h.output().shapes.iter().any(|shape| matches!(&shape.shape,
        Shape::Rect(painted) if painted.rect.center().distance(rect.center())<0.1
            && painted.rect.width()>=rect.width()-2.0 && painted.stroke.width>0.0
            && painted.stroke.color.b()>painted.stroke.color.r())),
        "{label} retains an accent outline"
    );
    fill
}

#[test]
fn classic_selected_controls_have_neutral_state_surfaces_without_changing_text_selection() {
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("classic-control-surfaces");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.set_size(Vec2::new(1360.0, 900.0));
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        h.event(Event::PointerMoved(Pos2::new(4.0, 4.0)));
        h.run();
        let idle = selected_surface(&h, "Pencil");
        h.get_by_role_and_label(Role::Button, "Pencil").hover();
        h.run();
        let hovered = selected_surface(&h, "Pencil");
        assert_ne!(idle, hovered);
        let point = h
            .get_by_role_and_label(Role::Button, "Pencil")
            .rect()
            .center();
        h.event(press(point));
        h.step();
        let pressed = selected_surface(&h, "Pencil");
        assert_ne!(pressed, hovered);
        h.event(release(point));
        h.run();
        for label in ["Select", "Brush", "Rectangle", "Text", "Align left"] {
            if label != "Align left" {
                h.get_by_role_and_label(Role::Button, label).click();
                h.run();
            }
            h.event(Event::PointerMoved(Pos2::new(4.0, 4.0)));
            h.run();
            selected_surface(&h, label);
            if label == "Align left" {
                let bounds = h.get_by_label(label).rect();
                assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
                    Shape::LineSegment { points, stroke } if points.iter().all(|point| bounds.contains(*point))
                        && if appearance == Appearance::Dark {stroke.color.r() >= 220} else {stroke.color.r() <= 40})),
                    "alignment glyph follows the strong control foreground");
            }
        }
        for label in ["Bold", "Layers"] {
            h.get_by_role_and_label(Role::Button, label).click();
            h.run();
            selected_surface(&h, label);
        }
        h.get_by_label("Font size").focus();
        h.run();
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
        h.run();
        let field = h.get_by_label("Font size").rect();
        let accent_fill = h.ctx.style_of(h.ctx.theme()).visuals.selection.bg_fill;
        assert!(accent_fill.b() > accent_fill.r());
        assert!(
            h.output().shapes.iter().any(|shape| matches!(&shape.shape,
            Shape::Text(text) if field.contains(text.pos) && text.galley.rows.iter().any(|row|
                row.visuals.mesh.vertices.iter().any(|vertex| vertex.color == accent_fill)))),
            "text selection stays accented"
        );
        h.get_by_label("Font size presets").click();
        h.run();
        let choice = h.get_by_label("11 pt").rect();
        assert!(
            h.output().shapes.iter().any(|shape| matches!(&shape.shape,
            Shape::Rect(painted) if choice.contains_rect(painted.rect)
                && painted.fill == accent_fill && painted.rect.width() > choice.width() - 2.0)),
            "popup choices retain their accented selection surface"
        );
        h.key_press(Key::Escape);
        h.run();
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit GPU Classic selected/hover/pressed surface review"]
fn export_classic_control_surfaces() {
    let output = std::env::temp_dir().join("paintcraft-control-surfaces");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        let dir = scratch("classic-control-surface-review");
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
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        h.event(Event::PointerMoved(Pos2::new(4.0, 4.0)));
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-pencil-idle.png")))
            .unwrap();
        h.get_by_role_and_label(Role::Button, "Pencil").hover();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-pencil-hover.png")))
            .unwrap();
        let point = h
            .get_by_role_and_label(Role::Button, "Pencil")
            .rect()
            .center();
        h.event(press(point));
        h.step();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-pencil-pressed.png")))
            .unwrap();
        h.event(release(point));
        h.run();
        for (label, name) in [("Brush", "brush"), ("Rectangle", "shape"), ("Text", "text")] {
            h.get_by_role_and_label(Role::Button, label).click();
            h.run();
            if label == "Text" {
                h.get_by_label("Bold").click();
                h.run();
            }
            h.event(Event::PointerMoved(Pos2::new(4.0, 4.0)));
            h.run();
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{name}.png")))
                .unwrap();
        }
        h.get_by_role_and_label(Role::Button, "Layers").click();
        h.run();
        h.event(Event::PointerMoved(Pos2::new(4.0, 4.0)));
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-layers.png")))
            .unwrap();
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
