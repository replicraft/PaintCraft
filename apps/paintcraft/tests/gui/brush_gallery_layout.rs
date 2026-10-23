use super::*;
use eframe::egui::{Role, accesskit::Toggled, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

fn row_visible(h: &Harness<App>, accessible: &str, label: &str) -> bool {
    let rect = h.get_by_label(accessible).rect();
    h.output().shapes.iter().any(|shape| {
        matches!(&shape.shape, Shape::Text(text) if text.galley.job.text == label)
            && shape.clip_rect.contains_rect(rect.shrink(0.5))
    })
}

#[test]
fn brush_gallery_reveals_its_selected_tip_only_when_opened() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(1360.0, 900.0), 2.0, Appearance::Dark),
        (Vec2::new(480.0, 360.0), 1.0, Appearance::Light),
        (Vec2::new(480.0, 360.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("brush-gallery-opening");
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
        h.get_by_role_and_label(Role::Button, "Brushes")
            .scroll_to_me();
        h.run();
        let document = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        h.get_by_role_and_label(Role::Button, "Brushes").click();
        h.run();
        h.get_by_label("Square brush").scroll_to_me();
        h.run();
        h.get_by_label("Square brush").click();
        h.run();
        h.get_by_role_and_label(Role::Button, "Brushes").click();
        h.run();
        h.get_by_label("Round brush").scroll_to_me();
        h.run();
        assert!(row_visible(&h, "Round brush", "Brush"));
        assert!(!row_visible(&h, "Square brush", "Square brush"));
        h.key_press(Key::Escape);
        h.run();
        h.get_by_role_and_label(Role::Button, "Brushes").click();
        h.run();
        assert!(
            row_visible(&h, "Square brush", "Square brush"),
            "{size:?} / {ppp}: square={:?}, round={}, clips={:?}",
            h.get_by_label("Square brush").rect(),
            h.state().editor().options.round,
            h.output()
                .shapes
                .iter()
                .filter_map(|s| {
                    matches!(&s.shape, Shape::Text(t) if t.galley.job.text=="Square brush")
                        .then_some(s.clip_rect)
                })
                .collect::<Vec<_>>()
        );
        assert_eq!(
            h.get_by_label("Square brush").accesskit_node().toggled(),
            Some(Toggled::True)
        );
        let settled = h.get_by_label("Square brush").rect();
        h.run_steps(12);
        assert_eq!(h.get_by_label("Square brush").rect(), settled);
        assert!(row_visible(&h, "Square brush", "Square brush"));
        h.get_by_label("Round brush").scroll_to_me();
        h.run();
        assert!(row_visible(&h, "Round brush", "Brush"));
        assert!(!row_visible(&h, "Square brush", "Square brush"));
        h.key_press(Key::Escape);
        h.run();
        assert!(!h.state().editor().options.round);
        assert!(h.state().editor().document().same_semantics(&document));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit GPU Classic brush gallery opening review"]
fn export_brush_gallery_opening() {
    let output = std::env::temp_dir().join("paintcraft-brush-gallery-opening");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        for (size, layout) in [
            (Vec2::new(1360.0, 900.0), "desktop"),
            (Vec2::new(480.0, 360.0), "compact"),
        ] {
            let dir = scratch("brush-opening-review");
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
            h.get_by_role_and_label(Role::Button, "Brushes")
                .scroll_to_me();
            h.run();
            let document = h.state().editor().document_snapshot();
            let ticket = h.state().editor().history_view().ticket;
            h.get_by_role_and_label(Role::Button, "Brushes").click();
            h.run();
            h.get_by_label("Square brush").scroll_to_me();
            h.run();
            h.get_by_label("Square brush").click();
            h.run();
            h.get_by_role_and_label(Role::Button, "Brushes").click();
            h.run();
            h.get_by_label("Round brush").scroll_to_me();
            h.run();
            h.key_press(Key::Escape);
            h.run();
            h.get_by_role_and_label(Role::Button, "Brushes").click();
            h.run();
            h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
            h.run();
            assert!(row_visible(&h, "Square brush", "Square brush"));
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{layout}-selected.png")))
                .unwrap();
            h.get_by_label("Round brush").scroll_to_me();
            h.run();
            assert!(row_visible(&h, "Round brush", "Brush"));
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{layout}-scrolled.png")))
                .unwrap();
            h.key_press(Key::Escape);
            h.run();
            assert!(!h.state().editor().options.round);
            assert!(h.state().editor().document().same_semantics(&document));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn brush_gallery_has_reference_sized_rows_and_reachable_truthful_choices() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(1360.0, 900.0), 2.0, Appearance::Dark),
        (Vec2::new(360.0, 360.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("brush-gallery-layout");
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
                            .request_discard("test: repeated brush gallery layout");
                    }
                },
                App::new(None).with_settings(dir.clone()),
            );
        h.run();
        h.ctx.style_mut_of(h.ctx.theme(), |style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        let base = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        h.get_by_role_and_label(Role::Button, "Brushes")
            .scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::Button, "Brushes").click();
        h.run();
        for (accessible, label, enabled) in [
            ("Round brush", "Brush", true),
            ("Calligraphy brush", "Calligraphy brush", false),
            ("Calligraphy pen", "Calligraphy pen", false),
            ("Airbrush", "Airbrush", false),
            ("Oil brush", "Oil brush", false),
            ("Crayon", "Crayon", false),
            ("Marker", "Marker", false),
            ("Natural pencil", "Natural pencil", false),
            ("Watercolor brush", "Watercolor brush", false),
            ("Square brush", "Square brush", true),
        ] {
            h.get_by_role_and_label(Role::Button, accessible)
                .scroll_to_me();
            h.run();
            let node = h.get_by_role_and_label(Role::Button, accessible);
            let rect = node.rect();
            assert_eq!(rect.height(), 56.0);
            assert!(rect.width() >= 320.0 && rect.width() <= 336.0);
            assert!(
                rect.min.x >= 0.0
                    && rect.min.y >= 0.0
                    && rect.max.x <= size.x
                    && rect.max.y <= size.y,
                "{accessible}: {rect:?} outside {size:?}"
            );
            assert_eq!(node.accesskit_node().is_disabled(), !enabled);
            let text = h
                .output()
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    Shape::Text(text)
                        if text.galley.job.text == label && rect.contains(text.pos) =>
                    {
                        Some(text)
                    }
                    _ => None,
                })
                .expect("gallery label is painted");
            assert_eq!(text.galley.job.sections[0].format.font_id.size, 16.0);
            let sample_left = rect.right() - 90.0 - 42.0;
            assert!(
                text.visual_bounding_rect().right() + 10.0 < sample_left,
                "{label} must leave room for its sample"
            );
        }
        h.get_by_label("Oil brush").scroll_to_me();
        h.run();
        h.get_by_label("Oil brush").hover();
        h.run();
        assert_eq!(
            h.query_all_by_role_and_label(Role::Label, "This brush is not available yet.")
                .count(),
            1
        );
        h.get_by_label("Oil brush").click();
        h.run();
        assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Pencil);
        h.get_by_label("Square brush").scroll_to_me();
        h.run();
        h.get_by_label("Square brush").click();
        h.run();
        assert!(!h.state().editor().options.round);
        assert!(h.query_by_label("Square brush").is_none());
        h.get_by_role_and_label(Role::Button, "Brushes").click();
        h.run();
        assert_eq!(
            h.get_by_label("Square brush").accesskit_node().toggled(),
            Some(Toggled::True)
        );
        h.get_by_label("Round brush").scroll_to_me();
        h.run();
        h.get_by_label("Round brush").click();
        h.run();
        assert!(h.state().editor().options.round);
        assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Brush);
        assert!(h.state().editor().document().same_semantics(&base));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
