use super::*;
use paintcraft_platform::tablet::{Boundary, WheelPhase};
fn control(host: &Host, ordinal: u64, kind: Option<Boundary>) {
    let event = kind.map_or_else(
        || {
            CaptureEvent::Mouse(MouseRecord {
                kind: MouseKind::Wheel {
                    delta: (0., 0.),
                    precise: true,
                    physical_scale: 1.,
                    phase: WheelPhase::Move,
                    momentum: false,
                },
                position: WindowPosition::PhysicalPixels(10., 10.),
                client: true,
                modifiers: Modifiers::default(),
                time: time(ordinal),
            })
        },
        |kind| CaptureEvent::Boundary { kind, time: None },
    );
    host.capture_endpoint()(CaptureRecord {
        installation: 1,
        ordinal,
        event,
    })
    .unwrap();
}
#[test]
fn active_retirement_preserves_an_older_completed_paused_pen_job() {
    for boundary in [None, Some(Boundary::Keyboard), Some(Boundary::ImeReceipt)] {
        let (mut host, ctx, _, _) = setup(Experience::Studio);
        let before = host.app.editor.publications();
        let depth = host.app.editor.undo_depth();
        let release = host.app.editing.clone().unwrap().pause();
        pen(&host, 0, Phase::Down, (20., 18.));
        pen(&host, 1, Phase::Up, (20., 18.));
        assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
        let activation = host.waiting.as_ref().unwrap().activation;
        pen(&host, 2, Phase::Down, (34., 18.));
        control(&host, 3, boundary);
        assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
        assert_eq!(host.waiting.as_ref().unwrap().activation, activation);
        assert_eq!(host.app.editor.publications(), before);
        release.send(()).unwrap();
        finish(&mut host, &ctx);
        assert_eq!(host.app.editor.publications(), before + 1);
        assert_eq!(host.app.editor.undo_depth(), depth + 1);
        pen(&host, 4, Phase::Up, (34., 18.));
        pen(&host, 5, Phase::Down, (36., 20.));
        pen(&host, 6, Phase::Up, (36., 20.));
        finish(&mut host, &ctx);
        assert_eq!(host.app.editor.publications(), before + 2);
    }
}
#[test]
fn active_retirement_preserves_a_completed_genuine_mouse_undo_job() {
    let (mut host, ctx, undo, _) = setup(Experience::Studio);
    let before = host.app.editor.publications();
    let depth = host.app.editor.undo_depth();
    let release = host.app.editing.clone().unwrap().pause();
    mouse(&host, 0, true, undo);
    mouse(&host, 1, false, undo);
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::Advanced);
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
    pen(&host, 2, Phase::Down, (24., 18.));
    control(&host, 3, Some(Boundary::Keyboard));
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
    release.send(()).unwrap();
    finish(&mut host, &ctx);
    assert_eq!(host.app.editor.publications(), before + 1);
    assert_eq!(host.app.editor.undo_depth(), depth - 1);
}
#[test]
fn typed_framework_interval_waits_for_receipt_then_precedes_later_pen() {
    let (mut host, ctx, _, _) = setup(Experience::Studio);
    let before = host.app.editor.publications();
    let release = host.app.editing.clone().unwrap().pause();
    pen(&host, 0, Phase::Down, (20., 18.));
    pen(&host, 1, Phase::Up, (20., 18.));
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
    let mut typed = input();
    typed
        .events
        .push(egui::Event::Text("deferred text".to_owned()));
    typed.events.push(egui::Event::Key {
        key: egui::Key::F20,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    });
    host.snapshot(DeliveryHealth::Ready).unwrap();
    let step = host.step(&ctx, typed).unwrap();
    assert_eq!(step.progress, Progress::WaitingForOwnEdit);
    assert!(!ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Text(_)))));
    pen(&host, 2, Phase::Down, (34., 18.));
    pen(&host, 3, Phase::Up, (34., 18.));
    release.send(()).unwrap();
    let mut delivered = 0;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let progress = tick(&mut host, &ctx).unwrap();
        if ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(text) if text == "deferred text"))
        }) {
            delivered += 1;
            assert_eq!(
                host.app.editor.publications(),
                before + 1,
                "framework interval must precede later pen publication"
            );
            assert!(ctx.input(|i| i.events.iter().any(|e| matches!(
                e,
                egui::Event::Key {
                    key: egui::Key::F20,
                    ..
                }
            ))));
        }
        if progress == Progress::Idle {
            break;
        }
        assert!(Instant::now() < deadline);
    }
    assert_eq!(delivered, 1);
    assert_eq!(host.app.editor.publications(), before + 2);
}
#[test]
fn exact_retirement_cancels_presented_preview_even_if_guard_failed_before_sync() {
    let (mut host, ctx, _, older) = setup(Experience::Studio);
    let depth = host.app.editor.undo_depth();
    pen(&host, 0, Phase::Down, (20., 18.));
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::Advanced);
    assert!(host.app.owned_presenter_state().0);
    assert!(host.app.owned_presenter_state().1.is_some());
    control(&host, 1, Some(Boundary::Keyboard));
    // Callback failure/retirement may happen between GUI work and this sync.
    // Keep the old opaque negative authority until its descriptor is applied.
    host.app.sync_owned_presenter();
    assert!(host.app.owned_presenter_state().1.is_some());
    finish(&mut host, &ctx);
    assert!(!host.app.owned_presenter_state().0);
    assert!(host.app.owned_presenter_state().1.is_none());
    assert_eq!(host.app.editor.undo_depth(), depth);
    assert!(host.app.editor.document().same_semantics(&older));
    pen(&host, 2, Phase::Up, (20., 18.));
    pen(&host, 3, Phase::Down, (34., 18.));
    pen(&host, 4, Phase::Up, (34., 18.));
    finish(&mut host, &ctx);
    assert_eq!(host.app.editor.undo_depth(), depth + 1);
}
fn ui_pen(host: &Host, ordinal: u64, phase: Phase, buttons: u16, at: Pos2) {
    let ppp = host
        .ingress
        .lock()
        .unwrap()
        .view()
        .unwrap()
        .pixels_per_point;
    host.capture_endpoint()(CaptureRecord {
        installation: 1,
        ordinal,
        event: CaptureEvent::Tablet(NativePacket {
            raw: RawSample {
                identity: Identity {
                    device: 7,
                    pointer: 9,
                    kind: DeviceKind::Pen,
                },
                sequence: ordinal,
                time: time(ordinal),
                phase,
                logical_position: None,
                buttons: Buttons(buttons),
                modifiers: Default::default(),
                pressure: Some(0.7),
                tilt: None,
                twist_degrees: None,
                contact: None,
                provenance: Provenance::Actual,
            },
            button_edge: None,
            delivery: paintcraft_platform::tablet::Delivery::ReceiverPointer,
            position: Some(WindowPosition::PhysicalPixels(
                f64::from(at.x) * ppp,
                f64::from(at.y) * ppp,
            )),
        }),
    })
    .unwrap();
}
#[test]
fn exact_retirement_abandons_only_the_matching_active_ui_chord_job() {
    let (mut host, ctx, undo, older) = setup(Experience::Studio);
    let depth = host.app.editor.undo_depth();
    let publications = host.app.editor.publications();
    let release = host.app.editing.clone().unwrap().pause();
    ui_pen(&host, 0, Phase::Down, Buttons::TIP | Buttons::BARREL, undo);
    ui_pen(&host, 1, Phase::Up, Buttons::BARREL, undo);
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::Advanced);
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
    assert_eq!(
        host.app.owned_presenter_state().1.unwrap(),
        paintcraft_ui::pen_input::NativePenTarget::Ui
    );
    host.snapshot(DeliveryHealth::Ready).unwrap();
    // Actual negative can arrive after the last gated snapshot, before step.
    // Test has no bound native gate, so this exercises guard-expiry fallback.
    control(&host, 2, Some(Boundary::Keyboard));
    let _ = host.step(&ctx, input()).unwrap();
    assert!(host.waiting.is_none());
    assert!(host.own_work.is_none());
    release.send(()).unwrap();
    finish(&mut host, &ctx);
    assert_eq!(host.app.editor.publications(), publications);
    assert_eq!(host.app.editor.undo_depth(), depth);
    assert!(host.app.editor.document().same_semantics(&older));
}
