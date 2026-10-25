use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

#[test]
fn classic_status_fields_and_zoom_stay_reachable_without_changing_document_state() {
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(700.0, 500.0), 2.0, Appearance::Dark),
        (Vec2::new(480.0, 360.0), 1.0, Appearance::Light),
        (Vec2::new(360.0, 360.0), 2.0, Appearance::Dark),
    ] {
        let dir = scratch("classic-status-layout");
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
                move |ui, app: &mut App| {
                    app.ui(ui);
                    if size.x != 1360.0 && ui.ctx().current_pass_index() == 0 {
                        ui.ctx().request_discard("test: repeated status layout");
                    }
                },
                App::new(None)
                    .with_settings(dir.clone())
                    .with_initial_zoom(1.0),
            );
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let zoom = h.get_by_label("Zoom percentage").rect();
        let canvas = h
            .get_by_role_and_label(Role::Label, "Canvas size: 1000 × 486 px")
            .rect();
        for label in [
            "Zoom to window",
            "Zoom percentage",
            "Zoom presets",
            "Zoom out",
            "Canvas zoom",
            "Zoom in",
        ] {
            let rect = h.get_by_label(label).rect();
            assert!(
                rect.min.x >= 0.0
                    && rect.min.y >= 0.0
                    && rect.max.x <= size.x
                    && rect.max.y <= size.y,
                "{label}: {rect:?} outside {size:?}"
            );
        }
        assert!(
            h.get_by_label("Zoom in").rect().right() > size.x - 50.0,
            "zoom stays at the right edge"
        );
        if size.x < 860.0 {
            assert!(canvas.bottom() <= zoom.top());
        } else {
            assert!(
                (canvas.center().y - zoom.center().y).abs() < 4.0,
                "canvas={canvas:?}, zoom={zoom:?}"
            );
        }
        h.run_steps(8);
        assert_eq!(h.get_by_label("Zoom percentage").rect(), zoom);
        // A pointer move updates its coordinates even without another input event.
        let point = h.state().canvas_rect().unwrap().min + Vec2::new(20.5, 10.5);
        h.event(Event::PointerMoved(point));
        h.step();
        assert!(h.query_by_label("Cursor position: 20, 10 px").is_some());
        h.run();
        assert_eq!(h.get_by_label("Zoom percentage").rect(), zoom);
        h.get_by_label("Zoom in").click();
        h.run();
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(h.get_by_label("Zoom percentage").rect(), zoom);
        h.event(Event::PointerMoved(Pos2::new(4.0, 4.0)));
        h.run();
        assert!(
            h.query_by_label("Cursor position: Outside canvas")
                .is_some()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn classic_status_selection_dimensions_follow_real_selection_without_moving_zoom() {
    let mut h = harness();
    let zoom = h.get_by_label("Zoom percentage").rect();
    h.get_by_label("Select").click();
    h.run();
    drag(&mut h, (10.0, 15.0), (20.0, 20.0));
    assert!(h.query_by_label("Selection size: 11 × 6 px").is_some());
    assert_eq!(h.get_by_label("Zoom percentage").rect(), zoom);
    let before = publications(&h);
    h.get_by_role_and_label(Role::Label, "Selection size: 11 × 6 px")
        .hover();
    h.run();
    assert_eq!(publications(&h), before);
    h.event(key(Key::D, Modifiers::COMMAND));
    h.run();
    assert!(
        h.query_all_by_role_and_label(Role::Label, "Selection size: No selection")
            .next()
            .is_some()
    );
    assert_eq!(h.get_by_label("Zoom percentage").rect(), zoom);
}

#[test]
#[ignore = "requires GPU; exports Classic footer glyphs and zoom hover"]
fn export_classic_footer_artwork() {
    let output = std::env::temp_dir().join("paintcraft-footer-artwork");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        let dir = scratch("footer-artwork-review");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(
            App::new(None)
                .with_settings(dir.clone())
                .with_initial_zoom(1.0),
            1.0,
            false,
        );
        h.set_size(Vec2::new(1360.0, 900.0));
        h.run();
        h.get_by_label("Select").click();
        h.run();
        drag(&mut h, (10.0, 15.0), (20.0, 20.0));
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-desktop-metrics.png")))
            .unwrap();
        h.get_by_label("Zoom out").hover();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-zoom-hover.png")))
            .unwrap();
        h.set_size(Vec2::new(480.0, 360.0));
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-compact.png")))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
