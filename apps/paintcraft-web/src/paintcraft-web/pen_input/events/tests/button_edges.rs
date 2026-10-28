use super::metadata::{source, view};
use super::*;

fn packet(
    n: &mut Normalizer,
    kind: Kind,
    buttons: u16,
    button: i16,
    provenance: Provenance,
) -> Packet {
    let mut input = event(kind, buttons);
    input.button = button;
    input.at = None;
    n.intake(input, provenance).unwrap().unwrap()
}
fn ui(source: &paintcraft_ui::pen_input::Source, packet: Packet) -> receiver::Route {
    deliver_packet(source, packet, |_| Ok(Some((120.0, 20.0)))).unwrap()
}
fn presses(batch: &paintcraft_ui::pen_input::Batch) -> usize {
    batch
        .frame
        .ui
        .iter()
        .filter(|event| matches!(event.event, receiver::UiEvent::Button { pressed: true, .. }))
        .count()
}

#[test]
fn actual_changed_barrel_on_chorded_move_recovers_only_orphan_history_not_strong_denial() {
    for prior_idle in [false, true] {
        let mut n = Normalizer::new(3);
        let source = source();
        source.publish(view()).unwrap();
        if prior_idle {
            ui(
                &source,
                packet(&mut n, Kind::Move, 0, -1, Provenance::Actual),
            );
        }
        let history = packet(
            &mut n,
            Kind::Move,
            Buttons::BARREL,
            2,
            Provenance::Coalesced,
        );
        assert_eq!(history.raw.phase, Phase::Hover);
        assert_eq!(history.edge, None);
        assert_eq!(history.disposition, Disposition::Orphan);
        assert_eq!(ui(&source, history), receiver::Route::Suppressed);
        let actual = packet(&mut n, Kind::Move, Buttons::BARREL, 2, Provenance::Actual);
        assert_eq!(actual.raw.phase, Phase::Hover);
        assert_eq!(actual.disposition, Disposition::Deliver);
        assert_eq!(
            actual.edge,
            Some(PenButtonEdge {
                identity: actual.raw.identity,
                button: PenButton::Barrel,
                pressed: true
            })
        );
        assert_eq!(ui(&source, actual), receiver::Route::Ui);
        assert_eq!(presses(&source.take(1, view().owner, false).unwrap()), 1);
    }
    let mut n = Normalizer::new(3);
    let source = source();
    let first = packet(&mut n, Kind::Down, Buttons::BARREL, 2, Provenance::Actual);
    assert_eq!(ui(&source, first), receiver::Route::Suppressed); //no usable view
    n.deny_contact(first.raw.identity.pointer);
    source.publish(view()).unwrap();
    let later = packet(&mut n, Kind::Move, Buttons::BARREL, 2, Provenance::Actual);
    assert_eq!(later.disposition, Disposition::Suppressed);
    assert_eq!(ui(&source, later), receiver::Route::Suppressed);
    assert_eq!(presses(&source.take(1, view().owner, false).unwrap()), 0);
}

#[test]
fn history_cannot_consume_actual_idle_transition_or_actual_tip_release() {
    let mut n = Normalizer::new(3);
    let source = source();
    source.publish(view()).unwrap();
    ui(
        &source,
        packet(&mut n, Kind::Move, 0, -1, Provenance::Actual),
    );
    ui(
        &source,
        packet(
            &mut n,
            Kind::Move,
            Buttons::BARREL,
            -1,
            Provenance::Coalesced,
        ),
    );
    // Aggregate actual0→2 is observed even if this actual record has no separate
    // changed-button proof; history must not consume that prior actual idle.
    let actual = packet(&mut n, Kind::Move, Buttons::BARREL, -1, Provenance::Actual);
    assert_eq!(actual.disposition, Disposition::Deliver);
    assert_eq!(actual.edge, None);
    assert_eq!(ui(&source, actual), receiver::Route::Ui);
    assert_eq!(presses(&source.take(1, view().owner, false).unwrap()), 1);
    let mut n = Normalizer::new(3);
    packet(&mut n, Kind::Down, Buttons::ERASER, 5, Provenance::Actual);
    let history = packet(&mut n, Kind::Move, 0, 5, Provenance::Coalesced);
    assert_eq!(history.raw.phase, Phase::Move);
    assert_eq!(history.edge, None);
    assert_eq!(history.raw.identity.kind, DeviceKind::Eraser);
    let actual = packet(&mut n, Kind::Up, 0, 5, Provenance::Actual);
    assert_eq!(actual.raw.phase, Phase::Up);
    assert_eq!(actual.raw.identity.kind, DeviceKind::Eraser);
}

#[test]
fn historical_zero_buttons_cannot_release_owned_ui_or_admit_competing_mouse() {
    let mut n = Normalizer::new(3);
    let source = source();
    source.publish(view()).unwrap();
    assert_eq!(
        ui(
            &source,
            packet(&mut n, Kind::Down, Buttons::BARREL, 2, Provenance::Actual)
        ),
        receiver::Route::Ui
    );
    assert_eq!(
        ui(
            &source,
            packet(&mut n, Kind::Move, 0, 2, Provenance::Coalesced)
        ),
        receiver::Route::Ui
    );
    assert!(!source.mouse_capture(true));
    let batch = source.take(1, view().owner, false).unwrap();
    assert_eq!(presses(&batch), 1);
    assert_eq!(
        batch
            .frame
            .ui
            .iter()
            .filter(|event| matches!(
                event.event,
                receiver::UiEvent::Button { pressed: false, .. }
            ))
            .count(),
        0
    );
    assert_eq!(
        ui(&source, packet(&mut n, Kind::Up, 0, 2, Provenance::Actual)),
        receiver::Route::Ui
    );
    assert!(source.mouse_capture(true));
    source.mouse_capture(false);
}
