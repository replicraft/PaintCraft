use super::*;
use paintcraft_core::types::brush::BrushMedia;
use paintcraft_input::{Modifiers, tablet::*};
use paintcraft_ui::pen_input::Source;

struct Host {
    app: WebApp,
    ctx: egui::Context,
    frame: eframe::Frame,
}
impl Host {
    fn state(&self) -> &WebApp {
        &self.app
    }
    fn state_mut(&mut self) -> &mut WebApp {
        &mut self.app
    }
    fn step(&mut self) {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 850.0),
            )),
            ..Default::default()
        };
        // egui_kittest0.37 does not call this hook. Reproduce actual eframe
        // ordering explicitly, before Context parses the GUI pointer stream.
        eframe::App::raw_input_hook(&mut self.app, &self.ctx, &mut input);
        let mut output = self.ctx.run_ui(input, |ui| {
            eframe::App::logic(&mut self.app, ui, &mut self.frame);
            eframe::App::ui(&mut self.app, ui, &mut self.frame);
        });
        output.textures_delta.clear();
    }
    fn run(&mut self) {
        for _ in 0..3 {
            self.step();
        }
    }
}
fn harness() -> Host {
    let ctx = egui::Context::default();
    let mut h = Host {
        app: WebApp::new(&ctx),
        ctx,
        frame: eframe::Frame::_new_kittest(),
    };
    h.run();
    h
}
fn raw(h: &Host, sequence: u64, phase: Phase, pressure: f32, buttons: u16) -> RawSample {
    let rect = h.state().canvas.rect.unwrap();
    let (w, height) = h.state().editor.session.document().canvas;
    let at = rect.min
        + egui::vec2(
            w as f32 * if sequence == 1 { 0.4 } else { 0.6 },
            height as f32 * 0.5,
        ) * h.state().zoom;
    RawSample {
        identity: Identity {
            device: 0,
            pointer: 9,
            kind: DeviceKind::Pen,
        },
        sequence,
        time: Time {
            epoch: 7,
            micros: sequence * 100,
            clock: Clock::BrowserPerformance,
        },
        phase,
        logical_position: Some((f64::from(at.x), f64::from(at.y))),
        buttons: Buttons(buttons),
        modifiers: Modifiers::default(),
        pressure: Some(pressure),
        tilt: Some(Tilt::Degrees(12.0, -8.0)),
        twist_degrees: Some(31.0),
        contact: Some((2.0, 1.0)),
        provenance: Provenance::Actual,
    }
}
fn submit(h: &mut Host, raw: RawSample) {
    let source: Source = h.state().pen.source.clone().unwrap();
    assert_eq!(
        source
            .submit_with(
                paintcraft_input::tablet::receiver::Delivery::ReceiverPointer,
                |_| Ok(raw)
            )
            .unwrap(),
        paintcraft_input::tablet::receiver::Route::Canvas
    );
    h.step();
}
#[test]
fn actual_eframe_host_pressure_preview_release_and_one_undo_match() {
    for frontend in [Frontend::Classic, Frontend::Studio] {
        let mut outputs = Vec::new();
        for varying in [false, true] {
            let mut h = harness();
            h.state_mut().frontend = frontend;
            h.state_mut().controls.options.size = 28;
            h.run();
            let before = h.state().editor.session.ticket();
            let down = raw(
                &h,
                1,
                Phase::Down,
                if varying { 0.2 } else { 1.0 },
                Buttons::TIP,
            );
            submit(&mut h, down);
            let mut moved = raw(
                &h,
                2,
                Phase::Move,
                if varying { 0.8 } else { 1.0 },
                Buttons::TIP,
            );
            moved.provenance = Provenance::Coalesced;
            submit(&mut h, moved);
            assert_eq!(h.state().editor.session.ticket(), before);
            assert!(h.state().editor.tools.pen_in_progress());
            let preview = paintcraft_render::render_flat(
                &h.state().editor.display_document().unwrap().unwrap(),
            )
            .unwrap();
            let up = raw(&h, 3, Phase::Up, if varying { 0.8 } else { 1.0 }, 0);
            submit(&mut h, up);
            assert_eq!(h.state().editor.session.undo_depth(), 1);
            assert_eq!(
                paintcraft_render::render_flat(h.state().editor.session.document()).unwrap(),
                preview
            );
            outputs.push(preview);
        }
        assert_ne!(outputs[0], outputs[1]);
    }
}
#[test]
fn held_fallback_button_is_latched_despite_barrel_changes() {
    for secondary in [false, true] {
        let mut h = harness();
        h.state_mut().controls.options.media = BrushMedia::ALL[1];
        h.state_mut().controls.primary = paintcraft_core::types::Rgba([10, 20, 30, 255]);
        h.state_mut().controls.secondary = paintcraft_core::types::Rgba([200, 30, 180, 255]);
        h.run();
        let mut down = raw(
            &h,
            1,
            Phase::Down,
            0.4,
            Buttons::TIP | if secondary { Buttons::BARREL } else { 0 },
        );
        down.provenance = Provenance::Actual;
        submit(&mut h, down);
        let expected = if secondary {
            paintcraft_input::PointerButton::Secondary
        } else {
            paintcraft_input::PointerButton::Primary
        };
        assert_eq!(h.state().editor.tools.gesture_button(), Some(expected));
        let moved = raw(
            &h,
            2,
            Phase::Move,
            0.7,
            Buttons::TIP | if secondary { 0 } else { Buttons::BARREL },
        );
        submit(&mut h, moved);
        assert_eq!(h.state().editor.tools.gesture_button(), Some(expected));
        let up = raw(&h, 3, Phase::Up, 0.0, 0);
        submit(&mut h, up);
        assert_eq!(h.state().editor.session.undo_depth(), 1);
        assert!(!h.state().editor.tools.gesture_in_progress());
    }
}
#[test]
fn drained_fallback_is_retired_if_modal_or_interface_changes_or_canvas_is_skipped() {
    for mode in 0..3 {
        let mut h = harness();
        h.state_mut().controls.options.media = BrushMedia::ALL[1];
        h.run();
        let down = raw(&h, 1, Phase::Down, 0.4, Buttons::TIP);
        submit(&mut h, down);
        assert!(h.state().editor.tools.gesture_in_progress());
        let source = h.state().pen.source.clone().unwrap();
        let up = raw(&h, 2, Phase::Up, 0.0, 0);
        source
            .submit_with(
                paintcraft_input::tablet::receiver::Delivery::ReceiverPointer,
                |_| Ok(up),
            )
            .unwrap();
        let ctx = h.ctx.clone();
        h.state_mut()
            .pen_raw_input(&ctx, &mut egui::RawInput::default());
        match mode {
            0 => h.state_mut().settings = true,
            1 => h.state_mut().frontend = Frontend::Studio,
            _ => {}
        }
        h.state_mut().finish_pen_view(&ctx);
        assert_eq!(h.state().editor.session.undo_depth(), 0, "mode {mode}");
        assert!(!h.state().editor.tools.gesture_in_progress(), "mode {mode}");
        h.step();
        assert_eq!(h.state().editor.session.undo_depth(), 0);
    }
}
#[test]
fn genuine_mouse_retains_capture_until_all_buttons_release() {
    let mut h = harness();
    let ctx = h.ctx.clone();
    let source = h.state().pen.source.clone().unwrap();
    let pos = egui::pos2(5.0, 5.0);
    let event = |button, pressed| egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let mut input = egui::RawInput {
        events: vec![
            event(egui::PointerButton::Primary, true),
            event(egui::PointerButton::Secondary, true),
            event(egui::PointerButton::Secondary, false),
        ],
        ..Default::default()
    };
    h.state_mut().pen_raw_input(&ctx, &mut input);
    let sample = raw(&h, 1, Phase::Down, 0.5, Buttons::TIP);
    assert_eq!(
        source
            .submit_with(
                paintcraft_input::tablet::receiver::Delivery::ReceiverPointer,
                |_| Ok(sample)
            )
            .unwrap(),
        paintcraft_input::tablet::receiver::Route::Suppressed
    );
    h.step();
    let mut input = egui::RawInput {
        events: vec![event(egui::PointerButton::Primary, false)],
        ..Default::default()
    };
    h.state_mut().pen_raw_input(&ctx, &mut input);
    assert_eq!(
        source
            .submit_with(
                paintcraft_input::tablet::receiver::Delivery::ReceiverPointer,
                |_| Ok(raw(&h, 2, Phase::Move, 0.5, Buttons::TIP))
            )
            .unwrap(),
        paintcraft_input::tablet::receiver::Route::Suppressed
    );
    // Releasing the mouse cannot revive the pen's denied physical tail.
    let mut terminal = raw(&h, 3, Phase::Up, 0.0, 0);
    terminal.logical_position = None;
    terminal.provenance = Provenance::Actual;
    assert_eq!(
        source.suppress_native(terminal).unwrap(),
        paintcraft_input::tablet::receiver::Route::Suppressed
    );
    assert_eq!(
        source
            .submit_with(
                paintcraft_input::tablet::receiver::Delivery::ReceiverPointer,
                |_| Ok(raw(&h, 4, Phase::Down, 0.5, Buttons::TIP)),
            )
            .unwrap(),
        paintcraft_input::tablet::receiver::Route::Canvas
    );
}

#[test]
fn fully_drained_down_up_never_publishes_before_current_modal_owner_or_canvas_check() {
    for mode in 0..4 {
        let mut h = harness();
        let source = h.state().pen.source.clone().unwrap();
        for sample in [
            raw(&h, 1, Phase::Down, 0.3, Buttons::TIP),
            raw(&h, 2, Phase::Up, 0.8, 0),
        ] {
            source
                .submit_with(
                    paintcraft_input::tablet::receiver::Delivery::ReceiverPointer,
                    |_| Ok(sample),
                )
                .unwrap();
        }
        let ctx = h.ctx.clone();
        let before = h.state().editor.session.ticket();
        eframe::App::raw_input_hook(h.state_mut(), &ctx, &mut egui::RawInput::default());
        assert_eq!(
            h.state().editor.session.ticket(),
            before,
            "raw hook mode {mode}"
        );
        assert_eq!(h.state().editor.session.undo_depth(), 0);
        match mode {
            0 => h.state_mut().settings = true,
            1 => h.state_mut().frontend = Frontend::Studio,
            2 => {
                let doc = h.state().editor.session.document().clone();
                h.state_mut().editor = Editor::from_document(doc).unwrap();
            }
            _ => h.state_mut().finish_pen_view(&ctx),
        }
        h.step();
        assert_eq!(h.state().editor.session.undo_depth(), 0, "mode {mode}");
        assert!(!h.state().editor.tools.gesture_in_progress());
    }
}

#[test]
fn a_drained_cancellation_does_not_recancel_the_following_fresh_stroke() {
    let mut h = harness();
    let down = raw(&h, 1, Phase::Down, 0.3, Buttons::TIP);
    submit(&mut h, down);
    let source = h.state().pen.source.clone().unwrap();
    source.cancel();
    // Host cancellation retires ink, but only a real zero-button terminal
    // retires this physical contact before its next actual Down.
    let mut terminal = raw(&h, 2, Phase::Up, 0.0, 0);
    terminal.logical_position = None;
    terminal.provenance = Provenance::Actual;
    assert_eq!(
        source.suppress_native(terminal).unwrap(),
        paintcraft_input::tablet::receiver::Route::Suppressed
    );
    let fresh = raw(&h, 3, Phase::Down, 0.8, Buttons::TIP);
    submit(&mut h, fresh);
    assert!(h.state().editor.tools.pen_in_progress());
    h.step();
    assert!(h.state().editor.tools.pen_in_progress());
    let up = raw(&h, 4, Phase::Up, 0.8, 0);
    submit(&mut h, up);
    assert_eq!(h.state().editor.session.undo_depth(), 1);
}
#[test]
fn historical_contact_edges_cannot_begin_or_end_actual_host_ink() {
    let mut h = harness();
    let source = h.app.pen.source.clone().unwrap();
    let mut historical_down = raw(&h, 1, Phase::Down, 0.5, Buttons::TIP);
    historical_down.provenance = Provenance::Coalesced;
    assert_eq!(
        source
            .submit_with(receiver::Delivery::ReceiverPointer, |_| Ok(historical_down))
            .unwrap(),
        receiver::Route::Suppressed
    );
    h.step();
    assert!(!h.app.editor.tools.pen_in_progress());
    assert_eq!(h.app.editor.session.undo_depth(), 0);
    let down = raw(&h, 1, Phase::Down, 0.5, Buttons::TIP);
    submit(&mut h, down);
    let mut historical_up = raw(&h, 2, Phase::Up, 0.5, 0);
    historical_up.provenance = Provenance::Coalesced;
    assert_eq!(
        source
            .submit_with(receiver::Delivery::ReceiverPointer, |_| Ok(historical_up))
            .unwrap(),
        receiver::Route::Suppressed
    );
    h.step();
    assert!(h.app.editor.tools.pen_in_progress());
    assert_eq!(h.app.editor.session.undo_depth(), 0);
    let up = raw(&h, 2, Phase::Up, 0.5, 0);
    submit(&mut h, up);
    assert!(!h.app.editor.tools.pen_in_progress());
    assert_eq!(h.app.editor.session.undo_depth(), 1);
}

#[path = "arbitration_tests.rs"]
mod arbitration;
