use super::*;
fn raw(sequence: u64, phase: Phase, buttons: u16) -> RawSample {
    RawSample {
        identity: Identity {
            device: 2,
            pointer: 3,
            kind: DeviceKind::Pen,
        },
        sequence,
        time: Time {
            epoch: 1,
            micros: sequence * 100,
            clock: Clock::BrowserPerformance,
        },
        phase,
        logical_position: Some((10.0 + sequence as f64, 12.0)),
        buttons: Buttons(buttons),
        modifiers: Modifiers::default(),
        pressure: Some(0.5),
        tilt: None,
        twist_degrees: None,
        contact: None,
        provenance: Provenance::Actual,
    }
}
fn view() -> Transform {
    Transform {
        owner: 5,
        epoch: 7,
        origin: (2.0, 4.0),
        points_per_pixel: 2.0,
    }
}
fn permissions(canvas_hit: bool) -> Permissions {
    Permissions {
        owner: 5,
        focused: true,
        canvas_hit,
        modal: false,
        ime: false,
    }
}
fn push(r: &mut Receiver, s: RawSample, p: Permissions) -> Route {
    r.submit(s, view(), p, Delivery::ReceiverPointer).unwrap()
}
#[test]
fn canvas_contact_is_not_promoted_and_frame_replays_cannot_duplicate_it() {
    let mut r = Receiver::new(32, 32).unwrap();
    assert_eq!(
        push(&mut r, raw(1, Phase::Down, Buttons::TIP), permissions(true)),
        Route::Canvas
    );
    let changed = Transform {
        epoch: 8,
        origin: (50.0, 60.0),
        ..view()
    };
    r.submit(
        raw(2, Phase::Move, Buttons::TIP),
        changed,
        permissions(true),
        Delivery::ReceiverPointer,
    )
    .unwrap();
    push(&mut r, raw(3, Phase::Up, 0), permissions(true));
    let frame = r.take(1, |_| Ok(())).unwrap();
    assert_eq!(frame.samples.len(), 3);
    assert!(frame.ui.is_empty());
    assert!(frame.suppress_legacy_pointer);
    assert_eq!(frame.samples[0].position, (4.5, 4.0));
    assert_eq!(frame.samples[0].transform.epoch, 7);
    assert_eq!(frame.samples[1].position, (-19.0, -24.0));
    assert_eq!(frame.samples[1].transform.epoch, 8);
    assert!(
        r.take(1, |_| panic!("same frame cannot re-admit"))
            .unwrap()
            .samples
            .is_empty()
    );
}
#[test]
fn hover_barrel_is_owned_without_ink_then_contact_captures_secondary_color_policy() {
    let mut r = Receiver::new(32, 32).unwrap();
    assert_eq!(
        push(
            &mut r,
            raw(1, Phase::Hover, Buttons::BARREL_UPPER),
            permissions(true)
        ),
        Route::Canvas
    );
    assert!(!r.pen_active());
    assert!(r.take(1, |_| Ok(())).unwrap().samples.is_empty());
    push(
        &mut r,
        raw(2, Phase::Down, Buttons::TIP | Buttons::BARREL_UPPER),
        permissions(true),
    );
    push(
        &mut r,
        raw(3, Phase::Up, Buttons::BARREL_UPPER),
        permissions(true),
    );
    push(&mut r, raw(4, Phase::Hover, 0), permissions(true));
    let f = r.take(2, |_| Ok(())).unwrap();
    assert_eq!(f.samples.len(), 2);
    assert!(f.ui.is_empty());
    assert_eq!(f.samples[0].buttons.ink_button(), PointerButton::Secondary);
}
#[test]
fn ui_button_ownership_stays_with_ui_even_after_crossing_canvas_and_contact_changes() {
    let mut r = Receiver::new(32, 32).unwrap();
    assert_eq!(
        push(
            &mut r,
            raw(1, Phase::Hover, Buttons::BARREL),
            permissions(false)
        ),
        Route::Ui
    );
    assert_eq!(
        push(
            &mut r,
            raw(2, Phase::Down, Buttons::TIP | Buttons::BARREL),
            permissions(true)
        ),
        Route::Ui
    );
    push(
        &mut r,
        raw(3, Phase::Up, Buttons::BARREL),
        permissions(true),
    );
    push(&mut r, raw(4, Phase::Hover, 0), permissions(true));
    let f = r.take(1, |_| Ok(())).unwrap();
    assert!(f.samples.is_empty());
    assert!(f.suppress_legacy_pointer);
    let buttons: Vec<_> =
        f.ui.iter()
            .filter_map(|e| {
                if let UiEvent::Button {
                    button, pressed, ..
                } = e
                {
                    Some((*button, *pressed))
                } else {
                    None
                }
            })
            .collect();
    assert_eq!(
        buttons,
        [
            (PointerButton::Secondary, true),
            (PointerButton::Primary, true),
            (PointerButton::Primary, false),
            (PointerButton::Secondary, false)
        ]
    );
}
#[test]
fn eraser_identity_while_hovering_does_not_hold_a_ui_button_or_capture_forever() {
    let mut r = Receiver::new(32, 32).unwrap();
    let eraser = |sequence, phase| RawSample {
        identity: Identity {
            kind: DeviceKind::Eraser,
            ..raw(1, Phase::Hover, 0).identity
        },
        ..raw(sequence, phase, Buttons::ERASER)
    };
    push(&mut r, eraser(1, Phase::Hover), permissions(false));
    push(&mut r, eraser(2, Phase::Down), permissions(false));
    push(&mut r, eraser(3, Phase::Up), permissions(false));
    let f = r.take(1, |_| Ok(())).unwrap();
    let buttons: Vec<_> =
        f.ui.iter()
            .filter_map(|e| {
                if let UiEvent::Button {
                    button, pressed, ..
                } = e
                {
                    Some((*button, *pressed))
                } else {
                    None
                }
            })
            .collect();
    assert_eq!(
        buttons,
        [
            (PointerButton::Primary, true),
            (PointerButton::Primary, false)
        ]
    );
    assert!(r.mouse_capture(true));
}
#[test]
fn modal_ime_owner_focus_and_positionless_cancel_retire_private_canvas_capture() {
    for (p, expected) in [
        (
            Permissions {
                modal: true,
                ..permissions(true)
            },
            None,
        ),
        (
            Permissions {
                ime: true,
                ..permissions(true)
            },
            None,
        ),
        (
            Permissions {
                owner: 99,
                ..permissions(true)
            },
            Some(Invalid::Owner),
        ),
        (
            Permissions {
                focused: false,
                ..permissions(true)
            },
            None,
        ),
    ] {
        let mut r = Receiver::new(32, 32).unwrap();
        push(&mut r, raw(1, Phase::Down, Buttons::TIP), permissions(true));
        let result = r.submit(
            raw(2, Phase::Move, Buttons::TIP),
            view(),
            p,
            Delivery::ReceiverPointer,
        );
        if let Some(error) = expected {
            assert_eq!(result.unwrap_err(), error);
        } else {
            result.unwrap();
        }
        let f = r.take(1, |_| Ok(())).unwrap();
        assert!(f.cancelled);
        assert!(f.samples.is_empty());
        assert!(!r.pen_active());
    }
    let mut r = Receiver::new(32, 32).unwrap();
    push(&mut r, raw(1, Phase::Down, Buttons::TIP), permissions(true));
    let cancel = RawSample {
        logical_position: None,
        ..raw(2, Phase::Cancel, 0)
    };
    push(&mut r, cancel, permissions(true));
    let f = r.take(1, |_| Ok(())).unwrap();
    assert!(f.cancelled);
    assert!(f.samples.is_empty());
}
#[test]
fn palm_tails_mouse_capture_and_predictions_do_not_become_ink() {
    let mut r = Receiver::new(32, 32).unwrap();
    assert!(r.mouse_capture(true));
    assert_eq!(
        push(&mut r, raw(1, Phase::Down, Buttons::TIP), permissions(true)),
        Route::Suppressed
    );
    assert!(r.mouse_capture(false));
    assert!(!r.finger(8, Phase::Down).unwrap());
    push(&mut r, raw(2, Phase::Down, Buttons::TIP), permissions(true));
    assert!(!r.mouse_capture(true));
    assert!(r.finger(8, Phase::Move).unwrap());
    let predicted = RawSample {
        provenance: Provenance::Predicted,
        ..raw(3, Phase::Up, 0)
    };
    assert_eq!(
        push(&mut r, predicted, permissions(true)),
        Route::Suppressed
    );
    assert!(r.pen_active());
    let predicted_cancel = RawSample {
        provenance: Provenance::Predicted,
        logical_position: None,
        ..raw(3, Phase::Cancel, 0)
    };
    assert_eq!(
        push(&mut r, predicted_cancel, permissions(true)),
        Route::Suppressed
    );
    assert!(r.pen_active());
    push(&mut r, raw(4, Phase::Up, 0), permissions(true));
    assert!(r.finger(8, Phase::Move).unwrap());
    assert!(r.finger(8, Phase::Up).unwrap());
    assert!(!r.finger(9, Phase::Down).unwrap());
    let f = r.take(1, |_| Ok(())).unwrap();
    assert_eq!(f.samples.len(), 2);
    assert!(
        f.samples
            .iter()
            .all(|s| s.provenance != Provenance::Predicted)
    );
}
#[test]
fn ui_capacity_and_admission_failure_keep_guaranteed_button_release_slots() {
    let mut r = Receiver::new(8, 3).unwrap();
    push(
        &mut r,
        raw(1, Phase::Down, Buttons::TIP | Buttons::BARREL),
        permissions(false),
    );
    assert_eq!(
        r.submit(
            raw(2, Phase::Move, Buttons::TIP | Buttons::BARREL),
            view(),
            permissions(false),
            Delivery::ReceiverPointer
        )
        .unwrap_err(),
        Invalid::Capacity
    );
    let f = r.take(1, |_| Ok(())).unwrap();
    assert!(f.cancelled);
    assert!(matches!(f.ui.last(), Some(UiEvent::Gone)));
    assert_eq!(
        f.ui.iter()
            .filter(|e| matches!(e, UiEvent::Button { pressed: true, .. }))
            .count(),
        0
    );
    assert_eq!(
        f.ui.iter()
            .filter(|e| matches!(e, UiEvent::Button { pressed: false, .. }))
            .count(),
        2
    );
    let mut r = Receiver::new(8, 8).unwrap();
    push(&mut r, raw(1, Phase::Down, Buttons::TIP), permissions(true));
    assert_eq!(
        r.take(1, |_| Err(Invalid::Capacity)).unwrap_err(),
        Invalid::Capacity
    );
    let f = r.take(2, |_| Ok(())).unwrap();
    assert!(f.cancelled);
    assert!(f.samples.is_empty());
}
#[test]
fn sink_failure_after_partial_up_discards_unpublished_source_until_explicit_reset() {
    let mut r = Receiver::new(32, 32).unwrap();
    push(&mut r, raw(1, Phase::Down, Buttons::TIP), permissions(true));
    push(&mut r, raw(2, Phase::Up, 0), permissions(true));
    // Native delivery-health gate observes a sink panic after this partial enqueue.
    r.fault();
    let f = r.take(1, |_| Ok(())).unwrap();
    assert!(f.cancelled);
    assert!(f.samples.is_empty());
    assert_eq!(
        push(&mut r, raw(3, Phase::Down, Buttons::TIP), permissions(true)),
        Route::Suppressed
    );
    r.reset();
    push(&mut r, raw(4, Phase::Down, Buttons::TIP), permissions(true));
    assert_eq!(r.take(2, |_| Ok(())).unwrap().samples.len(), 1);
}
#[test]
fn system_ui_delivery_stays_with_normal_backend_without_duplicate_ui_events() {
    let mut r = Receiver::new(32, 32).unwrap();
    let route = r
        .submit(
            raw(1, Phase::Down, Buttons::TIP),
            view(),
            permissions(false),
            Delivery::SystemPointer,
        )
        .unwrap();
    assert_eq!(route, Route::System);
    assert!(!route.consume());
    assert!(r.take(1, |_| Ok(())).unwrap().ui.is_empty());
}

#[test]
fn repeated_cancel_never_replays_unpublished_ui_presses_or_loses_required_releases() {
    let mut r = Receiver::new(32, 32).unwrap();
    push(
        &mut r,
        raw(1, Phase::Down, Buttons::TIP | Buttons::BARREL),
        permissions(false),
    );
    let pressed = r.take(1, |_| Ok(())).unwrap();
    assert_eq!(
        pressed
            .ui
            .iter()
            .filter(|e| matches!(e, UiEvent::Button { pressed: true, .. }))
            .count(),
        2
    );
    r.cancel();
    push(&mut r, raw(2, Phase::Hover, 0), permissions(false));
    r.cancel();
    r.cancel();
    let cancelled = r.take(2, |_| Ok(())).unwrap();
    assert!(cancelled.cancelled);
    assert!(
        !cancelled
            .ui
            .iter()
            .any(|e| matches!(e, UiEvent::Button { pressed: true, .. }))
    );
    assert_eq!(
        cancelled
            .ui
            .iter()
            .filter(|e| matches!(e, UiEvent::Button { pressed: false, .. }))
            .count(),
        2
    );
    assert!(matches!(cancelled.ui.last(), Some(UiEvent::Gone)));
}

#[test]
fn ordinary_cancel_discards_even_an_unpublished_completed_gesture_before_fresh_input() {
    let mut r = Receiver::new(32, 32).unwrap();
    push(&mut r, raw(1, Phase::Down, Buttons::TIP), permissions(true));
    push(&mut r, raw(2, Phase::Up, 0), permissions(true));
    r.cancel();
    let cancelled = r.take(1, |_| Ok(())).unwrap();
    assert!(cancelled.cancelled);
    assert!(cancelled.samples.is_empty());
    push(&mut r, raw(3, Phase::Down, Buttons::TIP), permissions(true));
    assert_eq!(r.take(2, |_| Ok(())).unwrap().samples.len(), 1);
}
#[test]
fn held_ui_pen_and_barrel_fence_existing_new_and_stationary_finger_tails() {
    for (phase, buttons) in [(Phase::Down, Buttons::TIP), (Phase::Hover, Buttons::BARREL)] {
        let mut r = Receiver::new(32, 32).unwrap();
        assert!(!r.finger(1, Phase::Down).unwrap());
        push(&mut r, raw(1, phase, buttons), permissions(false));
        assert!(r.finger(2, Phase::Down).unwrap());
        push(&mut r, raw(2, Phase::Up, 0), permissions(false));
        assert!(r.finger(1, Phase::Move).unwrap()); // no movement while the pen was held
        assert!(r.finger(2, Phase::Move).unwrap());
        assert!(r.finger(1, Phase::Up).unwrap());
        assert!(r.finger(2, Phase::Up).unwrap());
        push(&mut r, raw(3, Phase::Hover, 0), permissions(false));
        assert!(!r.finger(3, Phase::Down).unwrap());
    }
}

#[test]
fn held_stream_suppresses_competing_pointer_even_without_new_source_records() {
    for canvas in [false, true] {
        let mut r = Receiver::new(32, 32).unwrap();
        push(
            &mut r,
            raw(1, Phase::Down, Buttons::TIP),
            permissions(canvas),
        );
        assert!(r.take(1, |_| Ok(())).unwrap().suppress_legacy_pointer);
        let idle = r.take(2, |_| Ok(())).unwrap();
        assert!(idle.samples.is_empty());
        assert!(idle.ui.is_empty());
        assert!(idle.suppress_legacy_pointer);
        push(&mut r, raw(2, Phase::Up, 0), permissions(canvas));
        assert!(r.take(3, |_| Ok(())).unwrap().suppress_legacy_pointer);
        assert!(!r.take(4, |_| Ok(())).unwrap().suppress_legacy_pointer);
    }
}
