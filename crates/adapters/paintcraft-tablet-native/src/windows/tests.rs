use super::model::*;
use crate::{Delivery, WindowPosition};
use paintcraft_input::tablet::{Buttons, DeviceKind, Phase, Provenance, Tilt};

fn info(flags: u32, count: u64) -> Info {
    Info {
        device: 41,
        pointer: 7,
        frame: count as u32,
        flags: flags | IN_RANGE,
        pressure: 512,
        rotation: 359,
        tilt: (-31, 27),
        mask: 15,
        performance: count,
        ticks: count as u32,
        position: (12.25, -3.75),
        client: true,
        ..Info::default()
    }
}
fn packet(output: Output) -> crate::NativePacket {
    match output {
        Output::Packet(packet) => packet,
        other => panic!("expected packet, got {other:?}"),
    }
}

#[test]
fn mask_presence_distinguishes_zero_and_missing_axes_and_never_fabricates_contact() {
    let mut state = State::new(19, 1000);
    let mut down = info(DOWN | CONTACT, 1);
    down.mask = PRESSURE;
    down.pressure = 0;
    let p = packet(state.sample(down, Message::Down, false, 1000).unwrap());
    assert_eq!(p.raw.pressure, Some(0.0));
    assert_eq!(p.raw.tilt, None);
    assert_eq!(p.raw.twist_degrees, None);
    assert_eq!(p.raw.contact, None);
    assert_eq!(p.raw.buttons.0, Buttons::TIP);
    assert_eq!(p.raw.phase, Phase::Down);
    assert_eq!(p.raw.logical_position, None);
    assert_eq!(
        p.position,
        Some(WindowPosition::PhysicalPixels(12.25, -3.75))
    );
    assert_eq!(p.delivery, Delivery::ReceiverPointer);
    let mut movement = info(UPDATE | CONTACT, 2);
    movement.mask = 0;
    let p = packet(
        state
            .sample(movement, Message::Update, false, 1000)
            .unwrap(),
    );
    assert_eq!(p.raw.pressure, None);
    assert_eq!(p.raw.phase, Phase::Move);
    movement.performance = 3;
    movement.mask = TILT_X;
    assert_eq!(
        packet(
            state
                .sample(movement, Message::Update, false, 1000)
                .unwrap()
        )
        .raw
        .tilt,
        None
    );
}

#[test]
fn actual_pen_axes_barrel_hover_and_eraser_release_keep_their_meaning() {
    let mut state = State::new(19, 1000);
    let mut hover = info(UPDATE, 1);
    hover.pen_flags = BARREL | INVERTED;
    let p = packet(state.sample(hover, Message::Update, false, 1000).unwrap());
    assert_eq!(p.raw.phase, Phase::Hover);
    assert_eq!(p.raw.identity.kind, DeviceKind::Eraser);
    assert_eq!(p.raw.buttons.0, Buttons::BARREL | Buttons::ERASER);
    assert_eq!(p.raw.tilt, Some(Tilt::Degrees(-31.0, 27.0)));
    assert_eq!(p.raw.twist_degrees, Some(359.0));
    let mut down = info(DOWN | CONTACT, 2);
    down.pen_flags = BARREL | ERASER;
    assert_eq!(
        packet(state.sample(down, Message::Down, false, 1000).unwrap())
            .raw
            .phase,
        Phase::Down
    );
    let up = info(UP, 3);
    assert_eq!(
        packet(state.sample(up, Message::Up, false, 1000).unwrap())
            .raw
            .identity
            .kind,
        DeviceKind::Eraser
    );
}

#[test]
fn history_is_documented_oldest_first_without_duplicating_the_current_sample() {
    let mut state = State::new(19, 1000);
    state
        .sample(info(DOWN | CONTACT, 1), Message::Down, false, 1000)
        .unwrap();
    let mut out = Vec::with_capacity(8);
    state
        .history(
            &[
                info(UPDATE | CONTACT, 4),
                info(UPDATE | CONTACT, 3),
                info(UPDATE | CONTACT, 2),
            ],
            Message::Update,
            1000,
            &mut out,
        )
        .unwrap();
    let p: Vec<_> = out.iter().copied().map(packet).collect();
    assert_eq!(
        p.iter().map(|p| p.raw.time.micros).collect::<Vec<_>>(),
        [2000, 3000, 4000]
    );
    assert_eq!(
        p.iter().map(|p| p.raw.provenance).collect::<Vec<_>>(),
        [
            Provenance::Coalesced,
            Provenance::Coalesced,
            Provenance::Actual
        ]
    );
}

#[test]
fn malformed_or_truncated_history_cannot_mutate_state_or_emit_a_prefix() {
    let mut state = State::new(19, 1000);
    state
        .sample(info(DOWN | CONTACT, 1), Message::Down, false, 1000)
        .unwrap();
    let mut out = Vec::with_capacity(8);
    let mut bad = info(UPDATE | CONTACT, 3);
    bad.pressure = 1025;
    assert!(
        state
            .history(
                &[bad, info(UPDATE | CONTACT, 2)],
                Message::Update,
                1000,
                &mut out
            )
            .is_err()
    );
    assert!(out.is_empty());
    // Time 2 is still accepted: failed preparation did not install its prefix.
    state
        .sample(info(UPDATE | CONTACT, 2), Message::Update, false, 1000)
        .unwrap();
    assert!(
        state
            .history(
                &[info(UPDATE | CONTACT, 4), info(UP, 3)],
                Message::Update,
                1000,
                &mut out
            )
            .is_err()
    );
    assert!(out.is_empty());
    assert!(
        state
            .history(
                &[info(UPDATE | CONTACT, 4)],
                Message::Update,
                1000,
                &mut Vec::new()
            )
            .is_err()
    );
    assert!(state.history(&[], Message::Update, 1000, &mut out).is_err());
}

#[test]
fn capture_loss_preserves_real_terminal_then_allows_the_first_fresh_contact() {
    let mut state = State::new(19, 1000);
    state
        .sample(info(DOWN | CONTACT, 1), Message::Down, false, 1000)
        .unwrap();
    state.fence();
    assert_eq!(
        state
            .sample(info(UPDATE | CONTACT, 2), Message::Update, false, 1000)
            .unwrap(),
        Output::Suppressed
    );
    let p = packet(state.sample(info(UP, 3), Message::Up, false, 1000).unwrap());
    assert_eq!(p.raw.phase, Phase::Up);
    assert_eq!(p.raw.buttons.0, 0);
    assert!(p.position.is_some()); // actual native point, never a made-up origin
    assert_eq!(
        packet(
            state
                .sample(info(DOWN | CONTACT, 4), Message::Down, false, 1000)
                .unwrap()
        )
        .raw
        .phase,
        Phase::Down
    );
}

#[test]
fn initial_native_chrome_route_remains_native_after_crossing_client_bounds() {
    let mut state = State::new(19, 1000);
    let mut down = info(DOWN | CONTACT, 1);
    down.client = false;
    assert_eq!(
        state.sample(down, Message::Down, false, 1000).unwrap(),
        Output::Native
    );
    assert_eq!(
        state
            .sample(info(UPDATE | CONTACT, 2), Message::Update, false, 1000)
            .unwrap(),
        Output::Native
    );
    assert_eq!(
        state.sample(info(UP, 3), Message::Up, false, 1000).unwrap(),
        Output::Native
    );
    assert!(matches!(
        state
            .sample(info(DOWN | CONTACT, 4), Message::Down, false, 1000)
            .unwrap(),
        Output::Packet(_)
    ));
}

#[test]
fn orphan_contact_and_end_changes_do_not_invent_new_downs() {
    let mut state = State::new(19, 1000);
    assert!(
        state
            .sample(info(UPDATE | CONTACT, 1), Message::Update, false, 1000)
            .is_err()
    );
    state
        .sample(info(DOWN | CONTACT, 1), Message::Down, false, 1000)
        .unwrap();
    let mut flip = info(UPDATE | CONTACT, 2);
    flip.pen_flags = INVERTED;
    assert!(state.sample(flip, Message::Update, false, 1000).is_err());
    let mut rewind = info(UPDATE | CONTACT, 1);
    rewind.performance = 0;
    assert!(state.sample(rewind, Message::Update, false, 1000).is_err());
}

#[test]
fn promoted_signature_does_not_guess_from_coordinates_or_event_counts() {
    assert!(promoted(0xff51_5700));
    assert!(promoted(0xff51_5780));
    assert!(promoted(0xff51_577f));
    for ordinary in [0, 1, 0xff51_5600, 0x1234_5678] {
        assert!(!promoted(ordinary));
    }
}

#[test]
fn in_range_window_leave_retains_clock_but_true_range_end_releases_local_slot() {
    let mut state = State::new(19, 1000);
    state
        .sample(info(UPDATE, 10), Message::Update, false, 1000)
        .unwrap();
    state
        .sample(info(0, 11), Message::Leave, false, 1000)
        .unwrap();
    assert!(
        state
            .sample(info(UPDATE, 9), Message::Update, false, 1000)
            .is_err()
    );
    let mut out = info(0, 12);
    out.flags = 0;
    state.sample(out, Message::Leave, false, 1000).unwrap();
    for pointer in 1..=130 {
        let mut enter = info(UPDATE, 20);
        enter.pointer = pointer;
        state.sample(enter, Message::Enter, false, 1000).unwrap();
        let mut leave = info(0, 21);
        leave.pointer = pointer;
        leave.flags = 0;
        state.sample(leave, Message::Leave, false, 1000).unwrap();
    }
    // Native lifetime reuse starts only after the verified physical range end.
    state
        .sample(info(UPDATE, 1), Message::Enter, false, 1000)
        .unwrap();
}
#[test]
fn a_historical_barrel_edge_cannot_be_placed_before_other_retrieved_messages() {
    let mut state = State::new(19, 1000);
    state
        .sample(info(UPDATE, 1), Message::Update, false, 1000)
        .unwrap();
    let mut old = info(UPDATE, 2);
    old.pen_flags = BARREL;
    let mut output = Vec::with_capacity(8);
    assert!(
        state
            .history(&[info(UPDATE, 3), old], Message::Update, 1000, &mut output)
            .is_err()
    );
    assert!(output.is_empty());
}
#[test]
fn actual_device_removal_releases_only_that_native_devices_local_metadata() {
    let mut state = State::new(19, 1000);
    state
        .sample(info(UPDATE, 10), Message::Update, false, 1000)
        .unwrap();
    state.remove_device(42);
    assert!(
        state
            .sample(info(UPDATE, 9), Message::Update, false, 1000)
            .is_err()
    );
    state.remove_device(41);
    state
        .sample(info(UPDATE, 1), Message::Update, false, 1000)
        .unwrap();
}

#[test]
fn retirement_names_both_observed_ends_only_after_actual_range_end() {
    let mut state = State::new(19, 1000);
    state
        .sample(info(UPDATE, 1), Message::Update, false, 1000)
        .unwrap();
    let mut eraser = info(UPDATE, 2);
    eraser.pen_flags = INVERTED;
    state.sample(eraser, Message::Update, false, 1000).unwrap();
    state
        .sample(info(0, 3), Message::Leave, false, 1000)
        .unwrap();
    assert_eq!(state.retired(), [None, None]);
    let mut end = info(0, 4);
    end.flags = 0;
    end.client = false;
    state.sample(end, Message::Leave, false, 1000).unwrap();
    let retired = state.retired();
    assert_eq!(retired[0].unwrap().0.kind, DeviceKind::Pen);
    assert_eq!(retired[1].unwrap().0.kind, DeviceKind::Eraser);
    assert_eq!(retired[0].unwrap().1.micros, 4000);
    let mut native = info(UPDATE, 5);
    native.client = false;
    state.sample(native, Message::Update, false, 1000).unwrap();
    let mut end = info(0, 6);
    end.flags = 0;
    end.client = false;
    state.sample(end, Message::Leave, false, 1000).unwrap();
    assert_eq!(state.retired(), [None, None]);
}

#[test]
fn in_contact_enter_is_proximity_until_a_real_down_message_arrives() {
    let mut state = State::new(19, 1000);
    let enter = packet(
        state
            .sample(info(DOWN | CONTACT, 1), Message::Enter, false, 1000)
            .unwrap(),
    );
    assert_eq!(enter.raw.phase, Phase::Enter);
    assert_eq!(enter.raw.buttons.0 & Buttons::TIP, Buttons::TIP);
    let down = packet(
        state
            .sample(info(DOWN | CONTACT, 1), Message::Down, false, 1000)
            .unwrap(),
    );
    assert_eq!(down.raw.phase, Phase::Down);
    assert!(
        state
            .sample(info(DOWN | CONTACT, 2), Message::Down, false, 1000)
            .is_err()
    );
}

#[test]
fn contradictory_current_contact_edges_refuse_a_whole_history_without_a_prefix() {
    for (flags, message, already_down) in [
        (UP | CONTACT, Message::Up, true),
        (DOWN | UP | CONTACT, Message::Down, false),
        (DOWN | UP, Message::Down, false),
        (DOWN, Message::Down, false),
    ] {
        let mut state = State::new(19, 1000);
        let (initial, phase, valid_history) = if already_down {
            (
                info(DOWN | CONTACT, 1),
                Message::Down,
                info(UPDATE | CONTACT, 2),
            )
        } else {
            (info(UPDATE, 1), Message::Update, info(UPDATE, 2))
        };
        state.sample(initial, phase, false, 1000).unwrap();
        let mut output = Vec::with_capacity(8);
        assert!(
            state
                .history(&[info(flags, 3), valid_history], message, 1000, &mut output)
                .is_err()
        );
        assert!(output.is_empty());
        // No malformed terminal/begin may publish or mutate the prior state.
        let resumed = packet(
            state
                .sample(valid_history, Message::Update, false, 1000)
                .unwrap(),
        );
        assert_eq!(
            resumed.raw.phase,
            if already_down {
                Phase::Move
            } else {
                Phase::Hover
            }
        );
        if !already_down {
            assert_eq!(
                packet(
                    state
                        .sample(info(DOWN | CONTACT, 3), Message::Down, false, 1000)
                        .unwrap()
                )
                .raw
                .phase,
                Phase::Down
            );
        }
        assert_eq!(
            packet(state.sample(info(UP, 4), Message::Up, false, 1000).unwrap())
                .raw
                .phase,
            Phase::Up
        );
    }
}

#[test]
fn actual_cancel_and_fenced_release_preserve_valid_source_axes_without_creating_ink() {
    let mut state = State::new(19, 1000);
    state
        .sample(info(DOWN | CONTACT, 1), Message::Down, false, 1000)
        .unwrap();
    let cancelled = packet(
        state
            .sample(info(CANCELED | CONTACT, 2), Message::Update, false, 1000)
            .unwrap(),
    );
    assert_eq!(cancelled.raw.phase, Phase::Cancel);
    assert_eq!(cancelled.raw.provenance, Provenance::Actual);
    assert_eq!(cancelled.position, None);
    assert_eq!(cancelled.raw.pressure, Some(0.5));
    assert_eq!(cancelled.raw.tilt, Some(Tilt::Degrees(-31.0, 27.0)));
    assert_eq!(cancelled.raw.twist_degrees, Some(359.0));
    let released = packet(state.sample(info(UP, 3), Message::Up, false, 1000).unwrap());
    assert_eq!(released.raw.phase, Phase::Up);
    assert_eq!(released.raw.buttons.0 & Buttons::TIP, 0);
    assert_eq!(released.raw.pressure, Some(0.5));
    let mut invalid = info(CANCELED, 4);
    invalid.mask = PRESSURE;
    invalid.pressure = 1025;
    assert!(state.sample(invalid, Message::Update, false, 1000).is_err());
}
