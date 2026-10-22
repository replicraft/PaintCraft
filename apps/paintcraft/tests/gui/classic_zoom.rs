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
        assert!((h.state().canvas_rect().unwrap().width() * ppp - 730.0).abs() < 0.1);
        enter_percent(&mut h, "oops");
        assert!((h.state().canvas_rect().unwrap().width() * ppp - 730.0).abs() < 0.1);
        h.get_by_label("Zoom presets").click();
        h.run();
        h.get_by_label("12.5%").scroll_to_me();
        h.run();
        h.get_by_label("12.5%").click();
        h.run();
        assert_eq!(h.state().canvas_rect().unwrap().width() * ppp, 125.0);
        h.event(key(Key::Num0, Modifiers::COMMAND));
        h.run();
        assert_eq!(h.state().canvas_rect().unwrap().width() * ppp, 1000.0);
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
        let mut previous_bottom = 0.0;
        for label in [
            "1000%", "900%", "800%", "700%", "600%", "500%", "400%", "300%", "200%", "100%", "75%",
            "50%", "25%", "12.5%",
        ] {
            let rect = h.get_by_role_and_label(Role::Button, label).rect();
            assert_eq!(rect.height(), 40.0);
            assert!(rect.width() >= 100.0);
            assert!(rect.top() >= previous_bottom, "descending order: {label}");
            previous_bottom = rect.bottom();
        }
        h.get_by_label("1000%").scroll_to_me();
        h.run();
        let first = h.get_by_label("1000%").rect();
        assert!(first.top() >= 0.0 && first.bottom() <= size.y);
        h.get_by_label("1000%").click();
        h.run();
        assert_eq!(h.state().canvas_rect().unwrap().width() * ppp, 10000.0);
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
        assert_eq!(h.state().canvas_rect().unwrap().width() * ppp, 125.0);
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
        assert_eq!(h.state().canvas_rect().unwrap().width() * ppp, 125.0);
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
