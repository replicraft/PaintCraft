use super::tests::{raw, source, view};
use super::*;
#[test]
fn pen_only_retirement_rejects_old_pen_but_preserves_real_mouse_hold_and_gui_guard() {
    for drained in [false, true] {
        let source = source(&SharedBudget::new(1 << 20));
        for (sequence, phase) in [(1, Phase::Down), (2, Phase::Up)] {
            source
                .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(sequence, phase)))
                .unwrap();
        }
        let old = drained.then(|| source.take(1, view().owner, false).unwrap());
        let pen_guard = old
            .as_ref()
            .map(|batch| source.publication_guard(batch, view().owner).unwrap());
        assert!(source.mouse_capture(true));
        let mouse_guard = source.ui_publication_guard(view().owner).unwrap();
        let epoch = source.cancellation_epoch();
        source.retire_pen();
        assert!(source.cancellation_epoch() > epoch);
        assert!(mouse_guard.is_current());
        if let Some(old) = &old {
            assert!(!source.batch_is_current(old, view().owner));
        }
        if let Some(guard) = pen_guard {
            assert!(!guard.is_current());
        }
        let batch = source.take(2, view().owner, false).unwrap();
        assert!(batch.frame.samples.is_empty());
        assert!(batch.frame.ui.is_empty());
        assert!(!batch.frame.suppress_legacy_pointer);
        assert!(!batch.frame.reset_gui_pointer);
        assert_eq!(batch.frame.cancelled, !drained);
        // Global acquisition failure/cancel still revokes actual GUI authority.
        source.cancel();
        assert!(!mouse_guard.is_current());
        let global = source.take(3, view().owner, false).unwrap();
        assert!(global.frame.reset_gui_pointer);
        assert!(global.frame.suppress_legacy_pointer);
    }
}
#[test]
fn pen_ui_batch_is_pen_bound_even_when_its_public_payload_is_empty() {
    let source = source(&SharedBudget::new(1 << 20));
    for (sequence, phase) in [(1, Phase::Down), (2, Phase::Up)] {
        source
            .submit_with(Delivery::ReceiverPointer, |_| {
                let mut raw = raw(sequence, phase);
                raw.logical_position = Some((120.0, 50.0));
                Ok(raw)
            })
            .unwrap();
    }
    let mut batch = source.take(1, view().owner, false).unwrap();
    assert!(batch.frame.samples.is_empty());
    assert!(!batch.frame.ui.is_empty());
    let guard = source.publication_guard(&batch, view().owner).unwrap();
    batch.frame.ui.clear();
    source.retire_pen();
    assert!(!source.batch_is_current(&batch, view().owner));
    assert!(!guard.is_current());
}
#[test]
fn malformed_suppressed_foreign_clock_cannot_fence_or_cancel_the_actual_owner() {
    let source = source(&SharedBudget::new(1 << 20));
    source
        .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(1, Phase::Down)))
        .unwrap();
    let mut foreign = raw(2, Phase::Down);
    foreign.identity.pointer += 1;
    foreign.logical_position = None;
    source.suppress_native(foreign).unwrap();
    let epoch = source.cancellation_epoch();
    foreign.sequence = 3;
    foreign.time.micros = 1;
    assert_eq!(source.suppress_native(foreign), Err(Invalid::Clock));
    assert_eq!(source.cancellation_epoch(), epoch);
    for (sequence, phase) in [(4, Phase::Move), (5, Phase::Up)] {
        assert_eq!(
            source
                .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(sequence, phase)))
                .unwrap(),
            Route::Canvas
        );
    }
    let batch = source.take(1, view().owner, false).unwrap();
    assert!(!batch.frame.cancelled);
    assert!(!batch.frame.reset_gui_pointer);
    assert_eq!(batch.frame.samples.len(), 3);
}
