use crate::WebApp;
use eframe::egui::{self, Event, Modifiers, PointerButton, Role, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::Rgba;

fn harness() -> Harness<'static, WebApp> {
    let mut h = Harness::builder()
        .with_max_steps(12)
        .with_size(Vec2::new(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h
}
fn pointer(h: &mut Harness<WebApp>, pos: egui::Pos2, pressed: bool) {
    h.event(Event::PointerMoved(pos));
    h.event(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    });
    h.step();
}

#[test]
fn floating_slider_drags_change_preferences_without_starting_canvas_strokes() {
    let mut h = harness();
    let before = h.state().editor.session.ticket();
    let canvas = h.state().canvas.rect.unwrap();
    for label in ["Tool size", "Brush opacity"] {
        let rect = h.get_by_role_and_label(Role::Slider, label).rect();
        assert!(rect.height() > rect.width() * 3.0);
        assert!(rect.right() < canvas.left());
        let a = rect.center_bottom() - Vec2::new(0.0, rect.height() * 0.2);
        let b = rect.center_top() + Vec2::new(0.0, rect.height() * 0.2);
        pointer(&mut h, a, true);
        h.event(Event::PointerMoved(b));
        h.step();
        pointer(&mut h, b, false);
        h.run();
        assert_eq!(h.state().editor.session.ticket(), before);
        assert!(!h.state().editor.tools.gesture_in_progress());
        assert_eq!(h.state().canvas.rect, Some(canvas));
    }
    assert!(h.state().controls.options.size > 4);
    assert!(h.state().controls.options.brush_opacity < 100);
}

#[test]
fn brush_captures_ui_size_and_opacity_at_press_and_releases_exact_pixels() {
    let mut h = harness();
    h.get_by_role_and_label(Role::SpinButton, "Size")
        .set_value_accesskit(12.0);
    h.run();
    h.get_by_role_and_label(Role::SpinButton, "Opacity")
        .set_value_accesskit(50.0);
    h.run();
    let p = h.state().canvas.rect.unwrap().min + Vec2::splat(50.5);
    pointer(&mut h, p, true);
    h.state_mut().controls.options.size = 28;
    h.state_mut().controls.options.brush_opacity = 20;
    h.run();
    assert_eq!(h.state().editor.session.undo_depth(), 0);
    pointer(&mut h, p, false);
    h.run();
    let layer = h.state().editor.target.unwrap();
    let raster = &h.state().editor.session.document().layers[&layer].raster;
    assert_eq!(raster.get(50, 50), Rgba([127, 127, 127, 255]));
    assert_eq!(raster.get(60, 50), Rgba([255; 4]));
    assert_eq!(h.state().editor.session.undo_depth(), 1);
}

#[test]
fn pencil_size_is_independent_and_control_scroll_does_not_zoom_the_canvas() {
    let mut h = harness();
    h.get_by_role_and_label(Role::Button, "Pencil").click();
    h.run();
    h.get_by_role_and_label(Role::SpinButton, "Size")
        .set_value_accesskit(5.0);
    h.run();
    assert_eq!(h.state().controls.options.pencil_size, 5);
    assert_eq!(h.state().controls.options.size, 4);
    let p = h
        .get_by_role_and_label(Role::Slider, "Tool size")
        .rect()
        .center();
    h.event(Event::PointerMoved(p));
    h.event(Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: Vec2::new(0.0, 1.0),
        modifiers: Modifiers::COMMAND,
        source: egui::MouseWheelSource::Unknown,
        phase: egui::TouchPhase::Move,
    });
    h.run();
    assert_eq!(h.state().zoom, 1.0);
    assert_eq!(h.ctx.zoom_factor(), 1.0);
    assert_eq!(h.state().editor.session.undo_depth(), 0);
}
