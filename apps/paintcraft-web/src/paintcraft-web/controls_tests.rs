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
    let p = h.state().canvas.rect.unwrap().min + Vec2::splat(50.5) * h.state().zoom;
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
    let zoom = h.state().zoom;
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
    assert_eq!(h.state().zoom, zoom);
    assert_eq!(h.ctx.zoom_factor(), 1.0);
    assert_eq!(h.state().editor.session.undo_depth(), 0);
}

#[test]
fn selection_brush_previews_live_without_polygon_fill_and_escape_preserves_the_selection() {
    use paintcraft_core::types::selection::{Combine, Shape};
    use paintcraft_raster::Selection;
    let mut h = harness();
    h.state_mut().controls.tool = paintcraft_tools::Tool::Select;
    h.state_mut().controls.options.selection_mode = Shape::Brush;
    h.state_mut().controls.options.selection.diameter = 24;
    h.state_mut().controls.options.selection.hardness = 50;
    h.state_mut().controls.options.selection.strength = 60;
    h.run();
    let canvas = h.state().canvas.rect.unwrap();
    let scale = h.state().zoom;
    let position = |x, y| canvas.min + Vec2::new(x, y) * scale;
    let start = position(50.5, 80.5);
    let before = h.state().editor.session.ticket();
    pointer(&mut h, start, true);
    h.event(Event::PointerMoved(position(170.5, 80.5)));
    h.event(Event::PointerMoved(position(170.5, 180.5)));
    h.run();
    let draft = h
        .state()
        .canvas
        .selection_brush
        .as_ref()
        .expect("live brush draft");
    let exact = draft.selection.clone();
    let Selection::Mask(mask) = &exact else {
        panic!()
    };
    assert!(mask.coverage(170, 170) > 0);
    assert_eq!(mask.coverage(100, 130), 0, "no polygon closure");

    assert_eq!(h.state().editor.session.ticket(), before);
    assert_eq!(h.state().editor.session.selection(), Selection::Inactive);
    pointer(&mut h, position(170.5, 180.5), false);
    h.run();
    assert!(
        h.state()
            .editor
            .session
            .selection()
            .same_field_checked(&exact, 2_000_000, &|| Ok(()))
            .unwrap()
    );
    assert_eq!(h.state().controls.options.selection.combine, Combine::Add);
    let committed = h.state().editor.session.selection();
    let ticket = h.state().editor.session.ticket();
    pointer(&mut h, position(250.5, 180.5), true);
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.state().canvas.selection_brush.is_none());
    assert_eq!(h.state().editor.session.selection(), committed);
    assert_eq!(h.state().editor.session.ticket(), ticket);
}
