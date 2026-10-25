use super::*;
use paintcraft::settings::{ClassicVariant, Experience, Settings};
fn classic() -> Harness<'static, App> {
    let dir = scratch(&format!("interactions-{}", uuid::Uuid::new_v4()));
    Settings {
        classic_variant: Some(ClassicVariant::V11),
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    harness_with(App::new(None).with_settings(dir), 1.0, true)
}
#[test]
fn ctrl_and_command_wheel_zoom_the_canvas_once_without_changing_ui_scale_or_document() {
    for modifiers in [Modifiers::CTRL, Modifiers::COMMAND] {
        let mut h = classic();
        let before = h.state().editor().document_snapshot();
        let width = h.state().canvas_rect().unwrap().width();
        let p = at(&h, 20.0, 20.0);
        let ppp = h.ctx.pixels_per_point();
        h.input_mut().events.extend([
            Event::PointerMoved(p),
            Event::MouseWheel {
                unit: eframe::egui::MouseWheelUnit::Line,
                delta: Vec2::new(0.0, 1.0),
                phase: eframe::egui::TouchPhase::Move,
                source: eframe::egui::MouseWheelSource::Unknown,
                modifiers,
            },
        ]);
        h.run();
        assert!((h.state().canvas_rect().unwrap().width() / width - 1.1).abs() < 0.001);
        assert_eq!(h.ctx.pixels_per_point(), ppp);
        assert!(h.state().editor().document().same_semantics(&before));
    }
}
#[test]
fn pencil_size_has_an_easy_slider_and_is_captured_for_the_whole_stroke() {
    let mut h = classic();
    with_layer(&mut h);
    h.get_by_label("Pencil").click();
    h.run();
    h.get_by_label("Tool size").set_value_accesskit(12.0);
    h.run();
    let a = at(&h, 50.0, 50.0);
    let b = at(&h, 80.0, 50.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(a), press(a), Event::PointerMoved(b)]);
    h.run();
    assert_eq!(h.state().editor().options.pencil_size, 12);
    let preview = h.state().rendered().to_vec();
    h.event(release(b));
    h.run();
    assert_eq!(h.state().rendered(), preview);
    assert_eq!(top_pixel(&h, 65, 54).a(), 255);
    assert_eq!(top_pixel(&h, 65, 60).a(), 0);
    h.get_by_label("Undo").click();
    h.run();
    assert_eq!(top_pixel(&h, 65, 50).a(), 0);
}
#[test]
fn changer_is_next_to_settings_and_discard_drops_changed_documents_before_switching() {
    let mut h = classic();
    with_layer(&mut h);
    let old = h.state().editor().document().document_id;
    h.get_by_role_and_label(eframe::egui::Role::Button, "Interface")
        .click();
    h.run();
    h.get_by_label("Studio").click();
    h.run();
    h.get_by_label("Discard and switch").click();
    h.run();
    assert_eq!(h.state().experience(), Experience::Studio);
    assert_ne!(h.state().editor().document().document_id, old);
    h.get_by_role_and_label(eframe::egui::Role::Button, "Interface")
        .click();
    h.run();
    h.get_by_label("Classic").click();
    h.run();
    assert_eq!(h.state().experience(), Experience::Classic);
    assert_eq!(
        h.state().document_count(),
        0,
        "discarded documents do not return with unsaved pixels"
    );
}

#[test]
fn classic_floating_size_and_opacity_are_vertical_and_pointer_drags_do_not_paint() {
    let mut h = classic();
    with_layer(&mut h);
    h.get_by_label("Brush").click();
    h.run();
    let snapshot = h.state().editor().document_snapshot();
    let before = publications(&h);
    for label in ["Tool size", "Brush opacity"] {
        let rect = h
            .get_by_role_and_label(eframe::egui::Role::Slider, label)
            .rect();
        assert!(rect.height() > rect.width() * 3.0, "{label}: {rect:?}");
        let start = rect.center_bottom() - Vec2::new(0.0, rect.height() * 0.2);
        let end = rect.center_top() + Vec2::new(0.0, rect.height() * 0.2);
        h.input_mut().events.extend([
            Event::PointerMoved(start),
            press(start),
            Event::PointerMoved(end),
        ]);
        h.run();
        h.event(release(end));
        h.run();
        assert_eq!(
            publications(&h),
            before,
            "{label} must not start a canvas gesture"
        );
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert!(!h.state().editor().has_pending_edits());
    }
    assert!(h.state().editor().options.size > 4);
    assert!(h.state().editor().options.brush_opacity < 100);
}

#[test]
fn classic_tool_panels_fit_short_windows_and_zoom_fit_keeps_the_canvas_clear() {
    let mut h = classic();
    h.get_by_label("Brush").click();
    h.run();
    let before_size_change = h.state().canvas_rect().unwrap();
    h.get_by_role_and_label(eframe::egui::Role::SpinButton, "Size")
        .set_value_accesskit(1024.0);
    h.run();
    assert_eq!(h.state().editor().options.size, 1024);
    assert_eq!(h.state().canvas_rect().unwrap(), before_size_change);
    assert!(
        before_size_change.left()
            > h.get_by_role_and_label(eframe::egui::Role::SpinButton, "Opacity")
                .rect()
                .right()
                + 6.0
    );
    h.get_by_label("Zoom to window").click();
    h.run();
    let canvas = h.state().canvas_rect().unwrap();
    let opacity = h
        .get_by_role_and_label(eframe::egui::Role::Slider, "Brush opacity")
        .rect();
    assert!(canvas.left() > opacity.right());
    assert!(
        canvas.left()
            > h.get_by_role_and_label(eframe::egui::Role::SpinButton, "Opacity")
                .rect()
                .right()
                + 6.0
    );
    assert!(
        canvas.right() <= 1100.0 && canvas.bottom() < 880.0,
        "{canvas:?}"
    );
    h.set_size(Vec2::new(1100.0, 500.0));
    h.run();
    for label in ["Tool size", "Brush opacity"] {
        let rect = h
            .get_by_role_and_label(eframe::egui::Role::Slider, label)
            .rect();
        assert!(
            rect.top() >= 0.0 && rect.bottom() < 470.0,
            "{label}: {rect:?}"
        );
    }
}
