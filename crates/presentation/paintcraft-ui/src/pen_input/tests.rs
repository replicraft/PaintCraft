use super::*;
use paintcraft_input::{
    Modifiers,
    tablet::{Buttons, Clock, DeviceKind, Identity, Provenance, Time},
};

fn view() -> View {
    View {
        owner: Owner {
            session: 1,
            state: 2,
            publication: 3,
            interface: 1,
        },
        transform: Transform {
            owner: 1,
            epoch: 9,
            origin: (10.0, 20.0),
            points_per_pixel: 0.5,
        },
        pixels_per_point: 2.0,
        canvas: egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(100.0, 100.0)),
        exclusions: [None; 64],
        focused: true,
        blocked: false,
        ime: false,
    }
}
fn raw(sequence: u64, phase: Phase) -> RawSample {
    RawSample {
        identity: Identity {
            device: 1,
            pointer: 8,
            kind: DeviceKind::Pen,
        },
        sequence,
        time: Time {
            epoch: 1,
            micros: 100 * sequence,
            clock: Clock::BrowserPerformance,
        },
        phase,
        logical_position: Some((20.0, 30.0)),
        buttons: Buttons(if phase == Phase::Up { 0 } else { Buttons::TIP }),
        modifiers: Modifiers::default(),
        pressure: Some(0.4),
        tilt: None,
        twist_degrees: None,
        contact: None,
        provenance: Provenance::Actual,
    }
}
fn source(budget: &SharedBudget) -> Source {
    let source = Source::new(16, 16, budget).unwrap();
    source.publish(view()).unwrap();
    source
}
fn push(source: &Source, sequence: u64, phase: Phase) {
    assert_eq!(
        source
            .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(sequence, phase)))
            .unwrap(),
        Route::Canvas
    );
}

#[test]
fn exact_view_converts_physical_once_and_source_survives_handoff_until_drop() {
    let budget = SharedBudget::new(1_000_000);
    let source = source(&budget);
    let retained = budget.usage().reserved;
    source
        .submit_with(Delivery::ReceiverPointer, |ppp| {
            assert_eq!(ppp, 2.0);
            let mut sample = raw(1, Phase::Down);
            sample.logical_position = Some((40.0 / ppp, 60.0 / ppp));
            Ok(sample)
        })
        .unwrap();
    push(&source, 2, Phase::Up);
    let batch = source.take(1, view().owner, false).unwrap();
    assert_eq!(batch.frame.samples.len(), 2);
    assert_eq!(batch.frame.samples[0].position, (20.0, 20.0));
    assert_eq!(batch.frame.samples[0].transform, view().transform);
    drop(source);
    assert!(budget.usage().reserved > 0);
    assert!(budget.usage().reserved < retained);
    drop(batch);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn callback_failure_after_up_discards_all_unpublished_source_before_resolve() {
    let budget = SharedBudget::new(1_000_000);
    let source = source(&budget);
    push(&source, 1, Phase::Down);
    push(&source, 2, Phase::Up);
    // This is the host operation inside the native delivery-health gate; no
    // mask/controller call can observe the successfully queued prefix first.
    let batch = source.take(1, view().owner, true).unwrap();
    assert!(batch.frame.cancelled);
    assert!(batch.frame.samples.is_empty());
    assert!(batch.frame.suppress_legacy_pointer);
    assert_eq!(
        source
            .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(3, Phase::Down)))
            .unwrap(),
        Route::Suppressed
    );
}

#[test]
fn ownership_transform_focus_and_modal_retirement_never_publish_completed_prefix() {
    for mode in 0..6 {
        let budget = SharedBudget::new(1_000_000);
        let source = source(&budget);
        push(&source, 1, Phase::Down);
        push(&source, 2, Phase::Up);
        let mut changed = view();
        match mode {
            0 => changed.owner.state += 1,
            1 => changed.transform.epoch += 1,
            2 => changed.focused = false,
            3 => changed.blocked = true,
            4 => changed.ime = true,
            _ => changed.owner.interface += 1,
        }
        source.publish(changed).unwrap();
        let frame = source.take(1, changed.owner, false).unwrap();
        assert!(frame.frame.cancelled, "mode {mode}");
        assert!(frame.frame.samples.is_empty(), "mode {mode}");
    }
}

#[test]
fn fresh_owner_at_drain_and_invalid_display_do_not_reinterpret_old_positions() {
    let budget = SharedBudget::new(1_000_000);
    let source = source(&budget);
    push(&source, 1, Phase::Down);
    let mut owner = view().owner;
    owner.publication += 1;
    let batch = source.take(1, owner, false).unwrap();
    assert!(batch.frame.samples.is_empty());
    assert!(batch.frame.cancelled);
    let mut bad = view();
    bad.pixels_per_point = f64::NAN;
    assert_eq!(source.publish(bad), Err(Invalid::Coordinates));
    assert_eq!(
        source.submit_with(Delivery::ReceiverPointer, |_| panic!("no valid view")),
        Ok(Route::Suppressed)
    );
}

#[test]
fn exclusions_and_held_ui_pen_reject_palm_but_keep_modal_buttons_available() {
    let budget = SharedBudget::new(1_000_000);
    let source = source(&budget);
    let mut blocked = view();
    blocked.exclusions[0] = Some(blocked.canvas);
    blocked.blocked = true;
    source.publish(blocked).unwrap();
    assert_eq!(
        source
            .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(1, Phase::Down)))
            .unwrap(),
        Route::Ui
    );
    assert!(source.finger(15, Phase::Down).unwrap());
    // Unchanged modal layout must not retire its own UI press every frame.
    source.publish(blocked).unwrap();
    source
        .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(2, Phase::Up)))
        .unwrap();
    let batch = source.take(1, blocked.owner, false).unwrap();
    assert!(batch.frame.samples.is_empty());
    assert_eq!(
        batch
            .frame
            .ui
            .iter()
            .filter(|e| matches!(e, UiEvent::Button { pressed: true, .. }))
            .count(),
        1
    );
    assert_eq!(
        batch
            .frame
            .ui
            .iter()
            .filter(|e| matches!(e, UiEvent::Button { pressed: false, .. }))
            .count(),
        1
    );
    assert!(source.finger(15, Phase::Up).unwrap());
    assert!(!source.finger(16, Phase::Down).unwrap());
}

#[test]
fn predicted_cancel_cannot_retire_actual_capture() {
    let budget = SharedBudget::new(1_000_000);
    let source = source(&budget);
    push(&source, 1, Phase::Down);
    let mut prediction = raw(2, Phase::Cancel);
    prediction.provenance = Provenance::Predicted;
    source
        .submit_with(Delivery::ReceiverPointer, |_| Ok(prediction))
        .unwrap();
    push(&source, 3, Phase::Up);
    let batch = source.take(1, view().owner, false).unwrap();
    assert!(!batch.frame.cancelled);
    assert_eq!(batch.frame.samples.len(), 2);
}

#[test]
fn egui_multipass_does_not_clear_suppression_or_replay_source() {
    let budget = SharedBudget::new(1_000_000);
    let source = source(&budget);
    push(&source, 1, Phase::Down);
    push(&source, 2, Phase::Up);
    let batch = source.take(7, view().owner, false).unwrap();
    let mut pass = Pass::default();
    assert!(pass.begin(7, &batch.frame));
    let ctx = egui::Context::default();
    let mut passes = 0;
    let mut replayed = 0;
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let ctx = ui.ctx();
        passes += 1;
        let repeated = source.take(7, view().owner, false).unwrap();
        if pass.begin(7, &repeated.frame) {
            replayed += repeated.frame.samples.len();
        }
        assert!(pass.suppress_legacy_pointer());
        if passes == 1 {
            ctx.request_discard("pen multipass control");
        }
    });
    output.textures_delta.clear();
    assert_eq!(passes, 2);
    assert_eq!(replayed, 0);
    let next = source.take(8, view().owner, false).unwrap();
    assert!(pass.begin(8, &next.frame));
    assert!(!pass.suppress_legacy_pointer());
}

#[test]
fn admission_denial_precedes_install_and_frame_capacity_is_not_silently_borrowed() {
    assert!(Source::new(16, 16, &SharedBudget::new(1)).is_err());
    let budget = SharedBudget::new(1_000_000);
    assert!(Source::new(usize::MAX, usize::MAX, &budget).is_err());
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn owner_or_focus_change_without_pen_work_does_not_cancel_genuine_mouse() {
    let budget = SharedBudget::new(1_000_000);
    let source = source(&budget);
    let mut changed = view();
    changed.owner.state += 1;
    changed.focused = false;
    source.publish(changed).unwrap();
    let batch = source.take(1, changed.owner, false).unwrap();
    assert!(!batch.frame.cancelled);
    assert!(!batch.frame.suppress_legacy_pointer);
    let mut pass = Pass::default();
    assert!(pass.begin(1, &batch.frame));
    assert!(!pass.suppress_legacy_pointer());
}

#[test]
fn foreign_positionless_cancel_and_leave_cannot_cancel_the_owned_pen() {
    for phase in [Phase::Cancel, Phase::Leave] {
        let budget = SharedBudget::new(1_000_000);
        let source = source(&budget);
        push(&source, 1, Phase::Down);
        let mut foreign = raw(2, phase);
        foreign.identity.pointer += 1;
        foreign.logical_position = None;
        assert_eq!(
            source.submit_with(Delivery::ReceiverPointer, |_| Ok(foreign)),
            Ok(Route::Suppressed)
        );
        push(&source, 3, Phase::Up);
        let batch = source.take(1, view().owner, false).unwrap();
        assert_eq!(batch.frame.samples.len(), 2);
        assert!(!batch.frame.cancelled);
        drop(batch);
        push(&source, 4, Phase::Down);
        let mut owned = raw(5, phase);
        owned.logical_position = None;
        source
            .submit_with(Delivery::ReceiverPointer, |_| Ok(owned))
            .unwrap();
        let batch = source.take(2, view().owner, false).unwrap();
        assert!(batch.frame.cancelled);
        assert!(batch.frame.samples.is_empty());
    }
}
