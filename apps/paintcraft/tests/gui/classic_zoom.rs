use super::*;

fn enter_percent(h: &mut Harness<App>, text: &str) {
    h.get_by_label("Zoom percentage").click();
    h.run();
    h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
    h.run();
    h.get_by_label("Zoom percentage").type_text(text);
    h.run();
    h.key_press(Key::Enter);
    h.run();
}

#[test]
fn classic_percentage_presets_fit_and_shortcuts_preserve_document_and_ui_scale() {
    for ppp in [1.0, 2.0] {
        let mut h = harness_with(App::new(None), ppp, true);
        let before = h.state().editor().document_snapshot();
        let depth = h.state().editor().undo_depth();
        enter_percent(&mut h, "73%");
        assert!((h.state().canvas_rect().unwrap().width() - 730.0).abs() < 0.1);
        enter_percent(&mut h, "oops");
        assert!((h.state().canvas_rect().unwrap().width() - 730.0).abs() < 0.1);
        h.get_by_label("Zoom presets").click();
        h.run();
        h.get_by_label("12.5%").scroll_to_me();
        h.run();
        h.get_by_label("12.5%").click();
        h.run();
        assert_eq!(h.state().canvas_rect().unwrap().width(), 125.0);
        h.event(key(Key::Num0, Modifiers::COMMAND));
        h.run();
        assert_eq!(h.state().canvas_rect().unwrap().width(), 1000.0);
        h.get_by_label("Zoom to window").click();
        h.run();
        let canvas = h.state().canvas_rect().unwrap();
        let viewport = h.state().canvas_viewport().unwrap();
        assert!(canvas.width() <= viewport.width() && canvas.height() <= viewport.height());
        assert!(h.state().editor().document().same_semantics(&before));
        assert_eq!(h.state().editor().undo_depth(), depth);
        assert!(!h.state().editor().is_dirty());
    }
}

#[test]
fn classic_zoom_popup_has_reference_rows_and_scrolls_on_short_displays() {
    use eframe::egui::{Role, epaint::Shape};
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(1360.0, 900.0), 2.0, Appearance::Dark),
        (Vec2::new(360.0, 360.0), 1.0, Appearance::Dark),
        (Vec2::new(480.0, 360.0), 2.0, Appearance::Light),
    ] {
        let dir = scratch("classic-zoom-popup");
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
                        ui.ctx().request_discard("test: repeated zoom layout");
                    }
                },
                App::new(None).with_settings(dir.clone()),
            );
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        enter_percent(&mut h, "73%");
        assert_eq!(
            h.get_by_label("Zoom percentage")
                .accesskit_node()
                .value()
                .as_deref(),
            Some("73%")
        );
        h.get_by_label("Zoom presets").click();
        h.run();
        let mut previous_bottom = None;
        for label in [
            "1000%", "900%", "800%", "700%", "600%", "500%", "400%", "300%", "200%", "100%", "75%",
            "50%", "25%", "12.5%",
        ] {
            let rect = h.get_by_role_and_label(Role::Button, label).rect();
            assert_eq!(rect.height(), 40.0);
            assert!(rect.width() >= 100.0);
            if let Some(bottom) = previous_bottom {
                assert!(rect.top() >= bottom, "descending order: {label}");
            }
            previous_bottom = Some(rect.bottom());
        }
        h.get_by_label("1000%").scroll_to_me();
        h.run();
        let first = h.get_by_label("1000%").rect();
        assert!(first.top() >= 0.0 && first.bottom() <= size.y);
        h.get_by_label("1000%").click();
        h.run();
        assert_eq!(h.state().canvas_rect().unwrap().width(), 10000.0);
        assert!(h.query_by_role_and_label(Role::Button, "1000%").is_none());
        assert_eq!(
            h.get_by_label("Zoom percentage")
                .accesskit_node()
                .value()
                .as_deref(),
            Some("1000%")
        );
        h.get_by_label("Zoom presets").click();
        h.run();
        h.get_by_label("1000%").scroll_to_me();
        h.run();
        let selected = h.get_by_role_and_label(Role::Button, "1000%");
        assert_eq!(
            selected.accesskit_node().toggled(),
            Some(eframe::egui::accesskit::Toggled::True)
        );
        let selected_rect = selected.rect();
        assert!(h.output().shapes.iter().any(|shape| {
            matches!(&shape.shape, Shape::Rect(outline)
                if outline.brush.is_none()
                && outline.rect.center().distance(selected_rect.center()) < 1.0
                && outline.rect.width() >= selected_rect.width() - 2.0
                && outline.stroke.width > 0.0 && outline.stroke.color.a() > 0)
        }));
        h.get_by_label("12.5%").scroll_to_me();
        h.run();
        let last = h.get_by_label("12.5%").rect();
        assert!(last.top() >= 0.0 && last.bottom() <= size.y && last.right() <= size.x);
        h.get_by_label("12.5%").click();
        h.run();
        assert_eq!(h.state().canvas_rect().unwrap().width(), 125.0);
        assert_eq!(
            h.get_by_label("Zoom percentage")
                .accesskit_node()
                .value()
                .as_deref(),
            Some("12.5%")
        );
        h.get_by_label("Zoom presets").click();
        h.run();
        h.key_press(Key::Escape);
        h.run();
        assert!(h.query_by_role_and_label(Role::Button, "12.5%").is_none());
        assert_eq!(h.state().canvas_rect().unwrap().width(), 125.0);
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn escape_discards_zoom_drafts_before_reaching_unpublished_canvas_text() {
    for ppp in [1.0, 2.0] {
        let mut h = harness_with(App::new(None), ppp, true);
        h.get_by_label("Text").click();
        h.run();
        let point = h.state().canvas_rect().unwrap().min + Vec2::splat(50.5);
        h.input_mut()
            .events
            .extend([Event::PointerMoved(point), press(point), release(point)]);
        h.run();
        h.event(Event::Text("Keep me".into()));
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for (draft, expected) in [("250%", "100%"), ("oops", "100%")] {
            h.get_by_label("Zoom percentage").focus();
            h.run();
            h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
            h.run();
            h.get_by_label("Zoom percentage").type_text(draft);
            h.run();
            h.key_press(Key::Escape);
            h.run();
            assert_eq!(
                h.get_by_label("Zoom percentage")
                    .accesskit_node()
                    .value()
                    .as_deref(),
                Some(expected),
                "Escape must discard the percentage draft"
            );
            assert_eq!(h.state().canvas_rect().unwrap().width(), 1000.0);
            assert!(
                matches!(h.state().editor().preview(), Some(paintcraft_tools::Preview::Text { text, .. }) if text == "Keep me")
            );
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
        }
        h.get_by_label("Zoom percentage").focus();
        h.run();
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
        h.run();
        h.get_by_label("Zoom percentage").type_text("250%");
        h.run();
        h.key_press(Key::Tab);
        h.run();
        assert_eq!(
            h.state().canvas_rect().unwrap().width(),
            2500.0,
            "Tab still applies a valid draft"
        );
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
    }
}

#[test]
fn joined_zoom_selector_and_logarithmic_thumb_preserve_layout_and_history() {
    use eframe::egui::{Role, epaint::Shape};
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("classic-zoom-surfaces");
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
        let field = h.get_by_label("Zoom percentage").rect();
        let presets = h.get_by_label("Zoom presets").rect();
        assert!(
            (presets.left() - field.right()).abs() < 0.1,
            "joined selector: {field:?} / {presets:?}"
        );
        assert_eq!(field.height(), presets.height());
        assert_eq!(field.height(), 30.0);
        for percent in [12.5, 100.0, 500.0, 1000.0] {
            h.get_by_label("Canvas zoom").set_value_accesskit(percent);
            h.run();
            assert!((h.state().canvas_rect().unwrap().width() - percent as f32 * 10.0).abs() < 0.1);
            let rail = h.get_by_role_and_label(Role::Slider, "Canvas zoom").rect();
            let circles: Vec<_> = h
                .output()
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    Shape::Circle(circle) if rail.contains(circle.center) => Some(circle),
                    _ => None,
                })
                .collect();
            let shell = circles.iter().find(|circle| circle.radius > 10.0).unwrap();
            let center = circles
                .iter()
                .find(|circle| (5.0..=7.0).contains(&circle.radius))
                .unwrap();
            assert!(
                shell.center.distance(center.center) < 0.01,
                "logarithmic center must follow egui at {percent}%"
            );
            assert!(center.fill.b() > center.fill.r());
            if percent > 12.5 {
                let fill = h
                    .output()
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        Shape::Rect(rect)
                            if rail.contains_rect(rect.rect)
                                && (rect.rect.height() - 4.0).abs() < 0.01
                                && rect.fill == center.fill =>
                        {
                            Some(rect.rect)
                        }
                        _ => None,
                    })
                    .expect("blue rail fill before the thumb");
                // egui extends the rounded fill underneath the thumb.
                assert!(
                    (center.center.x - 0.01..=center.center.x + center.radius)
                        .contains(&fill.right())
                );
                assert!((fill.left() - rail.left()).abs() < 0.01);
            }
            assert_eq!(h.get_by_label("Zoom percentage").rect(), field);
        }
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn zoom_presets_open_near_the_current_value_with_left_aligned_labels() {
    use eframe::egui::{Role, epaint::Shape};
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(480.0, 360.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("zoom-popup-opening");
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
        for (value, nearest) in [("73%", "75%"), ("1000%", "1000%"), ("12.5%", "12.5%")] {
            enter_percent(&mut h, value);
            let canvas = h.state().canvas_rect().unwrap();
            h.get_by_label("Zoom presets").click();
            h.run();
            let row = h.get_by_role_and_label(Role::Button, nearest).rect();
            let painted = h
                .output()
                .shapes
                .iter()
                .find_map(|shape| {
                    if let Shape::Text(text) = &shape.shape
                        && text.galley.job.text == nearest
                        && row.contains(text.pos)
                    {
                        Some((shape.clip_rect, text))
                    } else {
                        None
                    }
                })
                .expect("preset label is painted");
            assert!(
                painted.0.contains_rect(row),
                "{value}: current row {row:?} outside {:?}",
                painted.0
            );
            let inset = painted.1.pos.x - row.left();
            assert!(
                (10.0..=18.0).contains(&inset),
                "left-aligned row inset: {inset}"
            );
            assert_eq!(row.height(), 40.0);
            if value == "73%" {
                h.get_by_label("1000%").scroll_to_me();
                h.run();
                let first = h.get_by_role_and_label(Role::Button, "1000%").rect();
                assert!(first.top() >= 0.0 && first.bottom() <= size.y);
                h.key_press(Key::Escape);
                h.run();
                h.get_by_label("Zoom presets").click();
                h.run();
                let nearest = h.get_by_role_and_label(Role::Button, nearest).rect();
                assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
                    Shape::Text(text) if text.galley.job.text == "75%"
                        && nearest.contains(text.pos) && shape.clip_rect.contains_rect(nearest))));
            }
            // Opening and scrolling are view-only. Escape retains the typed zoom.
            h.key_press(Key::Escape);
            h.run();
            assert_eq!(h.state().canvas_rect().unwrap(), canvas);
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
        }
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit GPU zoom popup review"]
fn export_zoom_popup_opening() {
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    let output = std::env::temp_dir().join("paintcraft-zoom-popup-opening");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        for (size, name) in [
            (Vec2::new(1360.0, 900.0), "desktop"),
            (Vec2::new(480.0, 360.0), "compact"),
        ] {
            let dir = scratch("zoom-popup-review");
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
            for value in ["73", "1000", "12.5"] {
                enter_percent(&mut h, value);
                h.get_by_label("Zoom presets").click();
                h.run();
                h.render()
                    .unwrap()
                    .save(output.join(format!("{theme}-{name}-{value}.png")))
                    .unwrap();
                h.key_press(Key::Escape);
                h.run();
                assert!(h.state().editor().document().same_semantics(&snapshot));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
            }
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
