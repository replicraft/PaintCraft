//! Actual host acquisition ordering, ownership, and cancellation controls.
use super::*;

fn acquired_mouse(h: &mut Host, at: egui::Pos2, pressed: bool) {
    let source = h.app.pen.source.clone().unwrap();
    let intake = h.app.pen.acquisition.clone().unwrap();
    let mut intake = intake.lock().unwrap();
    let previous = intake.buttons();
    assert!(
        intake
            .button_changes(
                &source,
                previous,
                u8::from(pressed),
                at,
                egui::Modifiers::default()
            )
            .unwrap()
    );
}
fn acquired_pen(
    h: &mut Host,
    phase: Phase,
    time: u64,
) -> paintcraft_input::tablet::receiver::Route {
    acquired_pen_as(h, phase, time, DeviceKind::Pen)
}
fn acquired_pen_as(
    h: &mut Host,
    phase: Phase,
    time: u64,
    kind: DeviceKind,
) -> paintcraft_input::tablet::receiver::Route {
    let mut sample = raw(
        h,
        if phase == Phase::Down { 1 } else { 2 },
        phase,
        0.7,
        if phase == Phase::Up { 0 } else { Buttons::TIP },
    );
    sample.time.micros = time;
    sample.identity.kind = kind;
    h.app
        .pen
        .acquisition
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .pen(&mut sample)
        .unwrap();
    h.app
        .pen
        .source
        .as_ref()
        .unwrap()
        .submit_with(
            paintcraft_input::tablet::receiver::Delivery::ReceiverPointer,
            |_| Ok(sample),
        )
        .unwrap()
}

#[test]
fn earlier_mouse_down_wins_without_an_intervening_gui_frame() {
    let mut h = harness();
    let rect = h.app.canvas.rect.unwrap();
    let at = rect.min + egui::vec2(100.0, 100.0) * h.app.zoom;
    acquired_mouse(&mut h, at, true);
    assert_eq!(
        acquired_pen(&mut h, Phase::Down, 100),
        paintcraft_input::tablet::receiver::Route::Suppressed
    );
    h.step();
    assert!(!h.app.editor.tools.pen_in_progress());
    acquired_mouse(&mut h, at + egui::vec2(40.0, 0.0), false);
    h.step();
    assert_eq!(h.app.editor.session.undo_depth(), 1);
    assert!(!h.app.editor.tools.pointer_is_down());
}

#[test]
fn earlier_actual_mouse_up_then_pen_down_preserve_two_transactions_and_undo_order() {
    let mut h = harness();
    h.app.controls.primary = paintcraft_core::types::Rgba([20, 30, 40, 255]);
    h.run();
    let baseline = paintcraft_render::render_flat(h.app.editor.session.document()).unwrap();
    let rect = h.app.canvas.rect.unwrap();
    let at = rect.min + egui::vec2(100.0, 100.0) * h.app.zoom;
    acquired_mouse(&mut h, at, true);
    h.step();
    acquired_mouse(&mut h, at + egui::vec2(40.0, 0.0), false);
    assert_eq!(
        acquired_pen(&mut h, Phase::Down, 100),
        paintcraft_input::tablet::receiver::Route::Canvas
    );
    h.step();
    assert_eq!(h.app.editor.session.undo_depth(), 1);
    assert!(h.app.editor.tools.pen_in_progress());
    let mouse_only = paintcraft_render::render_flat(h.app.editor.session.document()).unwrap();
    assert_ne!(mouse_only, baseline);
    assert_eq!(
        acquired_pen(&mut h, Phase::Up, 200),
        paintcraft_input::tablet::receiver::Route::Canvas
    );
    h.step();
    assert_eq!(h.app.editor.session.undo_depth(), 2);
    let both = paintcraft_render::render_flat(h.app.editor.session.document()).unwrap();
    assert_ne!(both, mouse_only);
    h.app.editor.session.undo().unwrap();
    assert_eq!(
        paintcraft_render::render_flat(h.app.editor.session.document()).unwrap(),
        mouse_only
    );
    h.app.editor.session.undo().unwrap();
    assert_eq!(
        paintcraft_render::render_flat(h.app.editor.session.document()).unwrap(),
        baseline
    );
}

#[test]
fn actual_mouse_fill_then_eraser_end_pen_keeps_established_fill_edge_and_undo_order() {
    let mut h = harness();
    h.app.controls.tool = paintcraft_tools::Tool::Fill;
    h.app.controls.primary = paintcraft_core::types::Rgba([20, 30, 40, 255]);
    h.run();
    let baseline = paintcraft_render::render_flat(h.app.editor.session.document()).unwrap();
    let at = h.app.canvas.rect.unwrap().min + egui::vec2(100.0, 100.0) * h.app.zoom;
    acquired_mouse(&mut h, at, true);
    h.step();
    // Preserve the established browser Fill contract: capture on Down, commit on Up.
    assert_eq!(h.app.editor.session.undo_depth(), 0);
    acquired_mouse(&mut h, at, false);
    assert_eq!(
        acquired_pen_as(&mut h, Phase::Down, 100, DeviceKind::Eraser),
        paintcraft_input::tablet::receiver::Route::Canvas
    );
    h.step();
    assert_eq!(h.app.editor.session.undo_depth(), 1);
    assert!(h.app.editor.tools.pen_in_progress());
    let filled = paintcraft_render::render_flat(h.app.editor.session.document()).unwrap();
    assert_ne!(filled, baseline);
    assert_eq!(
        acquired_pen_as(&mut h, Phase::Up, 200, DeviceKind::Eraser),
        paintcraft_input::tablet::receiver::Route::Canvas
    );
    h.step();
    assert_eq!(h.app.editor.session.undo_depth(), 2);
    assert_ne!(
        paintcraft_render::render_flat(h.app.editor.session.document()).unwrap(),
        filled
    );
    h.app.editor.session.undo().unwrap();
    assert_eq!(
        paintcraft_render::render_flat(h.app.editor.session.document()).unwrap(),
        filled
    );
    h.app.editor.session.undo().unwrap();
    assert_eq!(
        paintcraft_render::render_flat(h.app.editor.session.document()).unwrap(),
        baseline
    );
}

#[test]
fn acquisition_records_all_physical_button_releases_and_genuine_mouse_motion_once() {
    let h = harness();
    let source = h.app.pen.source.as_ref().unwrap();
    let mut intake = h.app.pen.acquisition.as_ref().unwrap().lock().unwrap();
    let at = h.app.canvas.rect.unwrap().center();
    assert!(
        intake
            .button_changes(source, 0, 0x1f, at, egui::Modifiers::default())
            .unwrap()
    );
    assert!(
        intake
            .mouse(
                source,
                egui::Event::PointerMoved(at + egui::vec2(5.0, 0.0)),
                0x1f,
                egui::Modifiers::default()
            )
            .unwrap()
    );
    for remaining in [0x1e, 0x1c, 0x18, 0x10, 0] {
        let previous = intake.buttons();
        assert!(
            intake
                .button_changes(source, previous, remaining, at, egui::Modifiers::default())
                .unwrap()
        );
    }
    let mut records = Vec::with_capacity(11);
    intake.drain(&mut records);
    assert_eq!(records.len(), 11);
    assert_eq!(
        records
            .iter()
            .filter(|r| matches!(r.event, egui::Event::PointerMoved(_)))
            .count(),
        1
    );
    for button in [
        egui::PointerButton::Primary,
        egui::PointerButton::Secondary,
        egui::PointerButton::Middle,
        egui::PointerButton::Extra1,
        egui::PointerButton::Extra2,
    ] {
        for pressed in [true, false] {
            assert_eq!(records.iter().filter(|r| matches!(r.event,
                egui::Event::PointerButton { button: b, pressed: p, .. } if b == button && p == pressed)).count(), 1);
        }
    }
    assert!(
        records
            .windows(2)
            .all(|pair| pair[0].order <= pair[1].order)
    );
    assert_eq!(intake.buttons(), 0);
}

#[test]
fn two_complete_acquired_pen_contacts_in_one_gui_interval_keep_two_undo_items() {
    let mut h = harness();
    h.app.controls.primary = paintcraft_core::types::Rgba([20, 30, 40, 255]);
    h.app.controls.options.size = 30;
    h.run();
    let baseline = paintcraft_render::render_flat(h.app.editor.session.document()).unwrap();
    for (n, phase, pressure) in [
        (1, Phase::Down, 0.3),
        (2, Phase::Up, 0.3),
        (3, Phase::Down, 0.9),
        (4, Phase::Up, 0.9),
    ] {
        let mut sample = raw(
            &h,
            if phase == Phase::Down { 1 } else { 2 },
            phase,
            pressure,
            if phase == Phase::Up { 0 } else { Buttons::TIP },
        );
        sample.time.micros = n * 100;
        h.app
            .pen
            .acquisition
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .pen(&mut sample)
            .unwrap();
        assert_eq!(
            h.app
                .pen
                .source
                .as_ref()
                .unwrap()
                .submit_with(
                    paintcraft_input::tablet::receiver::Delivery::ReceiverPointer,
                    |_| Ok(sample)
                )
                .unwrap(),
            paintcraft_input::tablet::receiver::Route::Canvas
        );
    }
    h.step();
    assert_eq!(h.app.editor.session.undo_depth(), 2);
    assert!(!h.app.editor.tools.gesture_in_progress());
    let both = paintcraft_render::render_flat(h.app.editor.session.document()).unwrap();
    h.app.editor.session.undo().unwrap();
    let first = paintcraft_render::render_flat(h.app.editor.session.document()).unwrap();
    assert_ne!(both, first);
    assert_ne!(first, baseline);
    h.app.editor.session.undo().unwrap();
    assert_eq!(
        paintcraft_render::render_flat(h.app.editor.session.document()).unwrap(),
        baseline
    );
}

#[test]
fn admitted_pen_continuation_never_rebases_external_publication_or_cancel() {
    for mode in 0..4 {
        let mut h = harness();
        h.app.controls.primary = paintcraft_core::types::Rgba([20, 30, 40, 255]);
        h.run();
        for (n, phase) in [
            (1, Phase::Down),
            (2, Phase::Up),
            (3, Phase::Down),
            (4, Phase::Up),
        ] {
            let mut sample = raw(
                &h,
                if phase == Phase::Down { 1 } else { 2 },
                phase,
                if n < 3 { 0.3 } else { 0.9 },
                if phase == Phase::Up { 0 } else { Buttons::TIP },
            );
            sample.time.micros = n * 100;
            h.app
                .pen
                .acquisition
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .pen(&mut sample)
                .unwrap();
            h.app
                .pen
                .source
                .as_ref()
                .unwrap()
                .submit_with(
                    paintcraft_input::tablet::receiver::Delivery::ReceiverPointer,
                    |_| Ok(sample),
                )
                .unwrap();
        }
        let mut input = egui::RawInput::default();
        h.app.pen_raw_input(&h.ctx, &mut input);
        let _admitted = h.app.pen_canvas_events(Vec::new(), &h.ctx);
        h.app.pen_canvas_sample(0, &h.ctx);
        h.app.pen_canvas_sample(1, &h.ctx);
        assert_eq!(h.app.editor.session.undo_depth(), 1);
        match mode {
            0 => h.app.pen.source.as_ref().unwrap().cancel(),
            1 => {
                use paintcraft_core::types::json::Value;
                let id = h.app.editor.target.unwrap();
                h.app
                    .editor
                    .commit(vec![Value::object([
                        ("op", Value::str("layer.set_properties")),
                        (
                            "layer_ref",
                            Value::object([("id", Value::str(id.to_string()))]),
                        ),
                        ("name", Value::str("Unrelated publication")),
                    ])])
                    .unwrap();
            }
            2 => h.app.zoom *= 2.0,
            _ => h.app.settings = true,
        }
        let before = h.app.editor.session.document().clone();
        let depth = h.app.editor.session.undo_depth();
        h.app.pen_canvas_sample(2, &h.ctx);
        h.app.pen_canvas_sample(3, &h.ctx);
        assert!(
            h.app.editor.session.document().same_semantics(&before),
            "mode {mode}"
        );
        assert_eq!(h.app.editor.session.undo_depth(), depth, "mode {mode}");
        assert!(!h.app.editor.tools.pen_in_progress());
    }
}

#[test]
fn acquisition_overflow_retires_its_actual_legacy_canvas_prefix_without_publishing() {
    let mut h = harness();
    let at = h.app.canvas.rect.unwrap().min + egui::vec2(100.0, 100.0) * h.app.zoom;
    acquired_mouse(&mut h, at, true);
    h.step();
    assert!(h.app.editor.tools.pointer_is_down());
    let source = h.app.pen.source.as_ref().unwrap();
    let mut intake = h.app.pen.acquisition.as_ref().unwrap().lock().unwrap();
    for _ in 0..4096 {
        assert!(
            intake
                .mouse(
                    source,
                    egui::Event::PointerMoved(at),
                    1,
                    egui::Modifiers::default()
                )
                .unwrap()
        );
    }
    assert_eq!(
        intake.mouse(
            source,
            egui::Event::PointerMoved(at),
            1,
            egui::Modifiers::default()
        ),
        Err(Invalid::Capacity)
    );
    drop(intake);
    // Same explicit retirement used by the DOM acquisition failure path.
    source.cancel();
    h.step();
    assert!(!h.app.editor.tools.pointer_is_down());
    assert_eq!(h.app.editor.session.undo_depth(), 0);
    let intake = h.app.pen.acquisition.clone().unwrap();
    assert!(
        !intake
            .lock()
            .unwrap()
            .button_changes(
                &h.app.pen.source.clone().unwrap(),
                1,
                0,
                at,
                egui::Modifiers::default()
            )
            .unwrap()
    );
    h.step();
    assert_eq!(h.app.editor.session.undo_depth(), 0);
}

#[test]
fn competing_mouse_motion_and_all_button_tails_never_cancel_or_steal_held_pen() {
    for ui_only in [false, true] {
        let mut h = harness();
        let source = h.app.pen.source.clone().unwrap();
        let mut down = raw(&h, 1, Phase::Down, 0.5, Buttons::TIP);
        if ui_only {
            down.logical_position = Some((-10.0, -10.0));
        }
        let route = source
            .submit_with(
                paintcraft_input::tablet::receiver::Delivery::ReceiverPointer,
                |_| Ok(down),
            )
            .unwrap();
        assert_eq!(
            route,
            if ui_only {
                paintcraft_input::tablet::receiver::Route::Ui
            } else {
                paintcraft_input::tablet::receiver::Route::Canvas
            }
        );
        h.step();
        let at = h.app.canvas.rect.unwrap().center();
        let intake = h.app.pen.acquisition.clone().unwrap();
        let message = h.app.message.clone();
        {
            let mut intake = intake.lock().unwrap();
            assert!(
                !intake
                    .mouse(
                        &source,
                        egui::Event::PointerMoved(at),
                        0,
                        egui::Modifiers::default()
                    )
                    .unwrap()
            );
            assert!(
                !intake
                    .button_changes(&source, 0, 0x1f, at, egui::Modifiers::default())
                    .unwrap()
            );
        }
        h.step();
        assert_eq!(h.app.editor.tools.pen_in_progress(), !ui_only);
        assert_eq!(h.app.message, message);
        let mut up = raw(&h, 2, Phase::Up, 0.5, 0);
        if ui_only {
            up.logical_position = Some((-10.0, -10.0));
        }
        source
            .submit_with(
                paintcraft_input::tablet::receiver::Delivery::ReceiverPointer,
                |_| Ok(up),
            )
            .unwrap();
        h.step();
        assert_eq!(h.app.editor.session.undo_depth(), usize::from(!ui_only));
        let mut intake = intake.lock().unwrap();
        for buttons in [0x1f, 0x1e, 0x1c, 0x18, 0x10, 0] {
            let previous = intake.buttons();
            assert!(
                !intake
                    .button_changes(&source, previous, buttons, at, egui::Modifiers::default())
                    .unwrap()
            );
            if buttons != 0 {
                assert!(
                    !intake
                        .mouse(
                            &source,
                            egui::Event::PointerMoved(at),
                            buttons,
                            egui::Modifiers::default()
                        )
                        .unwrap()
                );
            }
        }
        let mut denied_records = Vec::new();
        intake.drain(&mut denied_records);
        assert!(denied_records.is_empty());
        assert!(
            intake
                .button_changes(&source, 0, 1, at, egui::Modifiers::default())
                .unwrap()
        );
        assert!(
            intake
                .button_changes(&source, 1, 0, at, egui::Modifiers::default())
                .unwrap()
        );
        intake.drain(&mut denied_records);
        assert_eq!(denied_records.len(), 2);
    }
}
