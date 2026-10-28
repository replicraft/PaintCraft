use super::*;
use paintcraft_input::{
    Modifiers,
    tablet::{Buttons, Clock, DeviceKind, Identity, Provenance, Time, receiver::UiEvent},
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
            .map(|record| &record.event)
            .filter(|e| matches!(e, UiEvent::Button { pressed: true, .. }))
            .count(),
        1
    );
    assert_eq!(
        batch
            .frame
            .ui
            .iter()
            .map(|record| &record.event)
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

#[test]
fn ui_slider_survives_same_session_document_and_canvas_changes() {
    let budget = SharedBudget::new(1_000_000);
    let source = source(&budget);
    let mut ui = view();
    ui.exclusions[0] = Some(ui.canvas);
    source.publish(ui).unwrap();
    assert_eq!(
        source
            .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(1, Phase::Down)))
            .unwrap(),
        Route::Ui
    );
    let first = source.take(1, ui.owner, false).unwrap();
    assert!(
        first
            .frame
            .ui
            .iter()
            .map(|record| &record.event)
            .any(|e| matches!(e, UiEvent::Button { pressed: true, .. }))
    );
    drop(first);
    ui.owner.state += 1;
    ui.owner.publication += 1;
    ui.transform.origin = (80.0, 90.0);
    ui.transform.points_per_pixel = 1.25;
    ui.transform.epoch += 1;
    source.publish(ui).unwrap();
    let mut motion = raw(2, Phase::Move);
    motion.logical_position = Some((30.0, 40.0));
    source
        .submit_with(Delivery::ReceiverPointer, |_| Ok(motion))
        .unwrap();
    let mut newer = ui.owner;
    newer.state += 1;
    newer.publication += 1;
    let moved = source.take(2, newer, false).unwrap();
    assert!(!moved.frame.cancelled);
    assert!(moved.frame.samples.is_empty());
    assert!(
        moved
            .frame
            .ui
            .iter()
            .map(|record| &record.event)
            .any(|e| matches!(e, UiEvent::Move { .. }))
    );
    drop(moved);
    source
        .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(3, Phase::Up)))
        .unwrap();
    let released = source.take(3, newer, false).unwrap();
    assert!(!released.frame.cancelled);
    assert!(
        released
            .frame
            .ui
            .iter()
            .map(|record| &record.event)
            .any(|e| matches!(e, UiEvent::Button { pressed: false, .. }))
    );
}

#[test]
fn ui_capture_retires_for_session_interface_focus_and_physical_display_changes() {
    for mode in 0..4 {
        let budget = SharedBudget::new(1_000_000);
        let source = source(&budget);
        let mut ui = view();
        ui.exclusions[0] = Some(ui.canvas);
        source.publish(ui).unwrap();
        source
            .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(1, Phase::Down)))
            .unwrap();
        drop(source.take(1, ui.owner, false).unwrap());
        match mode {
            0 => ui.owner.session += 1,
            1 => ui.owner.interface += 1,
            2 => ui.focused = false,
            _ => ui.pixels_per_point *= 2.0,
        }
        source.publish(ui).unwrap();
        let batch = source.take(2, ui.owner, false).unwrap();
        assert!(batch.frame.cancelled, "mode {mode}");
        assert!(
            batch
                .frame
                .ui
                .iter()
                .map(|record| &record.event)
                .all(|e| !matches!(e, UiEvent::Button { pressed: true, .. }))
        );
    }
}

#[test]
fn completed_unpublished_ui_click_cannot_cross_session_interface_or_focus_change() {
    for mode in 0..3 {
        let budget = SharedBudget::new(1_000_000);
        let source = source(&budget);
        let mut ui = view();
        ui.exclusions[0] = Some(ui.canvas);
        source.publish(ui).unwrap();
        source
            .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(1, Phase::Down)))
            .unwrap();
        source
            .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(2, Phase::Up)))
            .unwrap();
        match mode {
            0 => ui.owner.session += 1,
            1 => ui.owner.interface += 1,
            _ => ui.focused = false,
        }
        source.publish(ui).unwrap();
        let batch = source.take(1, ui.owner, false).unwrap();
        assert!(batch.frame.cancelled, "mode {mode}");
        assert!(batch.frame.samples.is_empty());
        assert!(
            batch
                .frame
                .ui
                .iter()
                .map(|record| &record.event)
                .all(|e| !matches!(e, UiEvent::Button { pressed: true, .. })),
            "mode {mode}"
        );
    }
}

#[test]
fn drained_batch_remains_retired_after_cancel_invalidation_and_restored_view() {
    for mode in 0..10 {
        let budget = SharedBudget::new(1_000_000);
        let source = source(&budget);
        push(&source, 1, Phase::Down);
        push(&source, 2, Phase::Up);
        let mut batch = source.take(1, view().owner, false).unwrap();
        assert!(source.batch_is_current(&batch, view().owner));
        match mode {
            0 => source.cancel(),
            1 => source.invalidate_view(),
            2 => source.fault(),
            8 | 9 => {
                let mut foreign = view().owner;
                if mode == 8 {
                    foreign.session += 1;
                } else {
                    foreign.state += 1;
                }
                drop(source.take(2, foreign, false).unwrap());
            }
            _ => {
                let mut changed = view();
                match mode {
                    3 => changed.owner.session += 1,
                    4 => changed.focused = false,
                    5 => changed.transform.origin.0 += 1.0,
                    6 => changed.owner.state += 1,
                    _ => changed.blocked = true,
                }
                source.publish(changed).unwrap();
            }
        }
        source.publish(view()).unwrap();
        // Public payload edits cannot erase the admission's canvas classification.
        batch.frame.samples.clear();
        assert!(
            !source.batch_is_current(&batch, view().owner),
            "mode {mode}"
        );
    }
}

#[test]
fn drained_ui_batch_keeps_workspace_policy_but_not_cross_source_or_cancel_authority() {
    let budget = SharedBudget::new(1_000_000);
    let source = source(&budget);
    let mut ui = view();
    ui.exclusions[0] = Some(ui.canvas);
    source.publish(ui).unwrap();
    for (n, phase) in [(1, Phase::Down), (2, Phase::Up)] {
        source
            .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(n, phase)))
            .unwrap();
    }
    let batch = source.take(1, ui.owner, false).unwrap();
    ui.owner.state += 1;
    ui.owner.publication += 1;
    ui.transform.origin.0 += 20.0;
    ui.transform.points_per_pixel *= 2.0;
    source.publish(ui).unwrap();
    assert!(source.batch_is_current(&batch, ui.owner));
    let foreign = Source::new(16, 16, &budget).unwrap();
    foreign.publish(ui).unwrap();
    assert!(!foreign.batch_is_current(&batch, ui.owner));
    source.cancel();
    source.publish(ui).unwrap();
    assert!(!source.batch_is_current(&batch, ui.owner));
}

fn predecessor() -> Predecessor {
    let old = view();
    let mut after = old.owner;
    after.state += 1;
    after.publication += 1;
    Predecessor {
        before: old.owner,
        after,
        sequence: 1,
        transform: old.transform,
        canvas: old.canvas,
        pixels_per_point: old.pixels_per_point,
        focused: true,
        modal: false,
        ime: false,
    }
}

#[test]
fn predecessor_requires_exact_admission_and_one_earlier_receipt_step() {
    for mode in 0..12 {
        let budget = SharedBudget::new(1_000_000);
        let source = source(&budget);
        push(&source, 2, Phase::Down);
        push(&source, 3, Phase::Up);
        let mut batch = source.take(1, view().owner, false).unwrap();
        let mut proof = predecessor();
        match mode {
            0 => {}
            1 => proof.before.state += 1,
            2 => proof.after.session += 1,
            3 => proof.after.interface += 1,
            4 => proof.after.publication += 1,
            5 => proof.after = proof.before, // no-op never acknowledges a receipt
            6 => proof.transform.origin.0 += 1.0,
            7 => proof.canvas.max.x += 1.0,
            8 => proof.pixels_per_point *= 2.0,
            9 => proof.modal = true,
            10 => proof.focused = false,
            _ => proof.ime = true,
        }
        let result = source.acknowledge_predecessor(&mut batch, proof);
        assert_eq!(result.is_ok(), mode == 0, "mode {mode}");
        if mode == 0 {
            assert!(source.batch_is_current(&batch, proof.after));
            assert!(source.acknowledge_predecessor(&mut batch, proof).is_err());
        }
    }
}

#[test]
fn predecessor_cannot_use_edited_payload_or_revive_cancelled_admission() {
    for mode in 0..3 {
        let budget = SharedBudget::new(1_000_000);
        let source = source(&budget);
        push(&source, 2, Phase::Down);
        push(&source, 3, Phase::Up);
        let mut batch = source.take(1, view().owner, false).unwrap();
        let mut proof = predecessor();
        match mode {
            0 => {
                proof.sequence = 2;
                batch.frame.samples[0].sequence = 100;
            }
            1 => {
                source.cancel();
                source.publish(view()).unwrap();
            }
            _ => {
                source.invalidate_view();
                source.publish(view()).unwrap();
            }
        }
        assert!(source.acknowledge_predecessor(&mut batch, proof).is_err());
    }
}

#[test]
fn continuation_metadata_admits_only_monotonic_completion_before_a_later_contact() {
    let budget = SharedBudget::new(1_000_000);
    let source = source(&budget);
    for (sequence, phase) in [
        (2, Phase::Down),
        (3, Phase::Up),
        (4, Phase::Down),
        (5, Phase::Up),
    ] {
        push(&source, sequence, phase);
    }
    let mut batch = source.take(1, view().owner, false).unwrap();
    let first = predecessor();
    source.acknowledge_predecessor(&mut batch, first).unwrap();
    let mut second = first;
    second.before = first.after;
    second.after.state += 1;
    second.after.publication += 1;
    second.sequence = 3;
    source.acknowledge_predecessor(&mut batch, second).unwrap();
    assert!(source.batch_is_current(&batch, second.after));
    let mut terminal = second;
    terminal.before = second.after;
    terminal.after.state += 1;
    terminal.after.publication += 1;
    terminal.sequence = 5;
    assert_eq!(
        source.acknowledge_predecessor(&mut batch, terminal),
        Err(Invalid::Order)
    );
}

#[test]
fn host_retirement_epoch_is_observable_after_the_entire_contact_left_the_queue() {
    let budget = SharedBudget::new(1_000_000);
    let source = source(&budget);
    push(&source, 1, Phase::Down);
    push(&source, 2, Phase::Up);
    let batch = source.take(1, view().owner, false).unwrap();
    let before = source.cancellation_epoch();
    source.cancel();
    assert_eq!(source.cancellation_epoch(), before + 1);
    source.publish(view()).unwrap();
    assert_eq!(source.cancellation_epoch(), before + 1);
    assert!(!source.batch_is_current(&batch, view().owner));
}

#[test]
fn multiple_native_batches_continue_atomically_after_the_same_proven_receipt() {
    let budget = SharedBudget::new(1_000_000);
    let source = source(&budget);
    push(&source, 2, Phase::Down);
    push(&source, 3, Phase::Up);
    let mut first = source.take(1, view().owner, false).unwrap();
    push(&source, 4, Phase::Down);
    push(&source, 5, Phase::Up);
    let mut second = source.take(2, view().owner, false).unwrap();
    let proof = predecessor();
    source
        .acknowledge_predecessors(&mut [&mut first, &mut second], proof)
        .unwrap();
    assert!(source.batch_is_current(&first, proof.after));
    assert!(source.batch_is_current(&second, proof.after));
    assert!(!source.batch_is_current(&first, proof.before));
    assert_eq!(
        source.acknowledge_predecessors(&mut [], proof),
        Err(Invalid::Order)
    );
}
#[test]
fn any_foreign_stale_or_cancelled_member_rejects_without_partial_rebase() {
    for mode in 0..3 {
        let budget = SharedBudget::new(1_000_000);
        let source = source(&budget);
        push(&source, 2, Phase::Down);
        push(&source, 3, Phase::Up);
        let mut invalid = source.take(1, view().owner, false).unwrap();
        match mode {
            0 => {
                let foreign = super::tests::source(&budget);
                push(&foreign, 2, Phase::Down);
                push(&foreign, 3, Phase::Up);
                invalid = foreign.take(1, view().owner, false).unwrap();
            }
            1 => {
                let mut changed = view();
                changed.transform.origin.0 += 1.0;
                source.publish(changed).unwrap();
                source.publish(view()).unwrap();
            }
            _ => {
                source.cancel();
                source.publish(view()).unwrap();
            }
        }
        push(&source, 4, Phase::Down);
        push(&source, 5, Phase::Up);
        let mut valid = source.take(2, view().owner, false).unwrap();
        let proof = predecessor();
        assert!(source.batch_is_current(&valid, proof.before));
        assert_eq!(
            source.acknowledge_predecessors(&mut [&mut valid, &mut invalid], proof),
            Err(Invalid::Owner)
        );
        assert!(source.batch_is_current(&valid, proof.before));
        assert!(!source.batch_is_current(&valid, proof.after));
    }
}

#[test]
fn cancellation_or_changed_then_restored_view_revokes_retained_publication() {
    let budget = SharedBudget::new(1 << 20);
    let source = source(&budget);
    source
        .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(1, Phase::Down)))
        .unwrap();
    source
        .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(2, Phase::Up)))
        .unwrap();
    let batch = source.take(1, view().owner, false).unwrap();
    let guard = source.publication_guard(&batch, view().owner).unwrap();
    assert!(guard.is_current());
    source.cancel();
    source.publish(view()).unwrap();
    assert!(!guard.is_current());
    let fresh = source.ui_publication_guard(view().owner).unwrap();
    let mut changed = view();
    changed.pixels_per_point = 3.0;
    source.publish(changed).unwrap();
    source.publish(view()).unwrap();
    assert!(!fresh.is_current());
}
#[test]
fn guard_does_not_retain_retired_source_or_authorize_foreign_document() {
    let budget = SharedBudget::new(1 << 20);
    let source = source(&budget);
    let guard = source.ui_publication_guard(view().owner).unwrap();
    let mut changed = view();
    changed.owner.publication += 1;
    source.publish(changed).unwrap();
    assert!(!guard.is_current());
    let guard = source.ui_publication_guard(changed.owner).unwrap();
    drop(source);
    assert!(!guard.is_current());
}
