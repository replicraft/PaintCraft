use super::*;

#[test]
fn polygon_last_vertex_and_enter_are_ordered_in_one_multipass_frame() {
    let mut h = harness_with(App::new(None), 1.0, true);
    h.get_by_label("Polygon").click();
    h.run();
    let base = h.state().editor().document_snapshot();
    let a = at(&h, 20.0, 20.0);
    let b = at(&h, 90.0, 20.0);
    h.input_mut().events.extend([
        Event::PointerMoved(a),
        press(a),
        Event::PointerMoved(b),
        release(b),
    ]);
    h.run();
    assert!(h.state().editor().is_constructing_shape());
    assert_eq!(h.state().editor().publications(), 0);
    let c = at(&h, 90.0, 90.0);
    h.input_mut().events.extend([
        Event::PointerMoved(c),
        press(c),
        release(c),
        Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        },
    ]);
    h.run();
    assert!(!h.state().editor().gesture_in_progress());
    assert_eq!(h.state().editor().publications(), 1);
    assert_ne!(top_pixel(&h, 60, 60), Rgba([255; 4]));
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&base));
}

#[test]
fn curve_gallery_retains_two_handles_then_commits_once_and_undo_restores() {
    let mut h = harness_with(App::new(None), 1.0, true);
    h.get_by_label("Curve").click();
    h.run();
    let base = h.state().editor().document_snapshot();
    for (index, (a, b)) in [
        ((20.0, 20.0), (90.0, 20.0)),
        ((20.0, 100.0), (20.0, 100.0)),
        ((90.0, 100.0), (90.0, 100.0)),
    ]
    .into_iter()
    .enumerate()
    {
        let a = at(&h, a.0, a.1);
        let b = at(&h, b.0, b.1);
        h.input_mut().events.extend([
            Event::PointerMoved(a),
            press(a),
            Event::PointerMoved(b),
            release(b),
        ]);
        h.run();
        assert_eq!(h.state().editor().publications(), u64::from(index == 2));
    }
    assert!(!h.state().editor().gesture_in_progress());
    assert_ne!(top_pixel(&h, 55, 80), Rgba([255; 4]));
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&base));
}

#[test]
fn classic_shape_gallery_selects_real_presets_and_publishes_one_undoable_edit() {
    use paintcraft_tools::{ShapePreset, Tool};
    let mut h = harness();
    for preset in [
        ShapePreset::Triangle,
        ShapePreset::Diamond,
        ShapePreset::Hexagon,
    ] {
        h.get_by_label(preset.name()).click();
        h.run();
        assert_eq!(h.state().editor().tool, Tool::Preset(preset));
        let before = h.state().editor().document_snapshot();
        let a = at(&h, 20.0, 20.0);
        let b = at(&h, 80.0, 70.0);
        h.input_mut().events.extend([
            Event::PointerMoved(a),
            press(a),
            Event::PointerMoved(b),
            release(b),
        ]);
        h.run();
        assert!(!h.state().editor().document().same_semantics(&before));
        h.get_by_label("Undo").click();
        h.run();
        assert!(h.state().editor().document().same_semantics(&before));
    }
}

#[test]
fn shift_changes_shape_preview_without_pointer_motion_and_constrains_publication() {
    let mut h = harness_with(App::new(None), 1.0, true);
    h.get_by_label("Rectangle").click();
    h.run();
    let base = h.state().editor().document_snapshot();
    let a = at(&h, 50.0, 50.0);
    let b = at(&h, 80.0, 60.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(a), press(a), Event::PointerMoved(b)]);
    h.run();
    h.event(Event::ModifiersChanged(Modifiers::SHIFT));
    h.run();
    assert!(matches!(
        h.state().editor().preview(),
        Some(paintcraft::controller::Preview::Shape { to: (80, 80), .. })
    ));
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.run();
    assert!(matches!(
        h.state().editor().preview(),
        Some(paintcraft::controller::Preview::Shape { to: (80, 60), .. })
    ));
    h.event(Event::ModifiersChanged(Modifiers::SHIFT));
    h.event(Event::PointerButton {
        pos: b,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::SHIFT,
    });
    h.run();
    assert_eq!(top_pixel(&h, 80, 80), Rgba([0, 0, 0, 255]));
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&base));
}

#[test]
fn eraser_hides_system_cursor_only_over_the_canvas_and_overlay_does_not_edit_pixels() {
    let mut h = harness();
    h.get_by_label("Eraser").click();
    h.run();
    let base = h.state().editor().document_snapshot();
    let point = at(&h, 100.0, 100.0);
    h.event(Event::PointerMoved(point));
    h.run();
    assert_eq!(
        h.output().platform_output.cursor_icon,
        eframe::egui::CursorIcon::None
    );
    assert!(h.state().editor().document().same_semantics(&base));
    h.get_by_label("Settings…").hover();
    h.run();
    assert_ne!(
        h.output().platform_output.cursor_icon,
        eframe::egui::CursorIcon::None
    );
}
