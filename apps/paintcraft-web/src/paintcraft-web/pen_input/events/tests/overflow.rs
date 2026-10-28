use super::metadata::{source, view};
use super::*;
use paintcraft_core::types::{ErrorCode, resources::SharedBudget};

fn actual(
    n: &mut Normalizer,
    id: u64,
    kind: Kind,
    buttons: u16,
) -> Result<(RawSample, bool), Invalid> {
    let mut input = event(kind, buttons);
    input.id = id;
    input.at = None;
    n.ingest(input, Provenance::Actual)
}
fn fill(n: &mut Normalizer) {
    for id in 0..8 {
        assert!(!actual(n, id, Kind::Down, Buttons::TIP).unwrap().1);
    }
}

#[test]
fn ninth_barrel_stays_denied_when_an_unrelated_slot_frees_then_its_own_up_allows_fresh_edge() {
    let mut n = Normalizer::new(3);
    let source = source();
    source.publish(view()).unwrap();
    let (owner, denied) = actual(&mut n, 0, Kind::Down, Buttons::TIP).unwrap();
    assert_eq!(
        deliver(&source, owner, denied, |_| Ok(Some((20.0, 20.0)))),
        Ok(receiver::Route::Canvas)
    );
    for id in 1..8 {
        assert!(!actual(&mut n, id, Kind::Down, Buttons::TIP).unwrap().1);
    }
    let (raw, suppressed) = actual(&mut n, 9, Kind::Down, Buttons::BARREL).unwrap();
    assert!(suppressed);
    assert_eq!(
        deliver(&source, raw, suppressed, |_| panic!("denied geometry")),
        Ok(receiver::Route::Suppressed)
    );
    actual(&mut n, 1, Kind::Up, 0).unwrap();
    let (moved, suppressed) = actual(&mut n, 9, Kind::Move, Buttons::BARREL).unwrap();
    assert!(suppressed);
    assert_eq!(moved.phase, Phase::Hover);
    deliver(&source, moved, suppressed, |_| {
        panic!("freed slot must not admit tail")
    })
    .unwrap();
    let batch = source.take(1, view().owner, false).unwrap();
    assert!(batch.frame.ui.is_empty());
    assert!(!batch.frame.cancelled);
    assert_eq!(batch.frame.samples.len(), 1);
    assert_eq!(batch.frame.samples[0].identity, owner.identity);
    let (terminal, suppressed) = actual(&mut n, 9, Kind::Up, 0).unwrap();
    assert!(suppressed);
    deliver(&source, terminal, suppressed, |_| {
        panic!("terminal geometry")
    })
    .unwrap();
    let (owner_up, denied) = actual(&mut n, 0, Kind::Up, 0).unwrap();
    assert_eq!(
        deliver(&source, owner_up, denied, |_| Ok(Some((20.0, 20.0)))),
        Ok(receiver::Route::Canvas)
    );
    let (fresh, suppressed) = actual(&mut n, 9, Kind::Down, Buttons::BARREL).unwrap();
    assert!(!suppressed);
    assert_eq!(
        deliver(&source, fresh, suppressed, |_| Ok(Some((120.0, 20.0)))),
        Ok(receiver::Route::Ui)
    );
    let batch = source.take(2, view().owner, false).unwrap();
    assert_eq!(
        batch
            .frame
            .ui
            .iter()
            .filter(|event| matches!(event.event, receiver::UiEvent::Button { pressed: true, .. }))
            .count(),
        1
    );
}

#[test]
fn denied_eraser_keeps_end_identity_through_barrel_leave_and_zero_button_terminal() {
    let mut n = Normalizer::new(3);
    fill(&mut n);
    let source = source();
    let (down, denied) = actual(&mut n, 9, Kind::Down, Buttons::ERASER | Buttons::BARREL).unwrap();
    assert!(denied);
    assert_eq!(down.identity.kind, DeviceKind::Eraser);
    deliver(&source, down, denied, |_| panic!("denied down geometry")).unwrap();
    actual(&mut n, 0, Kind::Up, 0).unwrap();
    for (kind, buttons) in [
        (Kind::Move, Buttons::BARREL),
        (Kind::Leave, Buttons::BARREL),
        (Kind::Up, 0),
    ] {
        let (raw, denied) = actual(&mut n, 9, kind, buttons).unwrap();
        assert!(denied);
        assert_eq!(raw.identity, down.identity);
        deliver(&source, raw, denied, |_| panic!("denied eraser geometry")).unwrap();
    }
    source.publish(view()).unwrap();
    let (fresh, denied) = actual(&mut n, 9, Kind::Down, Buttons::ERASER).unwrap();
    assert!(!denied);
    assert_eq!(fresh.identity, down.identity);
    assert_eq!(
        deliver(&source, fresh, denied, |_| Ok(Some((20.0, 20.0)))),
        Ok(receiver::Route::Canvas)
    );
}

#[test]
fn orphan_held_barrel_is_fenced_but_observed_zero_hover_to_barrel_transition_is_real() {
    for buttons in [
        Buttons::BARREL,
        Buttons::BARREL_UPPER,
        Buttons::TIP,
        Buttons::ERASER,
    ] {
        let mut n = Normalizer::new(3);
        let (orphan, denied) = actual(&mut n, 77, Kind::Move, buttons).unwrap();
        assert!(denied);
        assert_eq!(
            orphan.phase,
            if buttons & (Buttons::TIP | Buttons::ERASER) != 0 {
                Phase::Move
            } else {
                Phase::Hover
            }
        );
        assert!(actual(&mut n, 77, Kind::Move, buttons).unwrap().1);
        assert!(actual(&mut n, 77, Kind::Up, 0).unwrap().1);
        assert!(!actual(&mut n, 77, Kind::Down, buttons).unwrap().1);
    }
    for buttons in [Buttons::BARREL, Buttons::BARREL_UPPER] {
        let mut n = Normalizer::new(3);
        assert!(!actual(&mut n, 4, Kind::Move, 0).unwrap().1);
        let (edge, denied) = actual(&mut n, 4, Kind::Move, buttons).unwrap();
        assert!(!denied);
        assert_eq!(edge.phase, Phase::Hover);
        assert_eq!(edge.buttons, Buttons(buttons));
    }
}

#[test]
fn exhausted_denial_table_stays_closed_after_every_unrelated_or_known_release_until_restart() {
    let mut n = Normalizer::new(3);
    fill(&mut n);
    for id in 8..16 {
        assert!(
            actual(&mut n, id, Kind::Down, Buttons::ERASER | Buttons::BARREL)
                .unwrap()
                .1
        );
    }
    assert_eq!(
        actual(&mut n, 16, Kind::Down, Buttons::BARREL),
        Err(Invalid::Capacity)
    );
    assert!(n.restart_required());
    let sequence = n.sequence;
    let mut predicted = event(Kind::Up, 0);
    predicted.id = 8;
    predicted.at = None;
    assert_eq!(
        n.ingest(predicted, Provenance::Predicted),
        Err(Invalid::Lifecycle)
    );
    assert_eq!(n.sequence, sequence);
    assert_eq!(n.denied.iter().flatten().count(), 8);
    assert_eq!(actual(&mut n, 99, Kind::Up, 0), Err(Invalid::Capacity));
    for id in 0..16 {
        let (terminal, denied) = actual(&mut n, id, Kind::Up, 0).unwrap();
        assert!(denied);
        assert_eq!(
            terminal.identity.kind,
            if id < 8 {
                DeviceKind::Pen
            } else {
                DeviceKind::Eraser
            }
        );
    }
    assert_eq!(
        n.tools.iter().flatten().count() + n.denied.iter().flatten().count(),
        0
    );
    n.clear();
    n.observe_cancellation(9);
    n.observe_cancellation(10);
    assert_eq!(actual(&mut n, 16, Kind::Up, 0), Err(Invalid::Capacity));
    assert_eq!(
        actual(&mut n, 0, Kind::Down, Buttons::TIP),
        Err(Invalid::Capacity)
    );
    assert!(n.restart_required());
    let mut restarted = Normalizer::new(3);
    assert!(!restarted.restart_required());
    assert!(
        !actual(&mut restarted, 0, Kind::Down, Buttons::TIP)
            .unwrap()
            .1
    );
}

#[test]
fn normalizer_storage_is_admitted_before_allocation_and_retained_until_last_listener_clone() {
    let tiny = SharedBudget::new(1);
    assert_eq!(
        Normalizer::admitted(3, &tiny).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(tiny.usage().reserved, 0);
    let budget = SharedBudget::new(1 << 20);
    let state = std::rc::Rc::new(std::cell::RefCell::new(
        Normalizer::admitted(3, &budget).unwrap(),
    ));
    let charged = budget.usage().reserved;
    assert!(charged >= std::mem::size_of::<Normalizer>() as u64);
    let listener = state.clone();
    drop(state);
    assert_eq!(budget.usage().reserved, charged);
    drop(listener);
    assert_eq!(budget.usage().reserved, 0);
}
