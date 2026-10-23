use super::*;
use eframe::egui::{Rect, Role, epaint::Shape};
use paintcraft::settings::{Appearance, Settings};

fn has_surface(h: &Harness<App>, bounds: Rect, outline: bool) -> bool {
    h.output().shapes.iter().any(|shape| {
        let Shape::Rect(rect) = &shape.shape else {
            return false;
        };
        rect.brush.is_none()
            && rect.rect.center().distance(bounds.center()) < 1.0
            && rect.rect.width() >= bounds.width() - 3.0
            && rect.rect.height() >= bounds.height() - 3.0
            && if outline {
                rect.stroke.width > 0.0 && rect.stroke.color.a() > 0
            } else {
                rect.fill.a() > 0 || (rect.stroke.width > 0.0 && rect.stroke.color.a() > 0)
            }
    })
}

#[test]
fn classic_dropdowns_highlight_hover_and_keep_an_outline_while_open() {
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("dropdown-states");
        Settings {
            appearance,
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.get_by_role_and_label(Role::Button, "Rectangle").click();
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for (trigger, entry) in [
            ("Selection options", "Rectangular selection"),
            ("Brushes", "Round brush"),
            ("Shape outline", "Solid outline"),
            ("Shape fill", "No fill"),
            ("Rotate", "Rotate right 90°"),
            ("Flip", "Flip vertical"),
        ] {
            h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
            h.run();
            let rect = h.get_by_role_and_label(Role::Button, trigger).rect();
            assert!(!has_surface(&h, rect, false), "{trigger} idle surface");
            h.get_by_role_and_label(Role::Button, trigger).hover();
            h.run();
            assert!(has_surface(&h, rect, false), "{trigger} hover surface");
            h.get_by_role_and_label(Role::Button, trigger).click();
            h.run();
            h.get_by_label(entry).hover();
            h.run();
            assert!(has_surface(&h, rect, true), "{trigger} open outline");
            assert_eq!(
                h.get_by_role_and_label(Role::Button, trigger)
                    .accesskit_node()
                    .toggled(),
                Some(eframe::egui::accesskit::Toggled::True)
            );
            h.key_press(Key::Escape);
            h.run();
            assert!(h.query_by_label(entry).is_none(), "{trigger} must close");
            assert_eq!(
                h.get_by_role_and_label(Role::Button, trigger)
                    .accesskit_node()
                    .toggled(),
                Some(eframe::egui::accesskit::Toggled::False)
            );
        }
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn shape_paint_choices_have_contiguous_readable_rows_and_safe_option_transitions() {
    use paintcraft::settings::ClassicVariant;
    use paintcraft_tools::ShapeStyle;
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(1360.0, 900.0), 2.0, Appearance::Dark),
        (Vec2::new(360.0, 360.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("shape-paint-layout");
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
                        ui.ctx()
                            .request_discard("test: repeated shape paint layout");
                    }
                },
                App::new(None).with_settings(dir.clone()),
            );
        h.run();
        // Reveal the outer ribbon through a direct child; Rectangle also has
        // the inner vertical shape-gallery scroller as an ancestor.
        h.get_by_role_and_label(Role::Button, "Shape outline")
            .scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::Button, "Rectangle").click();
        h.run();
        assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Rect);
        let base = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for (trigger, labels, selected) in [
            (
                "Shape outline",
                ["No outline", "Solid outline"],
                "Solid outline",
            ),
            ("Shape fill", ["No fill", "Solid fill"], "No fill"),
        ] {
            h.get_by_label(trigger).scroll_to_me();
            h.run();
            let bounds = h.get_by_label(trigger).rect();
            let glyphs: Vec<_> = h
                .output()
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    Shape::Rect(image)
                        if image.brush.is_some() && bounds.intersects(image.rect) =>
                    {
                        Some((image.rect, image.fill))
                    }
                    _ => None,
                })
                .collect();
            assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
                Shape::Rect(image) if image.brush.is_some() && bounds.contains_rect(image.rect)
                    && image.rect.size() == Vec2::splat(22.0)
                    && image.fill.r() == image.fill.g() && image.fill.g() == image.fill.b()
                    && (if appearance == Appearance::Dark {image.fill.r() >= 220} else {image.fill.r() <= 40})
            )), "{trigger} uses a theme-aware outline glyph at {size:?} / {ppp}: bounds={bounds:?}, glyphs={glyphs:?}, tool={:?}",h.state().editor().tool);
            h.get_by_label(trigger).click();
            h.run();
            let mut previous_bottom = None;
            let mut text_x = None;
            for label in labels {
                let node = h.get_by_role_and_label(Role::Button, label);
                let rect = node.rect();
                assert_eq!(rect.height(), 40.0);
                assert_eq!(rect.width(), 220.0);
                assert!(
                    rect.min.x >= 0.0
                        && rect.min.y >= 0.0
                        && rect.max.x <= size.x
                        && rect.max.y <= size.y
                );
                if let Some(bottom) = previous_bottom {
                    assert_eq!(rect.top(), bottom);
                }
                previous_bottom = Some(rect.bottom());
                assert_eq!(
                    node.accesskit_node().toggled(),
                    Some(if label == selected {
                        eframe::egui::accesskit::Toggled::True
                    } else {
                        eframe::egui::accesskit::Toggled::False
                    })
                );
                let text = h
                    .output()
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        Shape::Text(text) if text.galley.job.text == label => Some(text),
                        _ => None,
                    })
                    .expect("shape paint label is painted");
                assert_eq!(text.galley.job.sections[0].format.font_id.size, 16.0);
                if let Some(x) = text_x {
                    assert_eq!(text.pos.x, x);
                }
                text_x = Some(text.pos.x);
            }
            assert!(has_surface(&h, h.get_by_label(selected).rect(), true));
            h.key_press(Key::Escape);
            h.run();
            assert!(h.query_by_label(selected).is_none());
        }
        for (trigger, choice, expected) in [
            ("Shape fill", "Solid fill", ShapeStyle::Both),
            ("Shape outline", "No outline", ShapeStyle::Filled),
            ("Shape fill", "No fill", ShapeStyle::Outline),
            ("Shape outline", "Solid outline", ShapeStyle::Outline),
        ] {
            h.get_by_label(trigger).scroll_to_me();
            h.run();
            h.get_by_label(trigger).click();
            h.run();
            h.get_by_label(choice).click();
            h.run();
            assert!(h.query_by_label(choice).is_none());
            assert_eq!(h.state().editor().options.style, expected);
            assert!(h.state().editor().document().same_semantics(&base));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
        }
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
