use super::*;
use paintcraft_input::{Modifiers, tablet::*};
pub(super) fn time() -> Time {
    Time {
        epoch: 7,
        micros: 500,
        clock: Clock::AppKitUptime,
    }
}
fn mouse(ordinal: u64, kind: MouseKind) -> CaptureRecord {
    CaptureRecord {
        installation: 7,
        ordinal,
        event: CaptureEvent::Mouse(MouseRecord {
            kind,
            position: WindowPosition::PhysicalPixels(10.25, 11.75),
            client: true,
            modifiers: Modifiers::default(),
            time: time(),
        }),
    }
}
pub(super) fn pen(ordinal: u64, phase: Phase, buttons: u16) -> CaptureRecord {
    CaptureRecord {
        installation: 7,
        ordinal,
        event: CaptureEvent::Tablet(NativePacket {
            button_edge: None,
            raw: RawSample {
                identity: Identity {
                    device: 1,
                    pointer: 2,
                    kind: DeviceKind::Pen,
                },
                sequence: ordinal,
                time: time(),
                phase,
                buttons: Buttons(buttons),
                logical_position: None,
                modifiers: Modifiers::default(),
                pressure: Some(0.0),
                tilt: Some(Tilt::Normalized(0.25, -0.5)),
                twist_degrees: Some(37.0),
                contact: None,
                provenance: Provenance::Actual,
            },
            position: Some(WindowPosition::PhysicalPixels(10.25, 11.75)),
            delivery: Delivery::ReceiverPointer,
        }),
    }
}
fn collector() -> Collector {
    Collector::new(32, &SharedBudget::new(1 << 20)).unwrap()
}
fn routes(frame: &CollectedFrame) -> Vec<PenRoute> {
    frame
        .records
        .iter()
        .filter_map(|r| match r {
            Collected::Tablet { route, .. } => Some(*route),
            _ => None,
        })
        .collect()
}
#[test]
fn earlier_mouse_hold_beats_pen_before_any_gui_frame_and_terminal_is_preserved() {
    let mut c = collector();
    c.accept(mouse(
        0,
        MouseKind::Button {
            number: 0,
            pressed: true,
        },
    ))
    .unwrap();
    c.accept(pen(1, Phase::Down, 1)).unwrap();
    c.accept(mouse(
        2,
        MouseKind::Button {
            number: 0,
            pressed: false,
        },
    ))
    .unwrap();
    c.accept(pen(3, Phase::Move, 1)).unwrap();
    c.accept(pen(4, Phase::Up, 0)).unwrap();
    c.accept(pen(5, Phase::Down, 1)).unwrap();
    let frame = c.take(DeliveryHealth::Ready).unwrap();
    assert_eq!(
        routes(&frame),
        [
            PenRoute::Suppressed,
            PenRoute::Suppressed,
            PenRoute::Suppressed,
            PenRoute::Admitted
        ]
    );
    assert!(matches!(
        frame.records[2],
        Collected::Mouse {
            record: CaptureRecord {
                ordinal: 2,
                event: CaptureEvent::Mouse(MouseRecord {
                    kind: MouseKind::Button { pressed: false, .. },
                    ..
                }),
                ..
            },
            route: MouseRoute::Client,
            ..
        }
    ));
}
#[test]
fn mouse_up_then_pen_down_and_pen_up_then_mouse_down_keep_one_actual_order() {
    for pen_first in [false, true] {
        let mut c = collector();
        let data = if pen_first {
            [
                pen(0, Phase::Down, 1),
                pen(1, Phase::Up, 0),
                mouse(
                    2,
                    MouseKind::Button {
                        number: 2,
                        pressed: true,
                    },
                ),
            ]
        } else {
            [
                mouse(
                    0,
                    MouseKind::Button {
                        number: 2,
                        pressed: true,
                    },
                ),
                mouse(
                    1,
                    MouseKind::Button {
                        number: 2,
                        pressed: false,
                    },
                ),
                pen(2, Phase::Down, 1),
            ]
        };
        for record in data {
            c.accept(record).unwrap();
        }
        let frame = c.take(DeliveryHealth::Ready).unwrap();
        for (actual, expected) in frame.records.iter().zip(data) {
            match actual {
                Collected::Tablet { record, route } => {
                    assert_eq!(*record, expected);
                    assert_eq!(*route, PenRoute::Admitted);
                }
                Collected::Mouse { record, route, .. } => {
                    assert_eq!(*record, expected);
                    assert_eq!(*route, MouseRoute::Client);
                }
                _ => panic!("Unexpected boundary"),
            }
        }
    }
}
#[test]
fn all_five_buttons_preserve_chords_and_native_chrome_initial_route() {
    let mut c = collector();
    for number in 0..5 {
        c.accept(mouse(
            number.into(),
            MouseKind::Button {
                number,
                pressed: true,
            },
        ))
        .unwrap();
    }
    assert_eq!(c.mouse_buttons(), 31);
    for number in 0..4 {
        c.accept(mouse(
            5 + u64::from(number),
            MouseKind::Button {
                number,
                pressed: false,
            },
        ))
        .unwrap();
    }
    assert_eq!(c.mouse_buttons(), 16);
    c.accept(pen(9, Phase::Down, 1)).unwrap();
    c.accept(mouse(
        10,
        MouseKind::Button {
            number: 4,
            pressed: false,
        },
    ))
    .unwrap();
    assert_eq!(c.mouse_buttons(), 0);
    assert_eq!(
        routes(&c.take(DeliveryHealth::Ready).unwrap()),
        [PenRoute::Suppressed]
    );
    let mut c = collector();
    let mut chrome = mouse(
        0,
        MouseKind::Button {
            number: 0,
            pressed: true,
        },
    );
    if let CaptureEvent::Mouse(record) = &mut chrome.event {
        record.client = false;
    }
    c.accept(chrome).unwrap();
    c.accept(mouse(1, MouseKind::Motion)).unwrap();
    c.accept(mouse(
        2,
        MouseKind::Button {
            number: 0,
            pressed: false,
        },
    ))
    .unwrap();
    assert!(
        c.take(DeliveryHealth::Ready)
            .unwrap()
            .records
            .iter()
            .all(|r| matches!(
                r,
                Collected::Mouse {
                    route: MouseRoute::NativeChrome,
                    ..
                }
            ))
    );
}
#[test]
fn earlier_pen_blocks_mouse_ui_motion_and_release_even_after_pen_lifts() {
    let mut c = collector();
    c.accept(pen(0, Phase::Down, 1)).unwrap();
    c.accept(mouse(
        1,
        MouseKind::Button {
            number: 0,
            pressed: true,
        },
    ))
    .unwrap();
    c.accept(mouse(2, MouseKind::Motion)).unwrap();
    c.accept(pen(3, Phase::Up, 0)).unwrap();
    c.accept(mouse(
        4,
        MouseKind::Button {
            number: 0,
            pressed: false,
        },
    ))
    .unwrap();
    assert!(
        c.take(DeliveryHealth::Ready)
            .unwrap()
            .records
            .iter()
            .filter_map(|r| match r {
                Collected::Mouse { route, .. } => Some(*route),
                _ => None,
            })
            .all(|r| r == MouseRoute::Suppressed)
    );
}
#[test]
fn precise_wheel_retains_actual_unit_momentum_phase_and_fences_pen() {
    let mut c = collector();
    c.accept(pen(0, Phase::Down, 1)).unwrap();
    let wheel = mouse(
        1,
        MouseKind::Wheel {
            delta: (1.25, -2.5),
            precise: true,
            physical_scale: 2.0,
            phase: WheelPhase::Move,
            momentum: true,
        },
    );
    c.accept(wheel).unwrap();
    c.accept(pen(2, Phase::Move, 1)).unwrap();
    let frame = c.take(DeliveryHealth::Ready).unwrap();
    assert!(
        matches!(frame.records[1], Collected::Mouse { record, cancel_pen: true, route: MouseRoute::Client } if record == wheel)
    );
    assert_eq!(routes(&frame), [PenRoute::Admitted, PenRoute::Suppressed]);
}
#[test]
fn keyboard_ime_and_reentrant_boundaries_do_not_fabricate_native_order_or_release_mouse() {
    for kind in [
        Boundary::Keyboard,
        Boundary::ImeReceipt,
        Boundary::Reentrant,
    ] {
        let mut c = collector();
        c.accept(pen(0, Phase::Down, 1)).unwrap();
        c.framework_boundary(kind).unwrap();
        c.accept(pen(1, Phase::Move, 1)).unwrap();
        c.accept(pen(2, Phase::Up, 0)).unwrap();
        c.accept(pen(3, Phase::Down, 1)).unwrap();
        let frame = c.take(DeliveryHealth::Ready).unwrap();
        assert_eq!(frame.records[1], Collected::FrameworkBoundary(kind));
        assert_eq!(
            routes(&frame),
            [
                PenRoute::Admitted,
                PenRoute::Suppressed,
                PenRoute::Suppressed,
                PenRoute::Admitted
            ]
        );
        let mut c = collector();
        c.accept(mouse(
            0,
            MouseKind::Button {
                number: 0,
                pressed: true,
            },
        ))
        .unwrap();
        c.framework_boundary(kind).unwrap();
        assert_eq!(c.mouse_buttons(), 1);
        c.accept(mouse(
            1,
            MouseKind::Button {
                number: 0,
                pressed: false,
            },
        ))
        .unwrap();
        assert_eq!(c.mouse_buttons(), 0);
    }
}
#[test]
fn ordering_capacity_and_delivery_failure_are_sticky_and_have_no_partial_handoff() {
    let mut c = collector();
    c.accept(pen(0, Phase::Down, 1)).unwrap();
    assert_eq!(c.accept(pen(2, Phase::Up, 0)), Err(RecordFault::Invalid));
    let frame = c.take(DeliveryHealth::Ready).unwrap();
    assert!(frame.failed);
    assert!(frame.records.is_empty());
    assert_eq!(c.accept(pen(1, Phase::Up, 0)), Err(RecordFault::Retired));
    let mut c = Collector::new(4, &SharedBudget::new(1 << 20)).unwrap();
    for n in 0..4 {
        c.accept(mouse(n, MouseKind::Motion)).unwrap();
    }
    assert_eq!(c.accept(pen(4, Phase::Down, 1)), Err(RecordFault::Capacity));
    assert!(c.take(DeliveryHealth::Ready).unwrap().records.is_empty());
    let mut c = collector();
    c.accept(pen(0, Phase::Down, 1)).unwrap();
    c.accept(pen(1, Phase::Up, 0)).unwrap();
    let frame = c.take(DeliveryHealth::Failed).unwrap();
    assert!(frame.failed);
    assert!(frame.records.is_empty());
    assert!(Collector::new(32, &SharedBudget::new(1)).is_err());
}

#[test]
fn lifecycle_controls_survive_admission_and_temporal_corruption_does_not_publish() {
    let mut c = collector();
    c.accept(pen(0, Phase::Down, 1)).unwrap();
    let mut cancel = pen(1, Phase::Cancel, 0);
    if let CaptureEvent::Tablet(packet) = &mut cancel.event {
        packet.position = None;
        packet.raw.provenance = Provenance::Boundary;
    }
    c.accept(cancel).unwrap();
    c.accept(pen(2, Phase::Move, 1)).unwrap();
    let frame = c.take(DeliveryHealth::Ready).unwrap();
    assert_eq!(
        routes(&frame),
        [PenRoute::Admitted, PenRoute::Control, PenRoute::Suppressed]
    );
    let mut c = collector();
    c.accept(pen(0, Phase::Down, 1)).unwrap();
    let mut backwards = pen(1, Phase::Move, 1);
    if let CaptureEvent::Tablet(packet) = &mut backwards.event {
        packet.raw.time.micros = 499;
    }
    assert_eq!(c.accept(backwards), Err(RecordFault::Invalid));
    assert!(c.take(DeliveryHealth::Ready).unwrap().records.is_empty());
    let mut c = collector();
    let mut predicted = pen(0, Phase::Down, 1);
    if let CaptureEvent::Tablet(packet) = &mut predicted.event {
        packet.raw.provenance = Provenance::Predicted;
    }
    assert_eq!(c.accept(predicted), Err(RecordFault::Invalid));
    assert!(c.pen_owner().is_none());
}

#[test]
fn handed_off_records_keep_their_logical_lease_after_collector_retirement() {
    let budget = SharedBudget::new(1 << 20);
    let mut c = Collector::new(4, &budget).unwrap();
    let mut inverted = pen(0, Phase::Down, Buttons::TIP | Buttons::BARREL_UPPER);
    if let CaptureEvent::Tablet(packet) = &mut inverted.event {
        packet.raw.identity.kind = DeviceKind::Eraser;
    }
    c.accept(inverted).unwrap();
    let frame = c.take(DeliveryHealth::Ready).unwrap();
    assert!(
        matches!(frame.records[0], Collected::Tablet { record, route: PenRoute::Admitted } if record == inverted)
    );
    drop(c);
    assert!(budget.usage().reserved > 0);
    assert_eq!(frame.records.len(), 1);
    drop(frame);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn reused_pen_identity_keeps_monotonic_clock_after_up_and_handoff() {
    for changed_clock in [false, true] {
        let mut c = collector();
        c.accept(pen(0, Phase::Down, 1)).unwrap();
        c.accept(pen(1, Phase::Up, 0)).unwrap();
        drop(c.take(DeliveryHealth::Ready).unwrap());
        let mut restarted = pen(2, Phase::Down, 1);
        if let CaptureEvent::Tablet(packet) = &mut restarted.event {
            if changed_clock {
                packet.raw.time.clock = Clock::WindowsTicks;
            } else {
                packet.raw.time.micros = 499;
            }
        }
        assert_eq!(c.accept(restarted), Err(RecordFault::Invalid));
        let frame = c.take(DeliveryHealth::Ready).unwrap();
        assert!(frame.failed);
        assert!(frame.records.is_empty());
    }
    let mut c = collector();
    c.accept(pen(0, Phase::Down, 1)).unwrap();
    c.accept(pen(1, Phase::Up, 0)).unwrap();
    drop(c.take(DeliveryHealth::Ready).unwrap());
    c.accept(pen(2, Phase::Down, 1)).unwrap();
    assert_eq!(
        routes(&c.take(DeliveryHealth::Ready).unwrap()),
        [PenRoute::Admitted]
    );
}

fn boundary(ordinal: u64, kind: Boundary) -> CaptureRecord {
    CaptureRecord {
        installation: 7,
        ordinal,
        event: CaptureEvent::Boundary { kind, time: None },
    }
}
#[test]
fn observed_idle_recovers_missing_outside_mouse_up_without_inventing_a_terminal() {
    let mut c = collector();
    c.accept(mouse(
        0,
        MouseKind::Button {
            number: 0,
            pressed: true,
        },
    ))
    .unwrap();
    c.accept(boundary(1, Boundary::FocusLost)).unwrap();
    assert_eq!(c.mouse_buttons(), 1);
    // There is no mouse Up in this source stream. Only actual native idle
    // observation can retire that lost hold; proximity identity is unchanged.
    c.accept(boundary(2, Boundary::PointerIdle)).unwrap();
    assert_eq!(c.mouse_buttons(), 0);
    c.accept(pen(3, Phase::Down, 1)).unwrap();
    c.accept(pen(4, Phase::Up, 0)).unwrap();
    let f = c.take(DeliveryHealth::Ready).unwrap();
    assert_eq!(routes(&f), [PenRoute::Admitted, PenRoute::Admitted]);
    assert_eq!(
        f.records
            .iter()
            .filter(|r| matches!(r, Collected::Mouse { .. }))
            .count(),
        1
    );
}
#[test]
fn recovery_does_not_resurrect_a_pen_tail_or_accept_receipt_as_native_idle() {
    let mut c = collector();
    c.accept(mouse(
        0,
        MouseKind::Button {
            number: 0,
            pressed: true,
        },
    ))
    .unwrap();
    c.accept(boundary(1, Boundary::FocusLost)).unwrap();
    c.accept(pen(2, Phase::Down, 1)).unwrap();
    c.accept(boundary(3, Boundary::PointerIdle)).unwrap();
    c.accept(pen(4, Phase::Move, 1)).unwrap();
    c.accept(pen(5, Phase::Up, 0)).unwrap();
    c.accept(pen(6, Phase::Down, 1)).unwrap();
    c.accept(pen(7, Phase::Up, 0)).unwrap();
    let f = c.take(DeliveryHealth::Ready).unwrap();
    assert_eq!(
        routes(&f),
        [
            PenRoute::Suppressed,
            PenRoute::Suppressed,
            PenRoute::Suppressed,
            PenRoute::Admitted,
            PenRoute::Admitted
        ]
    );
    assert_eq!(
        c.framework_boundary(Boundary::PointerIdle),
        Err(RecordFault::Invalid)
    );
    assert!(c.failed());
}
#[test]
fn closed_source_cannot_resume_but_a_fresh_idle_installation_can() {
    let mut c = collector();
    c.accept(boundary(0, Boundary::WindowClosed)).unwrap();
    assert_eq!(
        c.accept(boundary(1, Boundary::PointerIdle)),
        Err(RecordFault::Retired)
    );
    let f = c.take(DeliveryHealth::Ready).unwrap();
    assert_eq!(f.records.len(), 1);
    let mut fresh = collector();
    fresh.accept(pen(0, Phase::Down, 1)).unwrap();
    assert_eq!(
        routes(&fresh.take(DeliveryHealth::Ready).unwrap()),
        [PenRoute::Admitted]
    );
}

#[test]
fn focus_without_a_local_mouse_hold_does_not_require_an_idle_ritual() {
    let mut c = collector();
    c.accept(pen(0, Phase::Hover, 0)).unwrap();
    c.accept(boundary(1, Boundary::FocusLost)).unwrap();
    // No native PointerIdle is needed: no locally captured mouse Up was lost.
    c.accept(pen(2, Phase::Down, 1)).unwrap();
    c.accept(pen(3, Phase::Up, 0)).unwrap();
    assert_eq!(
        routes(&c.take(DeliveryHealth::Ready).unwrap()),
        [PenRoute::Admitted, PenRoute::Admitted, PenRoute::Admitted]
    );
}
