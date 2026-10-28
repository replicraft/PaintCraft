use super::*;
mod button_edges;
mod metadata;
mod overflow;
fn event(kind: Kind, buttons: u16) -> Pointer {
    Pointer {
        id: 4,
        kind,
        at: Some((12.25, 19.75)),
        stamp: 100.5,
        buttons,
        button: match kind {
            Kind::Down if buttons & Buttons::ERASER != 0 => 5,
            Kind::Down if buttons & Buttons::TIP != 0 => 0,
            Kind::Down if buttons & Buttons::BARREL != 0 => 2,
            Kind::Down => 1,
            Kind::Up => 0,
            _ => -1,
        },
        pressure: 0.25,
        tilt: (12.0, -8.0),
        twist: 31.0,
        contact: (2.5, 1.5),
        modifiers: Modifiers::default(),
    }
}
#[test]
fn barrel_first_then_tip_move_is_real_contact_and_tip_up_keeps_ui_barrel() {
    let mut n = Normalizer::new(3);
    assert_eq!(
        n.sample(event(Kind::Down, Buttons::BARREL), Provenance::Actual)
            .unwrap()
            .phase,
        Phase::Hover
    );
    let down = n
        .sample(
            event(Kind::Move, Buttons::BARREL | Buttons::TIP),
            Provenance::Actual,
        )
        .unwrap();
    assert_eq!(down.phase, Phase::Down);
    assert!(!n.starting(4, Buttons::TIP));
    assert_eq!(down.pressure, Some(0.25));
    assert_eq!(down.logical_position, Some((12.25, 19.75)));
    assert_eq!(down.tilt, Some(Tilt::Degrees(12.0, -8.0)));
    assert_eq!(down.twist_degrees, Some(31.0));
    assert_eq!(down.time.micros, 100_500);
    assert_eq!(
        n.sample(event(Kind::Move, Buttons::BARREL), Provenance::Actual)
            .unwrap()
            .phase,
        Phase::Up
    );
    assert_eq!(
        n.sample(event(Kind::Up, 0), Provenance::Actual)
            .unwrap()
            .phase,
        Phase::Hover
    );
}
#[test]
fn inverted_kind_survives_up_and_bad_end_flip_is_refused() {
    let mut n = Normalizer::new(3);
    let down = n
        .sample(event(Kind::Down, Buttons::ERASER), Provenance::Actual)
        .unwrap();
    assert_eq!(down.identity.kind, DeviceKind::Eraser);
    assert_eq!(
        n.sample(event(Kind::Move, Buttons::TIP), Provenance::Actual),
        Err(Invalid::Lifecycle)
    );
    let up = n.sample(event(Kind::Up, 0), Provenance::Actual).unwrap();
    assert_eq!(up.phase, Phase::Up);
    assert_eq!(up.identity.kind, DeviceKind::Eraser);
}
#[test]
fn coalesced_semantics_and_invalid_source_are_not_replaced_by_mouse_defaults() {
    let mut n = Normalizer::new(3);
    n.sample(event(Kind::Down, Buttons::TIP), Provenance::Actual)
        .unwrap();
    let sample = n
        .sample(event(Kind::Move, Buttons::TIP), Provenance::Coalesced)
        .unwrap();
    assert_eq!(sample.provenance, Provenance::Coalesced);
    let mut bad = event(Kind::Move, Buttons::TIP);
    bad.pressure = f32::NAN;
    assert_eq!(n.sample(bad, Provenance::Actual), Err(Invalid::Pressure));
    bad = event(Kind::Move, Buttons::TIP);
    bad.stamp = -1.0;
    assert_eq!(n.sample(bad, Provenance::Actual), Err(Invalid::Clock));
}
#[test]
fn compatibility_pairing_never_suppresses_genuine_mouse_or_an_unpaired_event() {
    let pair = Pair {
        kind: Kind::Move,
        stamp: 100.5_f64.to_bits(),
        buttons: 0,
        trusted: true,
    };
    let mut gate = Compatibility::default();
    gate.pen(pair);
    assert!(gate.matches(pair));
    assert!(!gate.matches(pair));
    gate.pen(pair);
    gate.mouse();
    assert!(!gate.matches(pair));
    gate.pen(pair);
    assert!(!gate.matches(Pair {
        trusted: false,
        ..pair
    }));
    gate.pen(pair);
    assert!(!gate.matches(Pair { buttons: 1, ..pair }));
}
#[test]
fn retired_pointer_ids_are_reused_without_discarding_any_held_end_identity() {
    let mut n = Normalizer::new(3);
    let mut held = event(Kind::Down, Buttons::ERASER | Buttons::BARREL);
    held.id = 77;
    n.sample(held, Provenance::Actual).unwrap();
    for id in 0..40 {
        let mut down = event(Kind::Down, Buttons::TIP);
        down.id = id;
        assert_eq!(
            n.sample(down, Provenance::Actual).unwrap().phase,
            Phase::Down
        );
        let mut up = event(Kind::Up, 0);
        up.id = id;
        assert_eq!(n.sample(up, Provenance::Actual).unwrap().phase, Phase::Up);
        assert_eq!(
            n.sample(up, Provenance::Actual).unwrap().phase,
            Phase::Hover
        );
    }
    let mut tip_up = event(Kind::Move, Buttons::BARREL);
    tip_up.id = 77;
    let up = n.sample(tip_up, Provenance::Actual).unwrap();
    assert_eq!(up.phase, Phase::Up);
    assert_eq!(up.identity.kind, DeviceKind::Eraser);
    let mut barrel_up = event(Kind::Up, 0);
    barrel_up.id = 77;
    assert_eq!(
        n.sample(barrel_up, Provenance::Actual)
            .unwrap()
            .identity
            .kind,
        DeviceKind::Eraser
    );
    n.clear();
    assert_eq!(
        n.sample(event(Kind::Leave, 0), Provenance::Actual)
            .unwrap()
            .phase,
        Phase::Leave
    );
}

#[test]
fn predicted_terminal_cannot_close_actual_contact() {
    let mut n = Normalizer::new(3);
    n.sample(event(Kind::Down, Buttons::TIP), Provenance::Actual)
        .unwrap();
    assert_eq!(
        n.sample(event(Kind::Up, 0), Provenance::Predicted),
        Err(Invalid::Lifecycle)
    );
    assert_eq!(
        n.sample(event(Kind::Leave, 0), Provenance::Predicted),
        Err(Invalid::Lifecycle)
    );
    assert_eq!(
        n.sample(event(Kind::Move, Buttons::TIP), Provenance::Actual)
            .unwrap()
            .phase,
        Phase::Move
    );
    assert_eq!(
        n.sample(event(Kind::Up, 0), Provenance::Actual)
            .unwrap()
            .phase,
        Phase::Up
    );
}
#[test]
fn capture_focus_and_resize_retirement_preserve_tip_eraser_and_barrel_tails() {
    for buttons in [
        Buttons::TIP,
        Buttons::ERASER,
        Buttons::BARREL,
        Buttons::TIP | Buttons::BARREL | Buttons::BARREL_UPPER,
    ] {
        let mut n = Normalizer::new(3);
        n.sample(event(Kind::Down, buttons), Provenance::Actual)
            .unwrap();
        // Shared by managed pointercancel/lostcapture, blur, resize and teardown.
        n.clear();
        assert!(n.fenced(event(Kind::Move, buttons)));
        assert!(n.fenced(event(Kind::Up, Buttons::BARREL)));
        assert!(n.fenced(event(Kind::Move, Buttons::BARREL)));
        assert!(n.fenced(event(Kind::Up, 0)));
        assert!(!n.fenced(event(Kind::Down, Buttons::TIP)));
        assert_eq!(
            n.sample(event(Kind::Down, Buttons::TIP), Provenance::Actual)
                .unwrap()
                .phase,
            Phase::Down
        );
        assert_eq!(
            n.sample(event(Kind::Up, 0), Provenance::Actual)
                .unwrap()
                .phase,
            Phase::Up
        );
    }
}
#[test]
fn orphan_held_motion_cannot_invent_a_contact_after_lost_identity() {
    let mut n = Normalizer::new(3);
    assert_eq!(
        n.sample(event(Kind::Move, Buttons::TIP), Provenance::Actual),
        Err(Invalid::Lifecycle)
    );
    let mut terminal = event(Kind::Up, 0);
    terminal.at = None;
    assert!(n.ingest(terminal, Provenance::Actual).unwrap().1);
    assert_eq!(
        n.sample(event(Kind::Down, Buttons::TIP), Provenance::Actual)
            .unwrap()
            .phase,
        Phase::Down
    );
}
#[test]
fn host_retirement_epoch_fences_held_contact_across_toolbar_until_release() {
    for buttons in [
        Buttons::TIP,
        Buttons::ERASER,
        Buttons::BARREL,
        Buttons::TIP | Buttons::BARREL_UPPER,
    ] {
        let mut n = Normalizer::new(3);
        n.observe_cancellation(10);
        n.sample(event(Kind::Down, buttons), Provenance::Actual)
            .unwrap();
        // Escape/IME/owner/skip retirement all call the same Source.cancel.
        n.observe_cancellation(11);
        let mut over_toolbar = event(Kind::Move, buttons);
        over_toolbar.at = Some((2.0, 2.0));
        assert!(n.fenced(over_toolbar));
        n.observe_cancellation(11); // Repeated GUI pass must not release tail.
        assert!(n.fenced(event(Kind::Up, Buttons::BARREL)));
        assert!(n.fenced(event(Kind::Up, 0)));
        assert_eq!(
            n.sample(event(Kind::Down, Buttons::TIP), Provenance::Actual)
                .unwrap()
                .phase,
            Phase::Down
        );
    }
}
#[test]
fn mouse_owned_denial_keeps_pen_and_hover_barrel_tail_fenced_after_mouse_release() {
    for buttons in [
        Buttons::TIP,
        Buttons::ERASER,
        Buttons::BARREL,
        Buttons::BARREL_UPPER,
        Buttons::TIP | Buttons::BARREL,
    ] {
        let mut n = Normalizer::new(3);
        let down = n
            .sample(event(Kind::Down, buttons), Provenance::Actual)
            .unwrap();
        // Source rejected this held stream because genuine mouse owned it.
        n.deny_contact(down.identity.pointer);
        // Later mouse release is not permission to synthesize a UI press.
        assert!(n.fenced(event(Kind::Move, buttons)));
        assert!(n.fenced(event(Kind::Up, Buttons::BARREL)));
        assert!(n.fenced(event(Kind::Move, Buttons::BARREL)));
        assert!(n.fenced(event(Kind::Up, 0)));
        assert_eq!(
            n.sample(event(Kind::Down, Buttons::TIP), Provenance::Actual)
                .unwrap()
                .phase,
            Phase::Down
        );
    }
}
