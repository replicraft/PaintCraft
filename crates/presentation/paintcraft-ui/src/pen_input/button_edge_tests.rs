use super::tests::{raw, source, view};
use super::*;
use paintcraft_input::tablet::{Buttons, PenButton, PenButtonEdge, Provenance, receiver::UiEvent};
fn hover(sequence: u64, buttons: u16) -> RawSample {
    let mut raw = raw(sequence, Phase::Hover);
    raw.buttons = Buttons(buttons);
    raw.logical_position = None;
    raw
}
fn send(
    source: &Source,
    raw: RawSample,
    edge: Option<PenButtonEdge>,
) -> std::result::Result<Route, Invalid> {
    source.submit_native_with_button_edge(Delivery::ReceiverPointer, raw, edge, |_| {
        Ok(Some((120.0, 50.0)))
    })
}
#[test]
fn actual_side_edges_preserve_hover_phase_and_never_create_tip_ink() {
    for button in [PenButton::Barrel, PenButton::UpperBarrel] {
        let source = source(&SharedBudget::new(1 << 20));
        let down = hover(1, button.mask());
        let edge = PenButtonEdge {
            identity: down.identity,
            button,
            pressed: true,
        };
        assert_eq!(send(&source, down, Some(edge)).unwrap(), Route::Ui);
        let up = hover(2, 0);
        assert_eq!(
            send(
                &source,
                up,
                Some(PenButtonEdge {
                    pressed: false,
                    ..edge
                })
            )
            .unwrap(),
            Route::Ui
        );
        let batch = source.take(1, view().owner, false).unwrap();
        assert!(batch.frame.samples.is_empty());
        let changes: Vec<_> = batch
            .frame
            .ui
            .iter()
            .filter_map(|r| match r.event {
                UiEvent::Button {
                    button, pressed, ..
                } => Some((button, pressed)),
                _ => None,
            })
            .collect();
        assert_eq!(
            changes,
            [
                (paintcraft_input::PointerButton::Secondary, true),
                (paintcraft_input::PointerButton::Secondary, false)
            ]
        );
    }
}
#[test]
fn orphan_held_barrel_needs_real_lift_idle_transition_or_exact_actual_edge() {
    for explicit in [false, true] {
        let source = source(&SharedBudget::new(1 << 20));
        let down = hover(1, Buttons::BARREL);
        assert_eq!(send(&source, down, None).unwrap(), Route::Suppressed);
        if explicit {
            let edge = PenButtonEdge {
                identity: down.identity,
                button: PenButton::Barrel,
                pressed: true,
            };
            assert_eq!(
                send(&source, hover(2, Buttons::BARREL), Some(edge)).unwrap(),
                Route::Ui
            );
        } else {
            assert_eq!(
                send(&source, hover(2, Buttons::BARREL), None).unwrap(),
                Route::Suppressed
            );
            assert_eq!(send(&source, hover(3, 0), None).unwrap(), Route::Suppressed);
            assert_eq!(
                send(&source, hover(4, Buttons::BARREL), None).unwrap(),
                Route::Ui
            );
        }
    }
}
#[test]
fn mismatched_or_historical_side_edge_is_not_authority() {
    for reason in 0..4 {
        let source = source(&SharedBudget::new(1 << 20));
        let mut raw = hover(1, Buttons::BARREL);
        let mut edge = PenButtonEdge {
            identity: raw.identity,
            button: PenButton::Barrel,
            pressed: true,
        };
        match reason {
            0 => edge.pressed = false,
            1 => edge.identity.pointer += 1,
            2 => raw.provenance = Provenance::Coalesced,
            _ => raw.provenance = Provenance::Boundary,
        }
        assert!(send(&source, raw, Some(edge)).is_err());
        let batch = source.take(1, view().owner, false).unwrap();
        assert!(batch.frame.samples.is_empty());
        assert!(
            !batch
                .frame
                .ui
                .iter()
                .any(|r| matches!(r.event, UiEvent::Button { pressed: true, .. }))
        );
    }
}
#[test]
fn history_cannot_create_side_press_or_release_current_actual_ui_hold() {
    let source = source(&SharedBudget::new(1 << 20));
    send(&source, hover(1, 0), None).unwrap();
    let mut historical = hover(2, Buttons::BARREL);
    historical.provenance = Provenance::Coalesced;
    assert_eq!(send(&source, historical, None).unwrap(), Route::Suppressed);
    let actual = hover(3, Buttons::BARREL);
    send(
        &source,
        actual,
        Some(PenButtonEdge {
            identity: actual.identity,
            button: PenButton::Barrel,
            pressed: true,
        }),
    )
    .unwrap();
    let mut old_idle = hover(4, 0);
    old_idle.provenance = Provenance::Coalesced;
    assert_eq!(send(&source, old_idle, None).unwrap(), Route::Ui);
    let first = source.take(1, view().owner, false).unwrap();
    assert_eq!(
        first
            .frame
            .ui
            .iter()
            .filter(|r| matches!(r.event, UiEvent::Button { pressed: true, .. }))
            .count(),
        1
    );
    assert!(
        !first
            .frame
            .ui
            .iter()
            .any(|r| matches!(r.event, UiEvent::Button { pressed: false, .. }))
    );
    assert!(
        !source.mouse_capture(true),
        "history must not release actual pen ownership"
    );
    send(&source, hover(5, 0), None).unwrap();
    assert!(source.mouse_capture(true));
}
#[test]
fn historical_idle_cannot_clear_strong_fence_or_authorize_orphan_eraser_contact() {
    let source = source(&SharedBudget::new(1 << 20));
    let mut down = raw(1, Phase::Down);
    down.logical_position = None;
    source
        .submit_native(Delivery::ReceiverPointer, down, |_| Ok(Some((20.0, 30.0))))
        .unwrap();
    source.retire_pen();
    let mut old_idle = hover(2, 0);
    old_idle.provenance = Provenance::Coalesced;
    assert_eq!(send(&source, old_idle, None).unwrap(), Route::Suppressed);
    assert_eq!(
        send(&source, hover(3, Buttons::BARREL), None).unwrap(),
        Route::Suppressed
    );
    let mut eraser = raw(4, Phase::Move);
    eraser.identity.pointer += 1;
    eraser.identity.kind = paintcraft_input::tablet::DeviceKind::Eraser;
    eraser.buttons = Buttons(Buttons::ERASER);
    eraser.logical_position = None;
    assert_eq!(send(&source, eraser, None).unwrap(), Route::Suppressed);
}

#[test]
fn historical_snapshots_do_not_consume_actual_idle_to_held_transition_proof() {
    let source = source(&SharedBudget::new(1 << 20));
    send(&source, hover(1, 0), None).unwrap();
    let mut history = hover(2, Buttons::BARREL);
    history.provenance = Provenance::Coalesced;
    assert_eq!(send(&source, history, None).unwrap(), Route::Suppressed);
    assert_eq!(
        send(&source, hover(3, Buttons::BARREL), None).unwrap(),
        Route::Ui
    );
    let batch = source.take(1, view().owner, false).unwrap();
    let presses: Vec<_> = batch
        .frame
        .ui
        .iter()
        .filter(|record| matches!(record.event, UiEvent::Button { pressed: true, .. }))
        .map(|r| r.sequence)
        .collect();
    assert_eq!(presses, [Some(3)]);
}
