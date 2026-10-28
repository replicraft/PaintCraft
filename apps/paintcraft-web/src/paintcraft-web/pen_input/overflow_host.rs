//! Actual App/egui cancellation versus genuine mouse publication controls.
use super::*;
use paintcraft_input::tablet::{Buttons, Provenance, receiver};
struct Host {
    app: WebApp,
    ctx: egui::Context,
    frame: eframe::Frame,
}
impl Host {
    fn new() -> Self {
        let ctx = egui::Context::default();
        let mut host = Self {
            app: WebApp::new(&ctx),
            ctx,
            frame: eframe::Frame::_new_kittest(),
        };
        host.app.controls.tool = Tool::Brush;
        host.app.controls.options.size = 20;
        for _ in 0..3 {
            host.step();
        }
        host
    }
    fn step(&mut self) {
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 850.0),
            )),
            ..Default::default()
        };
        eframe::App::raw_input_hook(&mut self.app, &self.ctx, &mut raw);
        let mut output = self.ctx.run_ui(raw, |ui| {
            eframe::App::logic(&mut self.app, ui, &mut self.frame);
            eframe::App::ui(&mut self.app, ui, &mut self.frame);
        });
        output.textures_delta.clear();
    }
    fn point(&self, t: f32) -> egui::Pos2 {
        let rect = self.app.canvas.rect.unwrap();
        rect.min + rect.size() * egui::vec2(t, 0.5)
    }
    fn mouse(&mut self, at: egui::Pos2, buttons: u8) {
        self.queue_mouse(at, buttons);
        self.step();
    }
    fn queue_mouse(&mut self, at: egui::Pos2, buttons: u8) {
        let source = self.app.pen.source.clone().unwrap();
        let intake = self.app.pen.acquisition.clone().unwrap();
        let mut intake = intake.lock().unwrap();
        let previous = intake.buttons();
        if previous != buttons {
            assert!(
                intake
                    .button_changes(&source, previous, buttons, at, egui::Modifiers::NONE)
                    .unwrap()
            );
        } else {
            assert!(
                intake
                    .mouse(
                        &source,
                        Event::PointerMoved(at),
                        buttons,
                        egui::Modifiers::NONE
                    )
                    .unwrap()
            );
        }
        drop(intake);
    }
}

#[test]
fn trusted_outside_terminal_retires_only_known_tail_and_fresh_same_pen_publishes_once() {
    let mut h = Host::new();
    let initial = h.app.editor.session.document().clone();
    let source = h.app.pen.source.clone().unwrap();
    let acquisition = h.app.pen.acquisition.clone().unwrap();
    let mut n = events::Normalizer::new(7);
    let point = h.point(0.4);
    let send = |n: &mut events::Normalizer, kind, buttons, stamp| {
        n.observe_cancellation(source.cancellation_epoch());
        let mut event = pointer(100, kind, buttons);
        event.stamp = stamp;
        let mut packet = n.intake(event, Provenance::Actual).unwrap().unwrap();
        acquisition.lock().unwrap().pen(&mut packet.raw).unwrap();
        events::deliver_packet(&source, packet, |_| {
            Ok(Some((f64::from(point.x), f64::from(point.y))))
        })
        .unwrap()
    };
    assert_eq!(
        send(&mut n, events::Kind::Down, Buttons::TIP, 1.0),
        receiver::Route::Canvas
    );
    h.step();
    assert!(h.app.pen.controller_owned);
    // The capture-loss handler cancels; no canvas Up arrives afterward.
    n.clear();
    source.cancel();
    h.step();
    assert_eq!(h.app.editor.session.undo_depth(), 0);
    let mut terminal = pointer(100, events::Kind::Up, 0);
    terminal.stamp = 2.0;
    assert!(n.outside_terminal(terminal, false).unwrap().is_none());
    let mut foreign = terminal;
    foreign.id = 101;
    assert!(n.outside_terminal(foreign, true).unwrap().is_none());
    n.observe_cancellation(source.cancellation_epoch());
    let mut raw = n.outside_terminal(terminal, true).unwrap().unwrap();
    assert_eq!(raw.logical_position, None);
    assert_eq!(raw.identity.kind, paintcraft_input::tablet::DeviceKind::Pen);
    acquisition.lock().unwrap().pen(&mut raw).unwrap();
    assert_eq!(
        source.suppress_native(raw).unwrap(),
        receiver::Route::Suppressed
    );
    h.step();
    assert_eq!(h.app.editor.session.undo_depth(), 0);
    assert!(h.app.editor.session.document().same_semantics(&initial));
    assert_eq!(
        send(&mut n, events::Kind::Down, Buttons::TIP, 3.0),
        receiver::Route::Canvas
    );
    h.step();
    assert_eq!(
        send(&mut n, events::Kind::Up, 0, 4.0),
        receiver::Route::Canvas
    );
    h.step();
    assert_eq!(h.app.editor.session.undo_depth(), 1);
    assert!(!h.app.editor.session.document().same_semantics(&initial));
    h.app.editor.apply(paintcraft_tools::Action::Undo).unwrap();
    assert!(h.app.editor.session.document().same_semantics(&initial));
}

#[test]
fn already_drained_pen_batch_retirement_preserves_later_mouse_and_genuine_gui_guard() {
    let mut h = Host::new();
    let source = h.app.pen.source.clone().unwrap();
    let mut n = events::Normalizer::new(7);
    let point = h.point(0.4);
    for (kind, buttons) in [(events::Kind::Down, Buttons::TIP), (events::Kind::Up, 0)] {
        let mut packet = n
            .intake(pointer(100, kind, buttons), Provenance::Actual)
            .unwrap()
            .unwrap();
        h.app
            .pen
            .acquisition
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .pen(&mut packet.raw)
            .unwrap();
        events::deliver_packet(&source, packet, |_| {
            Ok(Some((f64::from(point.x), f64::from(point.y))))
        })
        .unwrap();
    }
    let mut raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 850.0),
        )),
        ..Default::default()
    };
    eframe::App::raw_input_hook(&mut h.app, &h.ctx, &mut raw);
    assert!(
        h.app
            .pen
            .pending
            .as_ref()
            .is_some_and(|(_, batch)| batch.frame.samples.len() == 2)
    );
    h.queue_mouse(point, 1);
    let gui = source.ui_publication_guard(h.app.pen_owner()).unwrap();
    lockout(&mut h, &mut n);
    assert!(
        gui.is_current(),
        "pen-only retirement must not revoke genuine GUI work"
    );
    assert_eq!(
        h.app.editor.session.undo_depth(),
        0,
        "drained pen must not publish"
    );
    let end = h.point(0.6);
    h.mouse(end, 1);
    h.mouse(end, 0);
    assert_eq!(h.app.editor.session.undo_depth(), 1);
}
fn pointer(id: u64, kind: events::Kind, buttons: u16) -> events::Pointer {
    events::Pointer {
        id,
        kind,
        at: None,
        stamp: 100.0 + id as f64,
        buttons,
        button: match kind {
            events::Kind::Down if buttons & Buttons::BARREL != 0 => 2,
            events::Kind::Down => 0,
            events::Kind::Up => 0,
            _ => -1,
        },
        pressure: 0.5,
        tilt: (0.0, 0.0),
        twist: 0.0,
        contact: (1.0, 1.0),
        modifiers: Default::default(),
    }
}
fn lockout(host: &mut Host, n: &mut events::Normalizer) {
    let source = host.app.pen.source.clone().unwrap();
    let p = host.point(0.4);
    for id in 0..16 {
        let mut packet = n
            .intake(
                pointer(id, events::Kind::Down, Buttons::BARREL),
                Provenance::Actual,
            )
            .unwrap()
            .unwrap();
        host.app
            .pen
            .acquisition
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .pen(&mut packet.raw)
            .unwrap();
        let route = events::deliver_packet(&source, packet, |_| {
            Ok(Some((f64::from(p.x), f64::from(p.y))))
        })
        .unwrap();
        if route == paintcraft_input::tablet::receiver::Route::Suppressed {
            n.deny_contact(id);
        }
    }
    assert_eq!(
        n.intake(
            pointer(16, events::Kind::Down, Buttons::BARREL),
            Provenance::Actual
        ),
        Err(paintcraft_input::tablet::Invalid::Capacity)
    );
    // Same classified-only transition used by DOM, without weakening the
    // existing global cancellation path for acquisition failure.
    let reset = source.retire_pen();
    host.app.retire_pen_private_scoped(&host.ctx, reset);
    n.clear();
    host.step();
}
#[test]
fn sticky_pen_failure_and_unknown_tails_do_not_cancel_genuine_mouse_or_change_saved_result() {
    let mut reference = Host::new();
    let initial = reference.app.editor.session.document().clone();
    let start = reference.point(0.4);
    let end = reference.point(0.6);
    reference.mouse(start, 1);
    reference.mouse(end, 1);
    reference.mouse(end, 0);
    assert_eq!(reference.app.editor.session.undo_depth(), 1);
    let expected =
        paintcraft_formats::pcnd::write(reference.app.editor.session.document()).unwrap();
    for already_owned in [false, true] {
        let mut h = Host::new();
        h.app.editor = crate::editor::Editor::from_document(initial.clone()).unwrap();
        for _ in 0..3 {
            h.step();
        }
        let start = h.point(0.4);
        let end = h.point(0.6);
        let mut n = events::Normalizer::new(7);
        if already_owned {
            h.mouse(start, 1);
        }
        lockout(&mut h, &mut n);
        if !already_owned {
            h.mouse(start, 1);
        }
        for (kind, buttons) in [(events::Kind::Move, Buttons::BARREL), (events::Kind::Up, 0)] {
            assert_eq!(
                n.intake(pointer(16, kind, buttons), Provenance::Actual)
                    .unwrap(),
                None
            );
            h.mouse(end, 1);
        }
        // Source keeps a conservative fence for this malformed known tail, but
        // the locked pen's clock failure cannot reset the genuine mouse.
        let source = h.app.pen.source.clone().unwrap();
        let mut bad = pointer(8, events::Kind::Move, Buttons::BARREL);
        bad.stamp = 1.0;
        let mut packet = n.intake(bad, Provenance::Actual).unwrap().unwrap();
        h.app
            .pen
            .acquisition
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .pen(&mut packet.raw)
            .unwrap();
        assert_eq!(
            events::deliver_locked_packet(&source, packet, true, |_| panic!(
                "locked tail geometry"
            )),
            Ok(paintcraft_input::tablet::receiver::Route::Suppressed)
        );
        h.mouse(end, 1);
        h.mouse(end, 0);
        assert_eq!(
            h.app.editor.session.undo_depth(),
            1,
            "mouse already_owned={already_owned}"
        );
        assert_eq!(
            paintcraft_formats::pcnd::write(h.app.editor.session.document()).unwrap(),
            expected,
            "mouse already_owned={already_owned}"
        );
        h.app.editor.apply(paintcraft_tools::Action::Undo).unwrap();
        assert!(h.app.editor.session.document().same_semantics(&initial));
    }
}
