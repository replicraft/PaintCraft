use super::*;
use crate::pen_input::tests::{raw, source, view};
fn send(source: &Source, mut raw: RawSample) -> Route {
    let position = raw.logical_position.take();
    source
        .submit_native(Delivery::ReceiverPointer, raw, |_| Ok(position))
        .unwrap()
}
fn unmapped(source: &Source, mut raw: RawSample) -> Route {
    raw.logical_position = None;
    source
        .submit_native(Delivery::ReceiverPointer, raw, |_| {
            panic!("no layout: conversion must not run")
        })
        .unwrap()
}
#[test]
fn terminal_without_a_view_retires_only_real_held_tail_then_fresh_contact_works() {
    let budget = SharedBudget::new(1 << 20);
    let source = source(&budget);
    assert_eq!(send(&source, raw(1, Phase::Down)), Route::Canvas);
    source.invalidate_view();
    assert_eq!(unmapped(&source, raw(2, Phase::Up)), Route::Suppressed);
    source.publish(view()).unwrap();
    assert_eq!(send(&source, raw(3, Phase::Down)), Route::Canvas);
    let batch = source.take(1, view().owner, false).unwrap();
    assert!(batch.frame.cancelled);
    assert_eq!(batch.frame.samples.len(), 1);
    assert_eq!(batch.frame.samples[0].sequence, 3);
}
#[test]
fn down_during_invalid_view_cannot_turn_into_mid_tail_widget_press_after_scale_restore() {
    let source = Source::new(16, 16, &SharedBudget::new(1 << 20)).unwrap();
    assert_eq!(unmapped(&source, raw(1, Phase::Down)), Route::Suppressed);
    let mut changed = view();
    changed.pixels_per_point = 3.0;
    source.publish(changed).unwrap();
    source.invalidate_view();
    source.publish(view()).unwrap();
    assert_eq!(send(&source, raw(2, Phase::Move)), Route::Suppressed);
    let mut predicted = raw(3, Phase::Up);
    predicted.provenance = Provenance::Predicted;
    assert_eq!(send(&source, predicted), Route::Suppressed);
    assert_eq!(send(&source, raw(4, Phase::Move)), Route::Suppressed);
    source.invalidate_view();
    assert_eq!(unmapped(&source, raw(5, Phase::Up)), Route::Suppressed);
    source.publish(view()).unwrap();
    assert_eq!(send(&source, raw(6, Phase::Down)), Route::Canvas);
}
#[test]
fn foreign_terminal_and_predicted_own_terminal_do_not_cancel_actual_active_pen() {
    let source = source(&SharedBudget::new(1 << 20));
    assert_eq!(send(&source, raw(1, Phase::Down)), Route::Canvas);
    let mut foreign = raw(2, Phase::Cancel);
    foreign.identity.pointer += 1;
    assert_eq!(send(&source, foreign), Route::Suppressed);
    let mut predicted = raw(3, Phase::Up);
    predicted.provenance = Provenance::Predicted;
    assert_eq!(send(&source, predicted), Route::Suppressed);
    let batch = source.take(1, view().owner, false).unwrap();
    assert!(!batch.frame.cancelled);
    assert_eq!(batch.frame.samples.len(), 1);
    assert_eq!(batch.frame.samples[0].phase, Phase::Down);
    assert_eq!(send(&source, raw(4, Phase::Up)), Route::Canvas);
}
#[test]
fn mouse_denied_contact_stays_fenced_until_real_lift_and_eraser_hover_is_not_held() {
    let source = source(&SharedBudget::new(1 << 20));
    assert!(source.mouse_capture(true));
    assert_eq!(send(&source, raw(1, Phase::Down)), Route::Suppressed);
    source.mouse_capture(false);
    assert_eq!(send(&source, raw(2, Phase::Move)), Route::Suppressed);
    assert_eq!(send(&source, raw(3, Phase::Up)), Route::Suppressed);
    assert_eq!(send(&source, raw(4, Phase::Down)), Route::Canvas);
    assert_eq!(send(&source, raw(5, Phase::Up)), Route::Canvas);
    source.invalidate_view();
    let mut hover = raw(6, Phase::Hover);
    hover.identity.kind = paintcraft_input::tablet::DeviceKind::Eraser;
    hover.buttons = Buttons(Buttons::ERASER);
    assert_eq!(unmapped(&source, hover), Route::Suppressed);
    source.publish(view()).unwrap();
    let mut down = raw(7, Phase::Down);
    down.identity = hover.identity;
    down.buttons = Buttons(Buttons::TIP | Buttons::ERASER);
    assert_eq!(send(&source, down), Route::Canvas);
}

#[test]
fn in_contact_proximity_is_not_a_fabricated_widget_press_but_real_down_can_start_it() {
    let source = source(&SharedBudget::new(1 << 20));
    let mut enter = raw(1, Phase::Enter);
    enter.logical_position = Some((120.0, 50.0)); // widget area, outside canvas
    assert_eq!(send(&source, enter), Route::Suppressed);
    let mut moved = raw(2, Phase::Move);
    moved.logical_position = enter.logical_position;
    assert_eq!(send(&source, moved), Route::Suppressed);
    let empty = source.take(1, view().owner, false).unwrap();
    assert!(empty.frame.ui.is_empty());
    assert!(empty.frame.samples.is_empty());
    let mut down = raw(3, Phase::Down);
    down.logical_position = enter.logical_position;
    assert_eq!(send(&source, down), Route::Ui);
    let batch = source.take(2, view().owner, false).unwrap();
    assert!(batch.frame.ui.iter().any(|record| matches!(
        record.event,
        paintcraft_input::tablet::receiver::UiEvent::Button { pressed: true, .. }
    )));
}

#[test]
fn malformed_foreign_predictions_cannot_fence_or_cancel_actual_capture() {
    let source = source(&SharedBudget::new(1 << 20));
    assert_eq!(send(&source, raw(1, Phase::Down)), Route::Canvas);
    for phase in [Phase::Cancel, Phase::Leave] {
        let mut predicted = raw(3, phase);
        predicted.sequence = u64::MAX;
        predicted.provenance = Provenance::Predicted;
        predicted.identity.kind = paintcraft_input::tablet::DeviceKind::Finger;
        predicted.identity.pointer = 999;
        predicted.logical_position = Some((f64::NAN, f64::NAN));
        assert_eq!(
            source
                .submit_native(Delivery::ReceiverPointer, predicted, |_| panic!(
                    "prediction conversion must not run"
                ))
                .unwrap(),
            Route::Suppressed
        );
    }
    assert_eq!(send(&source, raw(2, Phase::Up)), Route::Canvas);
    let batch = source.take(1, view().owner, false).unwrap();
    assert!(!batch.frame.cancelled);
    assert_eq!(batch.frame.samples.len(), 2);
    assert_eq!(batch.frame.samples[1].phase, Phase::Up);
}

#[test]
fn upstream_policy_suppression_observes_actual_terminal_without_forwarding_coordinates() {
    let source = source(&SharedBudget::new(1 << 20));
    assert_eq!(send(&source, raw(1, Phase::Down)), Route::Canvas);
    let mut denied = raw(2, Phase::Move);
    denied.logical_position = None;
    assert_eq!(source.suppress_native(denied).unwrap(), Route::Suppressed);
    let mut terminal = raw(3, Phase::Up);
    terminal.logical_position = None;
    assert_eq!(source.suppress_native(terminal).unwrap(), Route::Suppressed);
    assert_eq!(send(&source, raw(4, Phase::Down)), Route::Canvas);
    let batch = source.take(1, view().owner, false).unwrap();
    assert!(batch.frame.cancelled);
    assert_eq!(batch.frame.samples.len(), 1);
    assert_eq!(batch.frame.samples[0].sequence, 4);
}

#[test]
fn actual_idle_leave_preserves_completed_up_but_held_leave_and_explicit_cancel_retire() {
    for (completed, cancel) in [(true, false), (false, false), (true, true)] {
        let source = source(&SharedBudget::new(1 << 20));
        let identity = raw(1, Phase::Down).identity;
        assert_eq!(send(&source, raw(1, Phase::Down)), Route::Canvas);
        if completed {
            assert_eq!(send(&source, raw(2, Phase::Up)), Route::Canvas);
        }
        if !completed {
            assert!(source.retire_native_identity(identity).is_err());
        }
        let batch = source.take(1, view().owner, false).unwrap();
        let mut end = raw(3, if cancel { Phase::Cancel } else { Phase::Leave });
        end.buttons = Buttons(0);
        assert_eq!(send(&source, end), Route::Suppressed);
        assert_eq!(
            source.batch_is_current(&batch, view().owner),
            completed && !cancel
        );
        if !cancel {
            assert!(source.retire_native_identity(identity).is_ok());
        }
    }
}
