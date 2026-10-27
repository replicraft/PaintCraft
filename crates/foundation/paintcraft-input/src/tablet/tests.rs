use super::*;
fn sample(sequence: u64, phase: Phase) -> Sample {
    Sample {
        identity: Identity {
            device: 7,
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
        position: (3.25, 4.5),
        transform: Transform {
            owner: 5,
            epoch: 2,
            origin: (10.0, 20.0),
            points_per_pixel: 2.0,
        },
        buttons: Buttons(1),
        modifiers: Modifiers::default(),
        pressure: None,
        tilt: None,
        twist_degrees: None,
        contact: None,
        provenance: Provenance::Actual,
    }
}
#[test]
fn missing_and_zero_pressure_are_distinct_and_invalid_data_is_refused() {
    let mut s = sample(1, Phase::Down);
    s.validate().unwrap();
    s.pressure = Some(0.0);
    s.validate().unwrap();
    assert_ne!(s.pressure, sample(1, Phase::Down).pressure);
    s.pressure = Some(f32::NAN);
    assert_eq!(s.validate(), Err(Invalid::Pressure));
}
#[test]
fn predictions_and_promoted_mouse_cannot_commit_or_join_owned_pen() {
    let mut q = Queue::new(8).unwrap();
    q.push(sample(1, Phase::Down)).unwrap();
    let mut predicted = sample(2, Phase::Move);
    predicted.provenance = Provenance::Predicted;
    assert_eq!(q.push(predicted).unwrap(), Admission::PreviewOnly);
    let mut mouse = sample(2, Phase::Move);
    mouse.identity.kind = DeviceKind::Mouse;
    assert_eq!(q.push(mouse).unwrap(), Admission::Busy);
    q.push(sample(3, Phase::Up)).unwrap();
    let records = q.take(1).unwrap();
    assert_eq!(records.len(), 2);
    assert!(q.take(1).unwrap().is_empty());
}
#[test]
fn capacity_failure_has_explicit_cancel_and_recovery_preserves_boundaries() {
    let mut q = Queue::new(3).unwrap();
    q.push(sample(1, Phase::Down)).unwrap();
    q.push(sample(2, Phase::Move)).unwrap();
    assert_eq!(q.push(sample(3, Phase::Move)), Err(Invalid::Capacity));
    let values = q.take(1).unwrap();
    assert_eq!(values.len(), 3);
    assert_eq!(values[2].phase, Phase::Cancel);
    assert_eq!(q.push(sample(4, Phase::Up)).unwrap(), Admission::Busy);
    q.push(sample(5, Phase::Down)).unwrap();
    assert_eq!(q.take(2).unwrap().len(), 1);
}
#[test]
fn source_clocks_and_order_cannot_be_silently_merged() {
    let mut q = Queue::new(8).unwrap();
    q.push(sample(2, Phase::Down)).unwrap();
    assert_eq!(q.push(sample(1, Phase::Move)), Err(Invalid::Order));
    let mut q = Queue::new(8).unwrap();
    q.push(sample(1, Phase::Down)).unwrap();
    let mut changed = sample(2, Phase::Move);
    changed.time.clock = Clock::HostReceipt;
    assert_eq!(q.push(changed), Err(Invalid::Clock));
}
#[test]
fn delayed_samples_keep_the_capture_transform_and_fractional_position() {
    let old = Transform {
        owner: 4,
        epoch: 1,
        origin: (10.0, 20.0),
        points_per_pixel: 2.0,
    };
    let new = Transform {
        epoch: 2,
        points_per_pixel: 4.0,
        ..old
    };
    let logical = (17.0, 29.0);
    let mut s = sample(1, Phase::Down);
    s.transform = old;
    s.position = old.document(logical).unwrap();
    assert_eq!(s.position, (3.5, 4.5));
    assert_ne!(s.position, new.document(logical).unwrap());
    assert_eq!(s.q8().unwrap(), (896, 1152));
}

#[test]
fn coalesced_duplicates_are_removed_but_stationary_pressure_changes_remain() {
    let mut q = Queue::new(8).unwrap();
    let down = sample(1, Phase::Down);
    q.push(down).unwrap();
    let mut duplicate = down;
    duplicate.provenance = Provenance::Coalesced;
    assert_eq!(q.push(duplicate).unwrap(), Admission::Duplicate);
    let mut pressure = sample(2, Phase::Move);
    pressure.time = down.time;
    pressure.pressure = Some(0.0);
    q.push(pressure).unwrap();
    assert_eq!(q.take(1).unwrap().len(), 2);
}

#[test]
fn rotation_and_contact_changes_are_not_coalesced_away() {
    let mut q = Queue::new(8).unwrap();
    q.push(sample(1, Phase::Down)).unwrap();
    let mut s = sample(2, Phase::Move);
    s.time = sample(1, Phase::Down).time;
    q.push(s).unwrap();
    s.sequence = 3;
    s.twist_degrees = Some(90.0);
    q.push(s).unwrap();
    s.sequence = 4;
    s.contact = Some((3.0, 7.0));
    q.push(s).unwrap();
    assert_eq!(q.take(1).unwrap().len(), 4);
}
#[test]
fn invalid_active_input_emits_only_valid_monotonic_cancellation() {
    for kind in 0..4 {
        let mut q = Queue::new(8).unwrap();
        let start = sample(5, Phase::Down);
        q.push(start).unwrap();
        let mut bad = sample(6, Phase::Move);
        match kind {
            0 => bad.position.0 = f64::NAN,
            1 => bad.pressure = Some(-1.0),
            2 => bad.time.micros = 1,
            _ => bad.time.clock = Clock::XServer,
        };
        assert!(q.push(bad).is_err());
        let records = q.take(1).unwrap();
        let end = records.last().unwrap();
        end.validate().unwrap();
        assert_eq!(end.phase, Phase::Cancel);
        assert_eq!(end.provenance, Provenance::Boundary);
        assert_eq!(end.time.clock, start.time.clock);
        assert!(end.time.micros >= start.time.micros);
        assert!(end.sequence > start.sequence);
        q.cancel_capture();
        q.push(sample(10, Phase::Down)).unwrap();
    }
}
#[test]
fn distinct_repeated_down_cancels_and_fences_until_physical_release() {
    let mut q = Queue::new(8).unwrap();
    q.push(sample(1, Phase::Down)).unwrap();
    assert_eq!(q.push(sample(2, Phase::Down)), Err(Invalid::Lifecycle));
    assert_eq!(q.push(sample(3, Phase::Move)).unwrap(), Admission::Busy);
    assert_eq!(q.push(sample(4, Phase::Up)).unwrap(), Admission::Busy);
    q.push(sample(5, Phase::Down)).unwrap();
    let records = q.take(1).unwrap();
    assert_eq!(
        records.iter().map(|s| s.phase).collect::<Vec<_>>(),
        [Phase::Down, Phase::Cancel, Phase::Down]
    );
}
#[test]
fn failed_frame_admission_keeps_records_and_reserves_a_cancellation_slot() {
    assert!(Queue::new(1).is_err());
    assert!(Queue::new(65_538).is_err());
    let mut q = Queue::new(3).unwrap();
    q.push(sample(1, Phase::Down)).unwrap();
    q.push(sample(2, Phase::Move)).unwrap();
    assert_eq!(
        q.take_admitted(1, |_| Err(Invalid::Capacity)),
        Err(Invalid::Capacity)
    );
    let records = q.take(1).unwrap();
    assert_eq!(records.len(), 3);
    assert_eq!(records[2].phase, Phase::Cancel);
    assert!(q.take(1).unwrap().is_empty());
}

fn permissions() -> Permissions {
    Permissions {
        owner: 5,
        focused: true,
        canvas_hit: true,
        modal: false,
        ime: false,
    }
}
#[test]
fn modal_ime_focus_and_wrong_owner_cannot_paint_behind_ui() {
    for blocked in 0..4 {
        let mut router = Router::new(8).unwrap();
        router
            .submit(sample(1, Phase::Down), permissions())
            .unwrap();
        let mut p = permissions();
        match blocked {
            0 => p.modal = true,
            1 => p.ime = true,
            2 => p.focused = false,
            _ => p.owner = 99,
        };
        let _ = router.submit(sample(2, Phase::Move), p);
        let records = router.take(1).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[1].phase, Phase::Cancel);
        assert!(!router.pen_active());
    }
    let mut router = Router::new(8).unwrap();
    let mut p = permissions();
    p.canvas_hit = false;
    assert_eq!(
        router.submit(sample(1, Phase::Down), p).unwrap(),
        Admission::Busy
    );
    assert!(router.take(1).unwrap().is_empty());
}
#[test]
fn palm_tails_cannot_resume_as_a_finger_stroke_or_pinch_after_pen_up() {
    let mut router = Router::new(8).unwrap();
    assert!(!router.finger(1, Phase::Down).unwrap());
    router
        .submit(sample(1, Phase::Down), permissions())
        .unwrap();
    assert!(router.finger(1, Phase::Move).unwrap());
    assert!(router.finger(2, Phase::Down).unwrap());
    router.submit(sample(2, Phase::Up), permissions()).unwrap();
    assert!(router.finger(1, Phase::Up).unwrap());
    assert!(router.finger(2, Phase::Move).unwrap());
    assert!(router.finger(2, Phase::Up).unwrap());
    assert!(!router.finger(3, Phase::Down).unwrap());
}
#[test]
fn queue_stress_retains_actual_order_and_disconnect_ends_capture() {
    let mut router = Router::new(5000).unwrap();
    router
        .submit(sample(1, Phase::Down), permissions())
        .unwrap();
    for sequence in 2..=4096 {
        router
            .submit(sample(sequence, Phase::Move), permissions())
            .unwrap();
    }
    router.cancel();
    let records = router.take(1).unwrap();
    assert_eq!(records.len(), 4097);
    assert!(
        records
            .windows(2)
            .all(|pair| pair[1].sequence > pair[0].sequence)
    );
    assert_eq!(records.last().unwrap().phase, Phase::Cancel);
    assert!(router.take(1).unwrap().is_empty());
    router
        .submit(sample(5000, Phase::Down), permissions())
        .unwrap();
    assert!(router.pen_active());
}

#[test]
fn platform_capture_requires_real_coordinates_and_preserves_optional_attributes() {
    let base = sample(1, Phase::Down);
    let raw = RawSample {
        identity: base.identity,
        sequence: base.sequence,
        time: base.time,
        phase: base.phase,
        logical_position: Some((17.0, 29.0)),
        buttons: base.buttons,
        modifiers: base.modifiers,
        pressure: Some(0.0),
        tilt: Some(Tilt::Normalized(0.5, -0.25)),
        twist_degrees: Some(95.0),
        contact: None,
        provenance: Provenance::Coalesced,
    };
    let captured = raw.at(base.transform).unwrap();
    assert_eq!(captured.position, (3.5, 4.5));
    assert_eq!(captured.pressure, Some(0.0));
    assert_eq!(captured.tilt, raw.tilt);
    assert_eq!(captured.time, raw.time);
    assert_eq!(
        RawSample {
            logical_position: None,
            ..raw
        }
        .at(base.transform),
        Err(Invalid::Coordinates)
    );
}

#[test]
fn barrel_buttons_keep_distinct_identity_and_both_select_secondary_policy() {
    assert_ne!(Buttons::BARREL, Buttons::BARREL_UPPER);
    assert_eq!(
        Buttons(Buttons::BARREL).ink_button(),
        crate::PointerButton::Secondary
    );
    assert_eq!(
        Buttons(Buttons::BARREL_UPPER).ink_button(),
        crate::PointerButton::Secondary
    );
    assert_eq!(
        Buttons(Buttons::TIP).ink_button(),
        crate::PointerButton::Primary
    );
}
