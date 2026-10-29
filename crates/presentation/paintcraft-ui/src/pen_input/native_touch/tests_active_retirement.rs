use super::*;

#[test]
fn active_touch_retirement_preserves_completed_touch_pen_and_mouse_guards() {
    let s = source(64);
    assert_eq!(pen(&s, 0, Phase::Down), Route::Canvas);
    assert_eq!(pen(&s, 1, Phase::Up), Route::Canvas);
    let old_pen = s.take(1, view().owner, false).unwrap();
    let pen_guard = s.publication_guard(&old_pen, view().owner).unwrap();
    let completed = admitted(submit(&s, finger(2, 1, Phase::Down)));
    let touch_guard = s
        .native_touch_publication_guard(&completed, view().owner)
        .unwrap();
    drop(submit(&s, finger(3, 1, Phase::Up)));
    let mouse_guard = s.ui_publication_guard(view().owner).unwrap();
    // Same native identity, different exact opaque lifetime.
    let held = admitted(submit(&s, finger(4, 1, Phase::Down)));
    let held_guard = s
        .native_touch_publication_guard(&held, view().owner)
        .unwrap();
    let retired = s.retire_active_native_touches().unwrap();
    let cancellation = retired.iter().flatten().next().unwrap();
    assert_eq!(retired.iter().flatten().count(), 1);
    assert!(cancellation.matches_batch(&held));
    assert!(cancellation.matches_publication(&held_guard));
    assert!(!cancellation.matches_batch(&completed));
    assert!(!cancellation.matches_publication(&touch_guard));
    assert!(!held_guard.is_current());
    assert!(touch_guard.is_current());
    assert!(pen_guard.is_current());
    assert!(mouse_guard.is_current());
    assert!(s.batch_is_current(&old_pen, view().owner));
    assert!(s.native_touch_batch_is_current(&completed, view().owner));
    suppressed(submit(&s, finger(5, 1, Phase::Move)));
    suppressed(submit(&s, finger(6, 1, Phase::Up)));
    let fresh = admitted(submit(&s, finger(7, 1, Phase::Down)));
    assert!(!cancellation.matches_batch(&fresh));
    assert!(s.native_touch_batch_is_current(&fresh, view().owner));
}

#[test]
fn active_touch_retirement_ignores_denied_tails_and_needs_no_terminal_position() {
    let s = source(64);
    let mut unknown = finger(0, 3, Phase::Down);
    unknown.confidence = FingerConfidence::Unspecified;
    suppressed(submit(&s, unknown));
    assert!(
        s.retire_active_native_touches()
            .unwrap()
            .iter()
            .all(Option::is_none)
    );
    let held = admitted(submit(&s, finger(1, 1, Phase::Down)));
    let retired = s.retire_active_native_touches().unwrap();
    let cancellation = retired.iter().flatten().next().unwrap();
    assert!(cancellation.matches_batch(&held));
    assert_eq!(cancellation.last_observed_position(), Some((10., 10.)));
    assert!(cancellation.cause().is_none());
    let terminal = s
        .submit_native_finger(
            finger(2, 1, Phase::Up),
            NativeFingerAdmission::Candidate,
            |_| panic!("retired terminal uses metadata only"),
        )
        .unwrap();
    suppressed(terminal);
    assert!(
        s.retire_active_native_touches()
            .unwrap()
            .iter()
            .all(Option::is_none)
    );
}
