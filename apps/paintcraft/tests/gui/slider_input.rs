//! Real Classic slider input, including multiple events and layout passes.
use super::*;
use eframe::egui::Role;

#[test]
fn classic_tool_sliders_accept_coalesced_rail_clicks_without_painting() {
    for ppp in [1.0, 2.0] {
        let mut h = harness_with(App::new(None), ppp, true);
        h.get_by_label("Brush").click();
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for label in ["Tool size", "Brush opacity"] {
            let point = h.get_by_role_and_label(Role::Slider, label).rect().center();
            h.input_mut()
                .events
                .extend([Event::PointerMoved(point), press(point), release(point)]);
            h.run();
        }
        assert!(
            h.state().editor().options.size > 4,
            "a quick rail click must change size"
        );
        assert_eq!(h.state().editor().options.brush_opacity, 50);
        assert!(!h.state().editor().gesture_in_progress());
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
    }
}

#[test]
fn classic_zoom_slider_accepts_a_coalesced_endpoint_click_without_history() {
    for ppp in [1.0, 2.0] {
        let mut h = harness_with(App::new(None), ppp, true);
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let rail = h.get_by_label("Canvas zoom").rect();
        for (x, width) in [
            (rail.center().x, 1118.0),
            (rail.right() - 0.5, 10000.0),
            (rail.left() + 0.5, 125.0),
        ] {
            let point = Pos2::new(x, rail.center().y);
            h.input_mut()
                .events
                .extend([Event::PointerMoved(point), press(point), release(point)]);
            h.run();
            assert!((h.state().canvas_rect().unwrap().width() * ppp - width).abs() < 0.1);
        }
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
    }
}

#[test]
fn classic_slider_keyboard_steps_match_single_and_repeated_layouts() {
    for ppp in [1.0, 2.0] {
        let mut results = Vec::new();
        for two_passes in [false, true] {
            let mut h = harness_with(App::new(None), ppp, two_passes);
            h.get_by_label("Brush").click();
            h.run();
            let snapshot = h.state().editor().document_snapshot();
            let ticket = h.state().editor().history_view().ticket;
            for (label, value, key) in [
                ("Tool size", 20.0, Key::ArrowUp),
                ("Brush opacity", 50.0, Key::ArrowUp),
                ("Canvas zoom", 100.0, Key::ArrowRight),
            ] {
                h.get_by_role_and_label(Role::Slider, label)
                    .set_value_accesskit(value);
                h.run();
                h.get_by_role_and_label(Role::Slider, label).focus();
                h.run();
                h.key_press(key);
                h.run();
            }
            results.push((
                h.state().editor().options.size,
                h.state().editor().options.brush_opacity,
                h.state().canvas_rect().unwrap().width() * ppp,
            ));
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
        }
        assert_eq!(results[0].0, 21);
        assert_eq!(results[0].1, 51);
        assert_eq!(results[0].0, results[1].0, "size must step once");
        assert_eq!(results[0].1, results[1].1, "opacity must step once");
        assert!(
            (results[0].2 - results[1].2).abs() < 0.1,
            "zoom must step once: {results:?}"
        );
    }
}

#[test]
#[ignore = "requires GPU; exports Classic sliders after coalesced taps"]
fn export_classic_slider_taps() {
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    let output = std::env::temp_dir().join("paintcraft-slider-taps");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        let dir = scratch("classic-slider-tap-review");
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
        for label in ["Tool size", "Brush opacity"] {
            let p = h.get_by_role_and_label(Role::Slider, label).rect().center();
            h.input_mut()
                .events
                .extend([Event::PointerMoved(p), press(p), release(p)]);
            h.run();
        }
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-tool-taps.png")))
            .unwrap();
        let rail = h.get_by_label("Canvas zoom").rect();
        for (x, name) in [
            (rail.center().x, "zoom-middle"),
            (rail.left() + 0.5, "zoom-minimum"),
        ] {
            let p = Pos2::new(x, rail.center().y);
            h.input_mut()
                .events
                .extend([Event::PointerMoved(p), press(p), release(p)]);
            h.run();
            h.render()
                .unwrap()
                .save(output.join(format!("{theme}-{name}.png")))
                .unwrap();
        }
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
