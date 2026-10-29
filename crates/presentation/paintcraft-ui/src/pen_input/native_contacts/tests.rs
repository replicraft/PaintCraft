use super::*;
use crate::pen_input::tests::{raw, view};
fn source() -> Source {
    let source = Source::new_native_contacts(64, 64, &SharedBudget::new(8 << 20)).unwrap();
    source.publish(view()).unwrap();
    source
}
fn push(source: &Source, ordinal: u64, phase: Phase, buttons: u16, ui: bool) -> Route {
    let mut record = raw(ordinal, phase);
    record.logical_position = None;
    record.buttons = Buttons(buttons);
    source
        .submit_native(Delivery::ReceiverPointer, record, |_| {
            Ok(Some(if ui { (200., 200.) } else { (20., 30.) }))
        })
        .unwrap()
}
#[test]
fn later_held_pen_retirement_preserves_older_completed_pen_and_mouse_guards() {
    let s = source();
    assert_eq!(push(&s, 0, Phase::Down, Buttons::TIP, false), Route::Canvas);
    push(&s, 1, Phase::Up, 0, false);
    let old = s.take(1, view().owner, false).unwrap();
    let old_guard = s.publication_guard(&old, view().owner).unwrap();
    let mouse_guard = s.ui_publication_guard(view().owner).unwrap();
    push(&s, 2, Phase::Down, Buttons::TIP, false);
    let held = s.take(2, view().owner, false).unwrap();
    let held_guard = s.publication_guard(&held, view().owner).unwrap();
    let cancellations = s.retire_active_native_pen().unwrap();
    let cancellation = cancellations.iter().flatten().next().unwrap();
    assert!(cancellation.matches_batch(&held));
    assert!(cancellation.matches_publication(&held_guard));
    assert!(!cancellation.matches_batch(&old));
    assert!(old_guard.is_current());
    assert!(mouse_guard.is_current());
    assert!(!held_guard.is_current());
    assert_eq!(
        push(&s, 3, Phase::Move, Buttons::TIP, false),
        Route::Suppressed
    );
    push(&s, 4, Phase::Up, 0, false);
    push(&s, 5, Phase::Down, Buttons::TIP, false);
    let fresh = s.take(3, view().owner, false).unwrap();
    assert!(!cancellation.matches_batch(&fresh));
    assert!(s.batch_is_current(&fresh, view().owner));
}
#[test]
fn active_suffix_pruning_preserves_completed_undrained_interval() {
    let s = source();
    push(&s, 0, Phase::Down, Buttons::TIP, false);
    push(&s, 1, Phase::Up, 0, false);
    push(&s, 2, Phase::Down, Buttons::TIP, false);
    push(&s, 3, Phase::Move, Buttons::TIP, false);
    s.retire_active_native_pen().unwrap();
    let batch = s.take(1, view().owner, false).unwrap();
    assert_eq!(
        batch
            .frame
            .samples
            .iter()
            .map(|s| s.sequence)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(!batch.frame.cancelled);
    assert!(!batch.frame.reset_gui_pointer);
    assert!(s.batch_is_current(&batch, view().owner));
}
#[test]
fn aggregated_interval_is_never_partially_authorized_after_active_retirement() {
    let s = source();
    push(&s, 0, Phase::Down, Buttons::TIP, false);
    push(&s, 1, Phase::Up, 0, false);
    push(&s, 2, Phase::Down, Buttons::TIP, false);
    let mixed = s.take(1, view().owner, false).unwrap();
    let guard = s.publication_guard(&mixed, view().owner).unwrap();
    s.retire_active_native_pen().unwrap();
    assert!(!s.batch_is_current(&mixed, view().owner));
    assert!(!guard.is_current());
}
#[test]
fn barrel_held_after_canvas_up_cannot_revoke_completed_ink() {
    let s = source();
    push(&s, 0, Phase::Down, Buttons::TIP | Buttons::BARREL, false);
    push(&s, 1, Phase::Up, Buttons::BARREL, false);
    let old = s.take(1, view().owner, false).unwrap();
    let guard = s.publication_guard(&old, view().owner).unwrap();
    // Physical barrel capture exists but its completed ink is not active work.
    assert!(
        s.retire_active_native_pen()
            .unwrap()
            .iter()
            .all(Option::is_none)
    );
    assert!(guard.is_current());
    assert!(!s.0.lock().unwrap().receiver.capture_status().pen);
    push(&s, 2, Phase::Up, 0, false);
    push(&s, 3, Phase::Down, Buttons::TIP | Buttons::BARREL, false);
    let new = s.take(2, view().owner, false).unwrap();
    let retired = s.retire_active_native_pen().unwrap();
    assert!(retired.iter().flatten().any(|c| c.matches_batch(&new)));
    assert!(guard.is_current());
}
#[test]
fn new_tip_with_uninterrupted_barrel_hold_gets_its_own_ink_lifetime() {
    let s = source();
    push(&s, 0, Phase::Down, Buttons::TIP | Buttons::BARREL, false);
    push(&s, 1, Phase::Up, Buttons::BARREL, false);
    let old = s.take(1, view().owner, false).unwrap();
    let guard = s.publication_guard(&old, view().owner).unwrap();
    push(&s, 2, Phase::Down, Buttons::TIP | Buttons::BARREL, false);
    let next = s.take(2, view().owner, false).unwrap();
    let retired = s.retire_active_native_pen().unwrap();
    assert!(retired.iter().flatten().any(|c| c.matches_batch(&next)));
    assert!(!retired.iter().flatten().any(|c| c.matches_batch(&old)));
    assert!(guard.is_current());
}
#[test]
fn retired_ui_chord_does_not_remove_an_earlier_undrained_click() {
    let s = source();
    push(&s, 0, Phase::Down, Buttons::TIP, true);
    push(&s, 1, Phase::Up, 0, true);
    push(&s, 2, Phase::Down, Buttons::TIP | Buttons::BARREL, true);
    push(&s, 3, Phase::Up, Buttons::BARREL, true);
    s.retire_active_native_pen().unwrap();
    let batch = s.take(1, view().owner, false).unwrap();
    assert!(
        batch
            .frame
            .ui
            .iter()
            .all(|r| r.sequence.is_some_and(|s| s < 2))
    );
    assert!(batch.frame.ui.iter().any(|r| matches!(
        r.event,
        paintcraft_input::tablet::receiver::UiEvent::Button { pressed: false, .. }
    )));
    assert!(!batch.frame.cancelled);
    assert!(!batch.frame.reset_gui_pointer);
    assert!(s.batch_is_current(&batch, view().owner));
}
#[test]
fn native_contact_admission_and_retained_guards_have_explicit_lifetimes() {
    assert!(Source::new_native_contacts(8, 8, &SharedBudget::new(1)).is_err());
    let budget = SharedBudget::new(8 << 20);
    let s = Source::new_native_contacts(8, 8, &budget).unwrap();
    s.publish(view()).unwrap();
    push(&s, 0, Phase::Down, Buttons::TIP, false);
    push(&s, 1, Phase::Up, 0, false);
    let batch = s.take(1, view().owner, false).unwrap();
    let guard = s.publication_guard(&batch, view().owner).unwrap();
    drop(batch);
    drop(s);
    assert!(
        budget.usage().reserved > 0,
        "cell declaration follows publication holder"
    );
    assert!(!guard.is_current());
    drop(guard);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn ui_chord_is_one_lifetime_and_retirement_does_not_clear_earlier_click() {
    let s = source();
    push(&s, 0, Phase::Down, Buttons::TIP, true);
    push(&s, 1, Phase::Up, 0, true);
    let mut idle = raw(2, Phase::Hover);
    idle.buttons = Buttons(0);
    idle.logical_position = None;
    s.submit_native(Delivery::ReceiverPointer, idle, |_| Ok(Some((200., 200.))))
        .unwrap();
    push(&s, 3, Phase::Hover, Buttons::BARREL, true);
    let mixed = s.take(1, view().owner, false).unwrap();
    assert!(mixed.frame.ui.iter().any(|r| matches!(
        r.event,
        paintcraft_input::tablet::receiver::UiEvent::Button {
            button: paintcraft_input::PointerButton::Secondary,
            pressed: true,
            ..
        }
    )));
    let guard = s.publication_guard(&mixed, view().owner).unwrap();
    s.retire_active_native_pen().unwrap();
    assert!(
        !guard.is_current(),
        "mixed UI interval must not publish an unaffected prefix"
    );
}
#[test]
fn unconfigured_sources_explicitly_refuse_active_pen_retirement() {
    let budget = SharedBudget::new(1 << 20);
    for s in [
        Source::new(8, 8, &budget).unwrap(),
        Source::new_native_touch(8, 8, &budget).unwrap(),
    ] {
        assert!(matches!(s.retire_active_native_pen(), Err(Invalid::Owner)));
    }
}
#[test]
fn multipass_empty_handoff_cannot_consume_pending_contact_authority() {
    let s = source();
    push(&s, 0, Phase::Down, Buttons::TIP, false);
    let first = s.take(1, view().owner, false).unwrap();
    push(&s, 1, Phase::Move, Buttons::TIP, false);
    let repeated = s.take(1, view().owner, false).unwrap();
    assert!(repeated.frame.samples.is_empty());
    let next = s.take(2, view().owner, false).unwrap();
    assert_eq!(next.frame.samples.len(), 1);
    let cancellations = s.retire_active_native_pen().unwrap();
    assert!(
        cancellations
            .iter()
            .flatten()
            .any(|c| c.matches_batch(&first))
    );
    assert!(
        cancellations
            .iter()
            .flatten()
            .any(|c| c.matches_batch(&next))
    );
    assert!(!s.batch_is_current(&next, view().owner));
}
#[test]
fn mixed_native_interval_cannot_rebase_an_earlier_contact_via_its_later_down() {
    let s = source();
    push(&s, 0, Phase::Down, Buttons::TIP, false);
    push(&s, 1, Phase::Up, 0, false);
    push(&s, 2, Phase::Down, Buttons::TIP, false);
    let mut mixed = s.take(1, view().owner, false).unwrap();
    let before = view().owner;
    let after = Owner {
        state: before.state + 1,
        publication: before.publication + 1,
        ..before
    };
    let proof = Predecessor {
        before,
        after,
        sequence: 1,
        transform: view().transform,
        canvas: view().canvas,
        pixels_per_point: view().pixels_per_point,
        focused: true,
        modal: false,
        ime: false,
    };
    assert!(s.acknowledge_predecessor(&mut mixed, proof).is_err());
    assert!(
        s.batch_is_current(&mixed, before),
        "failed atomic validation does not mutate head/context"
    );
    assert!(!s.batch_is_current(&mixed, after));
}
