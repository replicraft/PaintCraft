use super::*;
use crate::collector::tests::{pen, time};
use paintcraft_input::{
    Modifiers,
    tablet::{DeviceKind, FingerConfidence, PenButton, PenButtonEdge},
};
fn collector() -> Collector {
    Collector::new(64, &SharedBudget::new(1 << 20)).unwrap()
}
fn mouse(n: u64, pressed: bool) -> CaptureRecord {
    CaptureRecord {
        installation: 7,
        ordinal: n,
        event: CaptureEvent::Mouse(MouseRecord {
            kind: MouseKind::Button { number: 0, pressed },
            position: WindowPosition::PhysicalPixels(10., 10.),
            client: true,
            modifiers: Modifiers::default(),
            time: time(),
        }),
    }
}
fn finger(n: u64, phase: Phase) -> CaptureRecord {
    CaptureRecord {
        installation: 7,
        ordinal: n,
        event: CaptureEvent::Finger {
            identity: Identity {
                device: 30,
                pointer: 1,
                kind: DeviceKind::Finger,
            },
            phase,
            time: time(),
            provenance: Provenance::Actual,
            position: Some(WindowPosition::PhysicalPixels(10., 10.)),
            confidence: FingerConfidence::High,
            in_contact: matches!(phase, Phase::Down | Phase::Move),
            primary: true,
        },
    }
}
fn finger_route(c: &mut Collector, record: CaptureRecord) -> FingerRoute {
    c.accept(record).unwrap();
    let f = c.take(DeliveryHealth::Ready).unwrap();
    let Collected::Finger { route, .. } = f.records.last().unwrap() else {
        panic!("finger metadata")
    };
    *route
}
#[test]
fn first_qualified_mouse_pen_or_touch_owns_and_denied_tails_do_not_take_over() {
    for first in 0..3 {
        let mut c = collector();
        let record = match first {
            0 => mouse(0, true),
            1 => pen(0, Phase::Down, Buttons::TIP),
            _ => finger(0, Phase::Down),
        };
        c.accept(record).unwrap();
        let expected = match first {
            0 => CaptureClass::Mouse,
            1 => CaptureClass::Pen(Identity {
                device: 1,
                pointer: 2,
                kind: DeviceKind::Pen,
            }),
            _ => CaptureClass::Touch,
        };
        assert_eq!(c.capture_class(), expected);
        if first != 2 {
            assert_eq!(
                finger_route(&mut c, finger(1, Phase::Down)),
                FingerRoute::Denied(FingerDenial::CompetingPointer)
            );
        } else {
            c.accept(pen(1, Phase::Down, Buttons::TIP)).unwrap();
            assert!(matches!(
                c.take(DeliveryHealth::Ready).unwrap().records.last(),
                Some(Collected::Tablet {
                    route: PenRoute::Suppressed,
                    ..
                })
            ));
        }
        if first != 0 {
            c.accept(mouse(2, true)).unwrap();
            assert!(matches!(
                c.take(DeliveryHealth::Ready).unwrap().records.last(),
                Some(Collected::Mouse {
                    route: MouseRoute::Suppressed,
                    ..
                })
            ));
        } else {
            c.accept(pen(2, Phase::Down, Buttons::TIP)).unwrap();
            assert!(matches!(
                c.take(DeliveryHealth::Ready).unwrap().records.last(),
                Some(Collected::Tablet {
                    route: PenRoute::Suppressed,
                    ..
                })
            ));
        }
        assert_eq!(c.capture_class(), expected);
    }
}
#[test]
fn unqualified_held_barrel_is_candidate_metadata_and_cannot_steal_mouse_or_finger() {
    for touch in [false, true] {
        let mut c = collector();
        c.accept(pen(0, Phase::Hover, Buttons::BARREL)).unwrap();
        assert_eq!(c.capture_class(), CaptureClass::Idle);
        assert!(c.pen_owner().is_none());
        if touch {
            assert_eq!(
                finger_route(&mut c, finger(1, Phase::Down)),
                FingerRoute::Eligible
            );
            assert_eq!(c.capture_class(), CaptureClass::Touch);
        } else {
            c.accept(mouse(1, true)).unwrap();
            assert_eq!(c.capture_class(), CaptureClass::Mouse);
        }
        let mut actual_edge = pen(2, Phase::Hover, Buttons::BARREL);
        if let CaptureEvent::Tablet(p) = &mut actual_edge.event {
            p.button_edge = Some(PenButtonEdge {
                identity: p.raw.identity,
                button: PenButton::Barrel,
                pressed: true,
            });
        }
        c.accept(actual_edge).unwrap();
        assert!(matches!(
            c.take(DeliveryHealth::Ready).unwrap().records.last(),
            Some(Collected::Tablet {
                route: PenRoute::Suppressed,
                ..
            })
        ));
    }
}
#[test]
fn actual_idle_survives_history_then_barrel_main_qualifies_without_fake_tip_down() {
    let mut c = collector();
    c.accept(pen(0, Phase::Hover, 0)).unwrap();
    let mut history = pen(1, Phase::Hover, Buttons::BARREL);
    if let CaptureEvent::Tablet(p) = &mut history.event {
        p.raw.provenance = Provenance::Coalesced;
    }
    c.accept(history).unwrap();
    assert_eq!(c.capture_class(), CaptureClass::Idle);
    let main = pen(2, Phase::Hover, Buttons::BARREL);
    c.accept(main).unwrap();
    assert!(matches!(c.capture_class(), CaptureClass::Pen(_)));
    assert_eq!(
        finger_route(&mut c, finger(3, Phase::Down)),
        FingerRoute::Denied(FingerDenial::CompetingPointer)
    );
    c.accept(pen(4, Phase::Hover, 0)).unwrap();
    assert_eq!(c.capture_class(), CaptureClass::Idle);
}
#[test]
fn exact_actual_side_press_qualifies_both_barrels_and_contradictory_edges_refuse() {
    for button in [PenButton::Barrel, PenButton::UpperBarrel] {
        let mut c = collector();
        let mut record = pen(0, Phase::Hover, button.mask());
        if let CaptureEvent::Tablet(p) = &mut record.event {
            p.button_edge = Some(PenButtonEdge {
                identity: p.raw.identity,
                button,
                pressed: true,
            });
        }
        c.accept(record).unwrap();
        assert!(matches!(c.capture_class(), CaptureClass::Pen(_)));
        c.accept(pen(1, Phase::Hover, 0)).unwrap();
        assert_eq!(c.capture_class(), CaptureClass::Idle);
        let mut c = collector();
        let mut bad = record;
        if let CaptureEvent::Tablet(p) = &mut bad.event {
            p.raw.buttons = Buttons(0);
        }
        assert_eq!(c.accept(bad), Err(RecordFault::Invalid));
        assert!(c.take(DeliveryHealth::Ready).unwrap().records.is_empty());
    }
}
#[test]
fn eraser_identity_and_proximity_are_not_contact_and_tip_down_needs_real_tip_fact() {
    let mut c = collector();
    let mut idle = pen(0, Phase::Hover, Buttons::ERASER);
    if let CaptureEvent::Tablet(p) = &mut idle.event {
        p.raw.identity.kind = DeviceKind::Eraser;
    }
    c.accept(idle).unwrap();
    assert_eq!(c.capture_class(), CaptureClass::Idle);
    assert_eq!(
        finger_route(&mut c, finger(1, Phase::Down)),
        FingerRoute::Eligible
    );
    let mut c = collector();
    let mut false_down = pen(0, Phase::Down, Buttons::ERASER);
    if let CaptureEvent::Tablet(p) = &mut false_down.event {
        p.raw.identity.kind = DeviceKind::Eraser;
    }
    assert_eq!(c.accept(false_down), Err(RecordFault::Invalid));
    assert!(c.pen_owner().is_none());
}
#[test]
fn geometry_zero_and_internal_leave_never_fabricate_physical_terminal() {
    let mut c = collector();
    c.accept(pen(0, Phase::Down, Buttons::TIP)).unwrap();
    let mut history = pen(1, Phase::Hover, 0);
    if let CaptureEvent::Tablet(p) = &mut history.event {
        p.raw.provenance = Provenance::Coalesced;
    }
    c.accept(history).unwrap();
    assert!(matches!(c.capture_class(), CaptureClass::Pen(_)));
    c.accept(mouse(2, true)).unwrap();
    assert!(matches!(
        c.take(DeliveryHealth::Ready).unwrap().records.last(),
        Some(Collected::Mouse {
            route: MouseRoute::Suppressed,
            ..
        })
    ));
    let mut leave = pen(3, Phase::Leave, 0);
    if let CaptureEvent::Tablet(p) = &mut leave.event {
        p.raw.provenance = Provenance::Boundary;
        p.position = None;
    }
    c.accept(leave).unwrap();
    assert_eq!(c.capture_class(), CaptureClass::Idle);
    assert!(c.pens.iter().flatten().any(|p| p.held && p.fenced));
    c.accept(pen(4, Phase::Hover, 0)).unwrap();
    assert!(!c.pens.iter().flatten().any(|p| p.held || p.fenced));
}
#[test]
fn metadata_enter_is_not_denial_but_real_mouse_capture_and_uncertainty_are() {
    let mut c = collector();
    let mut enter = finger(0, Phase::Enter);
    if let CaptureEvent::Finger { in_contact, .. } = &mut enter.event {
        *in_contact = true;
    }
    assert_eq!(finger_route(&mut c, enter), FingerRoute::MetadataOnly);
    assert_eq!(c.capture_class(), CaptureClass::Idle);
    assert_eq!(
        finger_route(&mut c, finger(1, Phase::Down)),
        FingerRoute::Eligible
    );
    let mut c = collector();
    let mut chrome = mouse(0, true);
    if let CaptureEvent::Mouse(m) = &mut chrome.event {
        m.client = false;
    }
    c.accept(chrome).unwrap();
    assert_eq!(
        finger_route(&mut c, finger(1, Phase::Down)),
        FingerRoute::Denied(FingerDenial::CompetingPointer)
    );
    c.accept(CaptureRecord {
        installation: 7,
        ordinal: 2,
        event: CaptureEvent::Boundary {
            kind: Boundary::FocusLost,
            time: None,
        },
    })
    .unwrap();
    let mut other = finger(3, Phase::Down);
    if let CaptureEvent::Finger { identity, .. } = &mut other.event {
        identity.pointer = 2;
    }
    assert_eq!(
        finger_route(&mut c, other),
        FingerRoute::Denied(FingerDenial::AwaitingMouseIdle)
    );
}
#[test]
fn wheel_retirement_describes_only_current_held_class_not_earlier_completed_work() {
    for touch in [false, true] {
        let mut c = collector();
        let begin = if touch {
            finger(0, Phase::Down)
        } else {
            pen(0, Phase::Down, Buttons::TIP)
        };
        c.accept(begin).unwrap();
        let end = if touch {
            finger(1, Phase::Up)
        } else {
            pen(1, Phase::Up, 0)
        };
        c.accept(end).unwrap();
        let completed = c.take(DeliveryHealth::Ready).unwrap();
        assert_eq!(c.capture_class(), CaptureClass::Idle);
        let wheel = |ordinal| CaptureRecord {
            installation: 7,
            ordinal,
            event: CaptureEvent::Mouse(MouseRecord {
                kind: MouseKind::Wheel {
                    delta: (0., 1.),
                    precise: false,
                    physical_scale: 1.,
                    phase: WheelPhase::Move,
                    momentum: false,
                },
                position: WindowPosition::PhysicalPixels(10., 10.),
                client: true,
                modifiers: Modifiers::default(),
                time: time(),
            }),
        };
        c.accept(wheel(2)).unwrap();
        assert!(matches!(
            c.take(DeliveryHealth::Ready).unwrap().records.last(),
            Some(Collected::Mouse {
                retire_capture: None,
                cancel_pen: false,
                ..
            })
        ));
        let fresh = if touch {
            finger(3, Phase::Down)
        } else {
            pen(3, Phase::Down, Buttons::TIP)
        };
        c.accept(fresh).unwrap();
        c.accept(wheel(4)).unwrap();
        let f = c.take(DeliveryHealth::Ready).unwrap();
        assert!(
            matches!(
                f.records.last(),
                Some(Collected::Mouse {
                    retire_capture: Some(CaptureClass::Touch),
                    cancel_pen: false,
                    ..
                })
            ) == touch
        );
        if !touch {
            assert!(matches!(
                f.records.last(),
                Some(Collected::Mouse {
                    retire_capture: Some(CaptureClass::Pen(_)),
                    cancel_pen: true,
                    ..
                })
            ));
        }
        assert_eq!(completed.records.len(), 2);
        assert_eq!(c.capture_class(), CaptureClass::Idle);
    }
}

#[test]
fn history_cannot_create_idle_and_fresh_actual_edge_only_recovers_orphan_without_takeover() {
    let mut c = collector();
    let mut history = pen(0, Phase::Hover, 0);
    if let CaptureEvent::Tablet(p) = &mut history.event {
        p.raw.provenance = Provenance::Coalesced;
    }
    c.accept(history).unwrap();
    c.accept(pen(1, Phase::Hover, Buttons::BARREL)).unwrap();
    assert_eq!(c.capture_class(), CaptureClass::Idle);
    let f = c.take(DeliveryHealth::Ready).unwrap();
    assert!(
        matches!(f.records.last(),Some(Collected::Tablet {route:PenRoute::Admitted,record,..}) if matches!(record.event,CaptureEvent::Tablet(p) if p.raw.phase==Phase::Hover))
    );
    let mut edge = pen(2, Phase::Hover, Buttons::BARREL);
    if let CaptureEvent::Tablet(p) = &mut edge.event {
        p.button_edge = Some(PenButtonEdge {
            identity: p.raw.identity,
            button: PenButton::Barrel,
            pressed: true,
        });
    }
    c.accept(edge).unwrap();
    assert!(matches!(c.capture_class(), CaptureClass::Pen(_)));
    let mut c = collector();
    c.accept(pen(0, Phase::Hover, 0)).unwrap();
    c.accept(mouse(1, true)).unwrap();
    c.accept(mouse(2, false)).unwrap();
    c.accept(pen(3, Phase::Hover, Buttons::BARREL)).unwrap();
    assert_eq!(c.capture_class(), CaptureClass::Idle);
}
#[test]
fn actual_clock_retirement_discards_idle_proof_and_chord_release_keeps_mouse_priority() {
    let mut c = collector();
    let idle = pen(0, Phase::Hover, 0);
    c.accept(idle).unwrap();
    let CaptureEvent::Tablet(p) = idle.event else {
        unreachable!()
    };
    c.accept(CaptureRecord {
        installation: 7,
        ordinal: 1,
        event: CaptureEvent::IdentityRetired {
            identity: p.raw.identity,
            time: p.raw.time,
        },
    })
    .unwrap();
    c.accept(pen(2, Phase::Hover, Buttons::BARREL)).unwrap();
    assert_eq!(c.capture_class(), CaptureClass::Idle);
    let mut c = collector();
    c.accept(mouse(0, true)).unwrap();
    let mut chord = mouse(1, true);
    if let CaptureEvent::Mouse(m) = &mut chord.event {
        m.kind = MouseKind::Button {
            number: 2,
            pressed: true,
        };
    }
    c.accept(chord).unwrap();
    c.accept(mouse(2, false)).unwrap();
    assert_eq!(c.capture_class(), CaptureClass::Mouse);
    assert_eq!(
        finger_route(&mut c, finger(3, Phase::Down)),
        FingerRoute::Denied(FingerDenial::CompetingPointer)
    );
    let mut up = mouse(4, false);
    if let CaptureEvent::Mouse(m) = &mut up.event {
        m.kind = MouseKind::Button {
            number: 2,
            pressed: false,
        };
    }
    c.accept(up).unwrap();
    assert_eq!(c.capture_class(), CaptureClass::Idle);
    assert_eq!(
        finger_route(&mut c, finger(5, Phase::Move)),
        FingerRoute::Denied(FingerDenial::CompetingPointer)
    );
}
#[test]
fn system_routed_qualified_pen_still_owns_physical_capture() {
    let mut c = collector();
    let mut record = pen(0, Phase::Down, Buttons::TIP);
    if let CaptureEvent::Tablet(p) = &mut record.event {
        p.delivery = Delivery::SystemPointer;
    }
    c.accept(record).unwrap();
    assert!(matches!(c.capture_class(), CaptureClass::Pen(_)));
    assert_eq!(
        finger_route(&mut c, finger(1, Phase::Down)),
        FingerRoute::Denied(FingerDenial::CompetingPointer)
    );
}
#[test]
fn another_qualified_pen_is_also_a_real_takeover_of_an_orphan_tail() {
    let mut c = collector();
    c.accept(pen(0, Phase::Hover, Buttons::BARREL)).unwrap();
    let mut second = pen(1, Phase::Down, Buttons::TIP);
    if let CaptureEvent::Tablet(p) = &mut second.event {
        p.raw.identity.device = 2;
    }
    c.accept(second).unwrap();
    let mut up = pen(2, Phase::Up, 0);
    if let CaptureEvent::Tablet(p) = &mut up.event {
        p.raw.identity.device = 2;
    }
    c.accept(up).unwrap();
    let mut edge = pen(3, Phase::Hover, Buttons::BARREL);
    if let CaptureEvent::Tablet(p) = &mut edge.event {
        p.button_edge = Some(PenButtonEdge {
            identity: p.raw.identity,
            button: PenButton::Barrel,
            pressed: true,
        });
    }
    c.accept(edge).unwrap();
    assert!(matches!(
        c.take(DeliveryHealth::Ready).unwrap().records.last(),
        Some(Collected::Tablet {
            route: PenRoute::Suppressed,
            ..
        })
    ));
    assert_eq!(c.capture_class(), CaptureClass::Idle);
}
