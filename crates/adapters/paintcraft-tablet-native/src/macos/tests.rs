#![deny(unsafe_code)]
use super::*;
use paintcraft_input::tablet::{Buttons, Phase, Provenance};
fn point(buttons: u16, micros: u64) -> Point {
    Point {
        device: 7,
        in_view: true,
        micros,
        position: WindowPosition::PhysicalPixels(300., 200.),
        buttons,
        modifiers: Modifiers::default(),
        pressure: 0.0,
        tilt: (0.25, -0.5),
        rotation: -45.0,
    }
}
fn state(caps: u64, kind: DeviceKind) -> State {
    let mut state = State::new(19).unwrap();
    assert!(state.proximity(7, 11, kind, caps).0);
    state
}
fn packet(state: &mut State, point: Point) -> NativePacket {
    match state.prepare(point) {
        Action::Dispatch(packet) => packet,
        other => panic!("{other:?}"),
    }
}
#[test]
fn reported_zero_unknown_axes_and_eraser_upper_barrel_remain_distinct() {
    let mut s = state(0x2580, DeviceKind::Eraser);
    let received = packet(&mut s, point(Buttons::TIP | 4, 1_250_000));
    assert_eq!(received.raw.phase, Phase::Down);
    assert_eq!(received.raw.pressure, Some(0.0));
    assert_eq!(received.raw.buttons.0, Buttons::TIP | 4 | Buttons::ERASER);
    assert_eq!(received.raw.identity.kind, DeviceKind::Eraser);
    assert_eq!(received.raw.twist_degrees, Some(315.0));
    assert_eq!(
        received.raw.tilt,
        Some(paintcraft_input::tablet::Tilt::Normalized(0.25, -0.5))
    );
    assert_eq!(
        received.raw.time.clock,
        paintcraft_input::tablet::Clock::AppKitUptime
    );
    assert_eq!(received.raw.time.epoch, 19);
    let mut unknown = state(0, DeviceKind::Pen);
    let missing = packet(&mut unknown, point(Buttons::TIP, 1));
    assert_eq!(missing.raw.pressure, None);
    assert_eq!(missing.raw.tilt, None);
    assert_eq!(missing.raw.twist_degrees, None);
}

#[test]
fn classified_stream_never_leaks_duplicates_hover_buttons_or_up_to_legacy_dispatch() {
    let state = Mutex::new(state(0x400, DeviceKind::Pen));
    let received = Arc::new(Mutex::new(Vec::new()));
    let sink_records = received.clone();
    let emitter = Emitter::new(
        Arc::new(move |packet| {
            sink_records.lock().unwrap().push(packet);
            false
        }),
        Arc::new(|| {}),
        Report::new(Backend::AppKit),
    );
    for p in [
        point(4, 1),
        point(1 | 4, 2),
        point(1 | 4, 2),
        point(0, 3),
        point(0, 3),
        point(0, 4),
    ] {
        assert!(dispatch(&state, &emitter, p));
    }
    let records = received.lock().unwrap();
    assert_eq!(records.len(), 4);
    assert_eq!(
        records.iter().map(|p| p.raw.phase).collect::<Vec<_>>(),
        [Phase::Hover, Phase::Down, Phase::Up, Phase::Hover]
    );
    assert_eq!(records[0].raw.buttons.0, 4);
    assert!(
        records
            .iter()
            .all(|p| p.delivery == Delivery::ReceiverPointer)
    );
}

#[test]
fn focus_loss_cancels_once_and_fences_the_still_down_physical_tail() {
    let mut s = state(0x400, DeviceKind::Pen);
    let down = packet(&mut s, point(1, 1));
    let cancellations: Vec<_> = s.cancel().into_iter().flatten().collect();
    assert_eq!(cancellations.len(), 1);
    assert_eq!(cancellations[0].raw.phase, Phase::Cancel);
    assert_eq!(cancellations[0].raw.provenance, Provenance::Boundary);
    assert_eq!(cancellations[0].raw.time, down.raw.time);
    assert_eq!(cancellations[0].position, None);
    assert!(s.cancel().into_iter().flatten().next().is_none());
    assert!(matches!(s.prepare(point(1, 2)), Action::Suppress));
    assert!(matches!(s.prepare(point(0, 3)), Action::Suppress));
    assert_eq!(packet(&mut s, point(1, 4)).raw.phase, Phase::Down);
}
#[test]
fn callback_reentrant_cancel_is_unlocked_and_cannot_resurrect_contact() {
    let state = Arc::new(Mutex::new(state(0x400, DeviceKind::Pen)));
    let callback_state = state.clone();
    let emitter = Emitter::new(
        Arc::new(move |_| {
            callback_state.lock().unwrap().cancel();
            true
        }),
        Arc::new(|| {}),
        Report::new(Backend::AppKit),
    );
    assert!(dispatch(&state, &emitter, point(1, 1)));
    assert!(matches!(
        state.lock().unwrap().prepare(point(1, 2)),
        Action::Suppress
    ));
}
#[test]
fn device_capacity_and_missing_proximity_never_invent_pen_attributes() {
    let mut s = State::new(1).unwrap();
    assert!(matches!(
        s.prepare(point(1, 1)),
        Action::Unclassified { consume: true }
    ));
    for device in 0..64 {
        assert!(s.proximity(device, device, DeviceKind::Pen, 0x400).0);
    }
    assert!(!s.proximity(64, 64, DeviceKind::Pen, 0x400).0);
    s.leave(20);
    assert!(s.proximity(64, 64, DeviceKind::Eraser, 0x400).0);
}

#[test]
fn malformed_first_contact_is_fenced_without_cancelling_an_unrelated_device() {
    let mut s = state(0x400, DeviceKind::Pen);
    assert!(s.proximity(8, 12, DeviceKind::Pen, 0x400).0);
    let mut other = point(1, 1);
    other.device = 8;
    packet(&mut s, other);
    assert!(s.quarantine(7, true, true).1.is_none());
    assert!(matches!(s.prepare(point(1, 2)), Action::Suppress));
    other.micros = 2;
    assert_eq!(packet(&mut s, other).raw.phase, Phase::Move);
    assert!(matches!(s.prepare(point(0, 3)), Action::Suppress));
    assert_eq!(packet(&mut s, point(1, 4)).raw.phase, Phase::Down);
}

#[test]
fn native_fencing_inside_delivery_handoff_does_not_reenter_the_sink() {
    let state = Mutex::new(state(0x400, DeviceKind::Pen));
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let callback_calls = calls.clone();
    let emitter = Emitter::new(
        Arc::new(move |_| {
            callback_calls.fetch_add(1, Ordering::SeqCst);
            true
        }),
        Arc::new(|| {}),
        Report::new(Backend::AppKit),
    );
    assert!(dispatch(&state, &emitter, point(1, 1)));
    emitter.gate.with_delivery(|health| {
        assert_eq!(health, DeliveryHealth::Ready);
        fence(&state);
    });
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(dispatch(&state, &emitter, point(1, 2)));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn native_chrome_and_client_contacts_keep_their_original_view_route() {
    let mut s = state(0x400, DeviceKind::Pen);
    let mut p = point(1, 1);
    p.in_view = false;
    assert!(matches!(s.prepare(p), Action::Native));
    p.micros = 2;
    p.in_view = true;
    assert!(matches!(s.prepare(p), Action::Native));
    assert!(s.cancel().into_iter().flatten().next().is_none());
    p.micros = 3;
    p.buttons = 0;
    assert!(matches!(s.prepare(p), Action::Native));
    assert!(matches!(s.prepare(p), Action::Native));
    p.micros = 4;
    p.buttons = 1;
    assert_eq!(packet(&mut s, p).raw.phase, Phase::Down);
    p.micros = 5;
    p.in_view = false;
    assert_eq!(packet(&mut s, p).raw.phase, Phase::Move);
    p.micros = 6;
    p.buttons = 0;
    p.in_view = true;
    assert_eq!(packet(&mut s, p).raw.phase, Phase::Up);
    p.in_view = false; // Focus/scope changed before a copied native mouse-up.
    assert!(matches!(s.prepare(p), Action::Suppress));
}

#[test]
fn unidentified_client_tail_needs_real_metadata_and_complete_release_before_ink() {
    let mut s = State::new(1).unwrap();
    for p in [point(1 | 4, 1), point(1 | 4, 2)] {
        assert!(matches!(
            s.prepare(p),
            Action::Unclassified { consume: true }
        ));
    }
    assert!(s.proximity(7, 123, DeviceKind::Eraser, 0x400).0);
    assert!(matches!(s.prepare(point(1 | 4, 3)), Action::Suppress));
    assert!(matches!(s.prepare(point(4, 4)), Action::Suppress));
    assert!(matches!(s.prepare(point(0, 5)), Action::Suppress));
    let mut duplicate = point(0, 5);
    duplicate.in_view = false;
    assert!(matches!(s.prepare(duplicate), Action::Suppress));
    let down = packet(&mut s, point(1, 6));
    assert_eq!(down.raw.phase, Phase::Down);
    assert_eq!(down.raw.identity.pointer, 123);
    assert_eq!(down.raw.identity.kind, DeviceKind::Eraser);
    assert_eq!(down.raw.pressure, Some(0.0));
}

#[test]
fn unidentified_native_chrome_keeps_route_across_metadata_and_view_crossing() {
    let mut s = State::new(1).unwrap();
    let mut chrome = point(1, 1);
    chrome.in_view = false;
    assert!(matches!(
        s.prepare(chrome),
        Action::Unclassified { consume: false }
    ));
    assert!(s.proximity(7, 11, DeviceKind::Pen, 0x400).0);
    assert!(matches!(s.prepare(point(1, 2)), Action::Native));
    assert!(matches!(s.prepare(point(0, 3)), Action::Native));
    assert_eq!(packet(&mut s, point(1, 4)).raw.phase, Phase::Down);
    // Genuine proximity loss and subsequent metadata cannot resurrect a held
    // canvas contact, including after a focus cancellation cleared last_point.
    assert!(s.cancel().into_iter().flatten().next().is_some());
    s.leave(7);
    assert!(s.proximity(7, 12, DeviceKind::Pen, 0x400).0);
    assert!(matches!(s.prepare(point(1, 5)), Action::Suppress));
    assert!(matches!(s.prepare(point(0, 6)), Action::Suppress));
    assert_eq!(packet(&mut s, point(1, 7)).raw.phase, Phase::Down);
}

#[test]
fn unknown_tablet_never_reaches_sink_and_capacity_is_fail_closed() {
    let state = Mutex::new(State::new(1).unwrap());
    let emitter = Emitter::new(
        Arc::new(|_| panic!("Unidentified tablet must not emit a fabricated sample")),
        Arc::new(|| {}),
        Report::new(Backend::AppKit),
    );
    assert!(dispatch(&state, &emitter, point(1, 1)));
    let mut outside = point(0, 2);
    outside.in_view = false;
    assert!(dispatch(&state, &emitter, outside));
    assert!(dispatch(&state, &emitter, outside));
    assert_eq!(emitter.report.snapshot().samples, 0);
    assert_eq!(emitter.report.snapshot().unclassified_points, 3);
    assert_eq!(emitter.report.snapshot().sink_failures, 0);
    for device in 8..71 {
        let mut p = point(1, 3);
        p.device = device;
        assert!(dispatch(&state, &emitter, p));
    }
    let mut overflow = outside;
    overflow.device = 1000;
    assert!(dispatch(&state, &emitter, overflow));
    assert!(
        state
            .lock()
            .unwrap()
            .proximity(1000, 1, DeviceKind::Pen, 0)
            .0
    );
    // The identity is now real, but the untracked physical tail must also lift.
    assert!(matches!(
        state.lock().unwrap().prepare(Point {
            buttons: 1,
            ..overflow
        }),
        Action::Suppress
    ));
}

#[test]
fn malformed_unidentified_contact_is_fenced_without_inventing_position() {
    let mut s = State::new(1).unwrap();
    assert_eq!(s.quarantine(7, true, true), (true, None));
    assert!(s.proximity(7, 11, DeviceKind::Pen, 0).0);
    assert!(matches!(s.prepare(point(1, 1)), Action::Suppress));
    assert!(matches!(s.prepare(point(0, 2)), Action::Suppress));
    assert_eq!(packet(&mut s, point(1, 3)).raw.phase, Phase::Down);
}

#[test]
fn inactive_client_contact_stays_receiver_owned_across_focus_transition() {
    use paintcraft_input::tablet::{
        Permissions, Transform,
        receiver::{Delivery, Receiver, Route},
    };
    let mut s = state(0x400, DeviceKind::Pen);
    let mut receiver = Receiver::new(16, 16).unwrap();
    let transform = Transform {
        owner: 1,
        epoch: 1,
        origin: (0.0, 0.0),
        points_per_pixel: 1.0,
    };
    let mut permissions = Permissions {
        owner: 1,
        focused: false,
        canvas_hit: true,
        modal: false,
        ime: false,
    };
    // in_view is geometric client membership, even for the inactive window.
    let down = packet(&mut s, point(1, 1)).into_logical(1.0).unwrap();
    assert_eq!(
        receiver
            .submit(down, transform, permissions, Delivery::ReceiverPointer)
            .unwrap(),
        Route::Suppressed
    );
    permissions.focused = true;
    let held = packet(&mut s, point(1, 2)).into_logical(1.0).unwrap();
    assert_eq!(
        receiver
            .submit(held, transform, permissions, Delivery::ReceiverPointer)
            .unwrap(),
        Route::Canvas
    ); // The queue declines a Move with no accepted Down; no sample is emitted.
    let up = packet(&mut s, point(0, 3)).into_logical(1.0).unwrap();
    let _ = receiver.submit(up, transform, permissions, Delivery::ReceiverPointer);
    assert!(receiver.take(1, |_| Ok(())).unwrap().samples.is_empty());
    let new_down = packet(&mut s, point(1, 4)).into_logical(1.0).unwrap();
    assert_eq!(
        receiver
            .submit(new_down, transform, permissions, Delivery::ReceiverPointer)
            .unwrap(),
        Route::Canvas
    );
    assert_eq!(receiver.take(2, |_| Ok(())).unwrap().samples.len(), 1);
    // Actual titlebar geometry retains the native route regardless of focus.
    let mut chrome = state(0x400, DeviceKind::Pen);
    assert!(matches!(
        chrome.prepare(Point {
            in_view: false,
            ..point(1, 1)
        }),
        Action::Native
    ));
}

#[test]
fn native_gesture_mapping_fences_held_pen_before_up_without_inventing_mouse_motion() {
    for kind in [
        NSEventType::Magnify,
        NSEventType::SmartMagnify,
        NSEventType::Rotate,
        NSEventType::Swipe,
    ] {
        let collector = Arc::new(Mutex::new(
            Collector::new(
                16,
                &paintcraft_core::types::resources::SharedBudget::new(1 << 20),
            )
            .unwrap(),
        ));
        let receive = collector.clone();
        let emitter = Emitter::new_records(
            Arc::new(move |record| receive.lock().unwrap().accept(record)),
            Arc::new(|| {}),
            Report::new(Backend::AppKit),
        );
        emitter.record_epoch.store(19, Ordering::Release);
        let mut source = state(0x400, DeviceKind::Pen);
        emitter.emit(packet(&mut source, point(1, 1)));
        let boundary = navigation_boundary(kind, None).unwrap();
        assert_eq!(
            boundary,
            CaptureEvent::Boundary {
                kind: Boundary::Navigation,
                time: None
            }
        );
        emitter.observe(boundary);
        emitter.emit(packet(&mut source, point(1, 2)));
        emitter.emit(packet(&mut source, point(0, 3)));
        let frame = emitter
            .gate
            .with_delivery(|health| collector.lock().unwrap().take(health).unwrap());
        assert!(matches!(
            frame.records[1],
            Collected::Boundary(CaptureRecord {
                event: CaptureEvent::Boundary {
                    kind: Boundary::Navigation,
                    ..
                },
                ..
            })
        ));
        let routes: Vec<_> = frame
            .records
            .iter()
            .filter_map(|record| match record {
                Collected::Tablet { route, .. } => Some(*route),
                _ => None,
            })
            .collect();
        assert_eq!(
            routes,
            [
                PenRoute::Admitted,
                PenRoute::Suppressed,
                PenRoute::Suppressed
            ]
        );
        assert!(
            !frame
                .records
                .iter()
                .any(|record| matches!(record, Collected::Mouse { .. }))
        );
    }
    assert!(navigation_boundary(NSEventType::MouseEntered, None).is_none());
}
