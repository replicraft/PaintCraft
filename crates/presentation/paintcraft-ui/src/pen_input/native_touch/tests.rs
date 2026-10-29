use super::*;
#[path = "tests_active_retirement.rs"]
mod active_retirement;
use paintcraft_input::{
    Modifiers,
    tablet::{Buttons, Clock, DeviceKind},
};
fn view() -> View {
    View {
        owner: Owner {
            session: 11,
            state: 12,
            publication: 1,
            interface: 1,
        },
        transform: Transform {
            owner: 11,
            epoch: 1,
            origin: (0., 0.),
            points_per_pixel: 1.,
        },
        pixels_per_point: 2.,
        canvas: egui::Rect::from_min_max(egui::pos2(0., 0.), egui::pos2(100., 100.)),
        exclusions: [None; 64],
        focused: true,
        blocked: false,
        ime: false,
    }
}
fn source(cap: usize) -> Source {
    let s = Source::new_native_touch(cap, cap, &SharedBudget::new(8 << 20)).unwrap();
    s.publish(view()).unwrap();
    s
}
fn finger(ordinal: u64, pointer: u64, phase: Phase) -> NativeFingerMetadata {
    NativeFingerMetadata {
        installation: 7,
        ordinal,
        identity: Identity {
            device: 30,
            pointer,
            kind: DeviceKind::Finger,
        },
        phase,
        time: Time {
            epoch: 7,
            micros: 100 + ordinal,
            clock: Clock::WindowsPerformance,
        },
        provenance: Provenance::Actual,
        confidence: FingerConfidence::High,
        in_contact: matches!(phase, Phase::Down | Phase::Move),
        primary: pointer == 1,
    }
}
fn submit(s: &Source, m: NativeFingerMetadata) -> NativeTouchOutcome {
    s.submit_native_finger(m, NativeFingerAdmission::Candidate, |_| {
        Ok(Some((10., 10.)))
    })
    .unwrap()
}
fn admitted(out: NativeTouchOutcome) -> NativeTouchBatch {
    let NativeTouchOutcome::Admitted(batch) = out else {
        panic!("expected admitted touch: {out:?}")
    };
    batch
}
fn suppressed(out: NativeTouchOutcome) {
    assert!(matches!(out, NativeTouchOutcome::Suppressed));
}
fn pen(s: &Source, ordinal: u64, phase: Phase) -> Route {
    s.submit_native(
        Delivery::ReceiverPointer,
        RawSample {
            identity: Identity {
                device: 2,
                pointer: 3,
                kind: DeviceKind::Pen,
            },
            sequence: ordinal,
            time: Time {
                epoch: 7,
                micros: 100 + ordinal,
                clock: Clock::WindowsPerformance,
            },
            phase,
            logical_position: None,
            buttons: Buttons(if phase == Phase::Up { 0 } else { Buttons::TIP }),
            modifiers: Modifiers::default(),
            pressure: Some(0.5),
            tilt: None,
            twist_degrees: None,
            contact: None,
            provenance: Provenance::Actual,
        },
        |_| Ok(Some((10., 10.))),
    )
    .unwrap()
}
#[test]
fn physical_mouse_and_pen_capture_win_but_completed_work_does_not_block_touch() {
    let s = source(64);
    assert!(s.mouse_capture(true));
    suppressed(submit(&s, finger(0, 1, Phase::Down)));
    assert!(s.mouse_capture(false));
    suppressed(submit(&s, finger(1, 1, Phase::Move)));
    suppressed(submit(&s, finger(2, 1, Phase::Up)));
    let touch = admitted(submit(&s, finger(3, 1, Phase::Down)));
    assert!(s.native_touch_batch_is_current(&touch, view().owner));
    drop(submit(&s, finger(4, 1, Phase::Up)));
    let s = source(64);
    assert_eq!(pen(&s, 0, Phase::Down), Route::Canvas);
    suppressed(submit(&s, finger(1, 1, Phase::Down)));
    assert_eq!(pen(&s, 2, Phase::Up), Route::Canvas);
    // A completed source stroke is still queued here, not a physical hold.
    suppressed(submit(&s, finger(3, 1, Phase::Up)));
    let touch = admitted(submit(&s, finger(4, 1, Phase::Down)));
    let old_pen = s.take(1, view().owner, false).unwrap();
    let old_guard = s.publication_guard(&old_pen, view().owner).unwrap();
    s.retire_native_touches().unwrap();
    assert!(s.batch_is_current(&old_pen, view().owner));
    assert!(old_guard.is_current());
    assert!(!s.native_touch_batch_is_current(&touch, view().owner));
}
#[test]
fn touch_first_denies_competing_physical_streams_through_their_real_tail() {
    let s = source(64);
    let down = admitted(submit(&s, finger(0, 1, Phase::Down)));
    assert!(!s.mouse_capture(true));
    assert_eq!(pen(&s, 1, Phase::Down), Route::Suppressed);
    let up = admitted(submit(&s, finger(2, 1, Phase::Up)));
    assert!(s.mouse_capture(true));
    assert!(s.mouse_capture(false));
    assert_eq!(pen(&s, 3, Phase::Move), Route::Suppressed);
    assert_eq!(pen(&s, 4, Phase::Up), Route::Suppressed);
    assert_eq!(pen(&s, 5, Phase::Down), Route::Canvas);
    assert!(s.native_touch_batch_is_current(&down, view().owner));
    assert!(s.native_touch_batch_is_current(&up, view().owner));
}
#[test]
fn simultaneous_full_identities_keep_actual_primary_without_reassignment() {
    let s = source(64);
    let a = admitted(submit(&s, finger(0, 1, Phase::Down)));
    let b = admitted(submit(&s, finger(1, 2, Phase::Down)));
    let mut other = finger(2, 2, Phase::Down);
    other.identity.device = 31;
    let c = admitted(submit(&s, other));
    assert_ne!(b.metadata().identity, c.metadata().identity);
    assert!(!b.metadata().primary && !c.metadata().primary);
    drop(submit(&s, finger(3, 1, Phase::Up)));
    let remaining = admitted(submit(&s, finger(4, 2, Phase::Move)));
    assert!(!remaining.metadata().primary);
    assert!(!s.mouse_capture(true));
    assert!(s.native_touch_batch_is_current(&a, view().owner));
    let mut changed = finger(5, 2, Phase::Move);
    changed.primary = true;
    assert!(matches!(
        submit(&s, changed),
        NativeTouchOutcome::Cancelled(_)
    ));
    assert!(!s.native_touch_batch_is_current(&b, view().owner));
    assert!(s.native_touch_batch_is_current(&c, view().owner));
}
#[test]
fn touch_only_retirement_revokes_drained_batches_and_keeps_genuine_mouse_guards() {
    let s = source(64);
    let down = admitted(submit(&s, finger(0, 1, Phase::Down)));
    let up = admitted(submit(&s, finger(1, 1, Phase::Up)));
    let touch_guard = s.native_touch_publication_guard(&up, view().owner).unwrap();
    assert!(s.mouse_capture(true));
    let mouse_guard = s.ui_publication_guard(view().owner).unwrap();
    let cancelled = s.retire_native_touches().unwrap();
    assert_eq!(cancelled.iter().flatten().count(), 1);
    let cancel = cancelled.iter().flatten().next().unwrap();
    assert_eq!(cancel.down_ordinal(), 0);
    assert!(cancel.cause().is_none());
    assert!(!s.native_touch_batch_is_current(&down, view().owner));
    assert!(!touch_guard.is_current());
    assert!(mouse_guard.is_current());
    assert!(s.mouse_capture(true));
    s.cancel();
    assert!(!mouse_guard.is_current());
}
#[test]
fn confidence_withdrawal_retains_real_phase_and_does_not_steal_later_mouse() {
    let s = source(64);
    let down = admitted(submit(&s, finger(0, 1, Phase::Down)));
    let guard = s
        .native_touch_publication_guard(&down, view().owner)
        .unwrap();
    let mut withdrawal = finger(1, 1, Phase::Move);
    withdrawal.confidence = FingerConfidence::Unspecified;
    let outcome = s
        .submit_native_finger(withdrawal, NativeFingerAdmission::Candidate, |_| {
            panic!("no conversion on policy withdrawal")
        })
        .unwrap();
    let NativeTouchOutcome::Cancelled(cancel) = outcome else {
        panic!("cancel expected")
    };
    assert_eq!(cancel.cause().unwrap().phase, Phase::Move);
    assert_eq!(cancel.last_observed_position(), Some((10., 10.)));
    assert!(!guard.is_current());
    assert!(s.mouse_capture(true));
    let mouse = s.ui_publication_guard(view().owner).unwrap();
    suppressed(submit(&s, finger(2, 1, Phase::Move)));
    suppressed(submit(&s, finger(3, 1, Phase::Up)));
    assert!(mouse.is_current());
}
#[test]
fn unusable_view_and_terminal_metadata_do_not_invent_coordinates_or_recover_held_tails() {
    let s = source(64);
    s.invalidate_view();
    let call = |m| {
        s.submit_native_finger(m, NativeFingerAdmission::Candidate, |_| {
            panic!("metadata only")
        })
        .unwrap()
    };
    suppressed(call(finger(0, 1, Phase::Down)));
    s.publish(view()).unwrap();
    suppressed(call(finger(1, 1, Phase::Move)));
    suppressed(call(finger(2, 1, Phase::Up)));
    let down = admitted(submit(&s, finger(3, 1, Phase::Down)));
    let outcome = s
        .submit_native_finger(
            finger(4, 1, Phase::Up),
            NativeFingerAdmission::Candidate,
            |_| Ok(None),
        )
        .unwrap();
    let NativeTouchOutcome::Cancelled(cancel) = outcome else {
        panic!("missing terminal position retires presentation")
    };
    assert_eq!(cancel.cause().unwrap().phase, Phase::Up);
    assert_eq!(cancel.down_ordinal(), 3);
    assert!(!s.native_touch_batch_is_current(&down, view().owner));
    drop(admitted(submit(&s, finger(5, 1, Phase::Down))));
}
#[test]
fn actual_up_idle_leave_and_true_range_retirement_preserve_completed_opaque_work() {
    let s = source(64);
    let down = admitted(submit(&s, finger(0, 1, Phase::Down)));
    let up = admitted(submit(&s, finger(1, 1, Phase::Up)));
    suppressed(
        s.submit_native_finger(
            finger(2, 1, Phase::Leave),
            NativeFingerAdmission::Candidate,
            |_| panic!("idle leave metadata"),
        )
        .unwrap(),
    );
    s.retire_native_touch_identity(
        up.metadata().identity,
        7,
        3,
        Time {
            micros: 103,
            ..up.metadata().time
        },
    )
    .unwrap();
    let fresh = admitted(submit(&s, finger(4, 1, Phase::Down)));
    assert_ne!(down.down_ordinal(), fresh.down_ordinal());
    assert!(s.native_touch_batch_is_current(&up, view().owner));
}
#[test]
fn source_context_changes_cannot_be_restored_to_revive_cancelled_touch() {
    for mode in 0..5 {
        let s = source(64);
        let batch = admitted(submit(&s, finger(0, 1, Phase::Down)));
        let guard = s
            .native_touch_publication_guard(&batch, view().owner)
            .unwrap();
        let mut changed = view();
        match mode {
            0 => changed.pixels_per_point = 3.,
            1 => changed.focused = false,
            2 => changed.ime = true,
            3 => changed.blocked = true,
            _ => changed.owner.session = 99,
        }
        s.publish(changed).unwrap();
        s.publish(view()).unwrap();
        assert!(!guard.is_current());
        assert!(!s.native_touch_batch_is_current(&batch, view().owner));
        suppressed(submit(&s, finger(1, 1, Phase::Move)));
        suppressed(submit(&s, finger(2, 1, Phase::Up)));
        drop(admitted(submit(&s, finger(3, 1, Phase::Down))));
    }
}
#[test]
fn ui_touch_keeps_same_workspace_geometry_changes_and_native_touch_retire_keeps_pen() {
    let s = source(64);
    let batch = admitted(
        s.submit_native_finger(
            finger(0, 1, Phase::Down),
            NativeFingerAdmission::Candidate,
            |_| Ok(Some((200., 200.))),
        )
        .unwrap(),
    );
    assert_eq!(batch.target(), NativeTouchTarget::Ui);
    let mut changed = view();
    changed.owner.state = 55;
    changed.owner.publication += 1;
    changed.transform.origin = (10., 10.);
    changed.transform.epoch += 1;
    s.publish(changed).unwrap();
    assert!(s.native_touch_batch_is_current(&batch, changed.owner));
    assert!(
        s.native_touch_publication_guard(&batch, changed.owner)
            .unwrap()
            .is_current()
    );
    let s = source(64);
    assert_eq!(pen(&s, 0, Phase::Down), Route::Canvas);
    assert_eq!(pen(&s, 1, Phase::Up), Route::Canvas);
    let pen = s.take(1, view().owner, false).unwrap();
    let guard = s.publication_guard(&pen, view().owner).unwrap();
    s.retire_native_touches().unwrap();
    assert!(guard.is_current());
    assert!(s.batch_is_current(&pen, view().owner));
}
#[test]
fn predictions_foreign_errors_and_boundary_cannot_change_actual_authority() {
    let s = source(64);
    let batch = admitted(submit(&s, finger(0, 1, Phase::Down)));
    let mut predicted = finger(1, 1, Phase::Cancel);
    predicted.ordinal = u64::MAX;
    predicted.identity.kind = DeviceKind::Pen;
    predicted.time.epoch = 999;
    predicted.provenance = Provenance::Predicted;
    suppressed(
        s.submit_native_finger(predicted, NativeFingerAdmission::PolicyCancel, |_| {
            panic!("prediction")
        })
        .unwrap(),
    );
    assert!(s.native_touch_batch_is_current(&batch, view().owner));
    let mut foreign = finger(1, 2, Phase::Down);
    foreign.time.epoch = 999;
    assert!(
        s.submit_native_finger(foreign, NativeFingerAdmission::Candidate, |_| Ok(None))
            .is_err()
    );
    assert!(s.native_touch_batch_is_current(&batch, view().owner));
    let mut boundary = finger(1, 2, Phase::Down);
    boundary.provenance = Provenance::Boundary;
    assert!(
        s.submit_native_finger(boundary, NativeFingerAdmission::Candidate, |_| panic!(
            "boundary"
        ))
        .is_err()
    );
    assert!(s.native_touch_batch_is_current(&batch, view().owner));
}
#[path = "tests_capacity.rs"]
mod capacity;

#[test]
fn cancellation_matches_private_source_cell_not_equal_public_identity_tuples() {
    let a = source(64);
    let b = source(64);
    let ab = admitted(submit(&a, finger(0, 1, Phase::Down)));
    let bb = admitted(submit(&b, finger(0, 1, Phase::Down)));
    let ag = a.native_touch_publication_guard(&ab, view().owner).unwrap();
    let bg = b.native_touch_publication_guard(&bb, view().owner).unwrap();
    let mouse = a.ui_publication_guard(view().owner).unwrap();
    let cancellation = a
        .retire_native_touches()
        .unwrap()
        .into_iter()
        .flatten()
        .next()
        .unwrap();
    assert!(cancellation.matches_batch(&ab));
    assert!(cancellation.matches_publication(&ag));
    assert!(!cancellation.matches_batch(&bb));
    assert!(!cancellation.matches_publication(&bg));
    assert!(!cancellation.matches_publication(&mouse));
    assert!(bg.is_current());
    assert!(mouse.is_current());
}
#[test]
fn captured_primary_release_fact_is_distinct_from_the_terminal_raw_flag() {
    let s = source(64);
    let down = admitted(submit(&s, finger(0, 1, Phase::Down)));
    let mut up = finger(1, 1, Phase::Up);
    up.primary = false;
    let release = admitted(submit(&s, up));
    assert!(down.captured_primary() && release.captured_primary());
    assert!(!release.metadata().primary);
}
