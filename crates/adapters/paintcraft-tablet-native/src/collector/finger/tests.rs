use super::*;
use paintcraft_input::tablet::Clock;
fn finger(ordinal: u64, pointer: u64, phase: Phase) -> CaptureRecord {
    CaptureRecord {
        installation: 7,
        ordinal,
        event: CaptureEvent::Finger {
            identity: Identity {
                device: 11,
                pointer,
                kind: DeviceKind::Finger,
            },
            phase,
            time: Time {
                epoch: 7,
                micros: 100 + ordinal,
                clock: Clock::WindowsPerformance,
            },
            provenance: Provenance::Actual,
            position: Some(WindowPosition::PhysicalPixels(10.0, 20.0)),
            confidence: FingerConfidence::High,
            in_contact: matches!(phase, Phase::Down | Phase::Move),
            primary: pointer == 1,
        },
    }
}
fn collector() -> Collector {
    Collector::new(128, &SharedBudget::new(1 << 20)).unwrap()
}
fn route(c: &mut Collector, record: CaptureRecord) -> FingerRoute {
    c.accept(record).unwrap();
    let frame = c.take(DeliveryHealth::Ready).unwrap();
    let [Collected::Finger { route, .. }] = frame.records.as_slice() else {
        panic!("finger record")
    };
    *route
}
#[test]
fn native_facts_and_full_identity_survive_without_primary_reassignment() {
    let mut c = collector();
    for (ordinal, pointer) in [(0, 1), (1, 2)] {
        assert_eq!(
            route(&mut c, finger(ordinal, pointer, Phase::Down)),
            FingerRoute::Eligible
        );
    }
    assert_eq!(
        route(&mut c, finger(2, 1, Phase::Up)),
        FingerRoute::Eligible
    );
    let record = finger(3, 2, Phase::Move);
    c.accept(record).unwrap();
    let frame = c.take(DeliveryHealth::Ready).unwrap();
    assert_eq!(
        frame.records[0],
        Collected::Finger {
            record,
            route: FingerRoute::Eligible
        }
    );
    let CaptureEvent::Finger { primary, .. } = record.event else {
        unreachable!()
    };
    assert!(!primary);
    // Same pointer ID on another source device is a different lifetime.
    let mut other = finger(4, 2, Phase::Down);
    if let CaptureEvent::Finger { identity, .. } = &mut other.event {
        identity.device = 12;
    }
    assert_eq!(route(&mut c, other), FingerRoute::Eligible);
}
#[test]
fn unspecified_is_unknown_and_withdrawal_is_separate_from_physical_move() {
    let mut c = collector();
    let mut unknown = finger(0, 1, Phase::Down);
    if let CaptureEvent::Finger { confidence, .. } = &mut unknown.event {
        *confidence = FingerConfidence::Unspecified;
    }
    assert_eq!(
        route(&mut c, unknown),
        FingerRoute::Denied(FingerDenial::UnspecifiedConfidence)
    );
    assert_eq!(
        route(&mut c, finger(1, 1, Phase::Move)),
        FingerRoute::Denied(FingerDenial::UnspecifiedConfidence)
    );
    assert_eq!(
        route(&mut c, finger(2, 1, Phase::Up)),
        FingerRoute::Denied(FingerDenial::UnspecifiedConfidence)
    );
    assert_eq!(
        route(&mut c, finger(3, 1, Phase::Down)),
        FingerRoute::Eligible
    );
    let mut withdrawal = finger(4, 1, Phase::Move);
    if let CaptureEvent::Finger { confidence, .. } = &mut withdrawal.event {
        *confidence = FingerConfidence::Unspecified;
    }
    c.accept(withdrawal).unwrap();
    let frame = c.take(DeliveryHealth::Ready).unwrap();
    assert_eq!(
        frame.records[0],
        Collected::Finger {
            record: withdrawal,
            route: FingerRoute::PolicyCancel
        }
    );
    let CaptureEvent::Finger { phase, .. } = withdrawal.event else {
        unreachable!()
    };
    assert_eq!(phase, Phase::Move);
    assert_eq!(
        route(&mut c, finger(5, 1, Phase::Move)),
        FingerRoute::Denied(FingerDenial::UnspecifiedConfidence)
    );
    assert_eq!(
        route(&mut c, finger(6, 1, Phase::Up)),
        FingerRoute::Denied(FingerDenial::UnspecifiedConfidence)
    );
    assert_eq!(
        route(&mut c, finger(7, 1, Phase::Down)),
        FingerRoute::Eligible
    );
}
#[test]
fn historical_withdrawal_or_terminal_refuses_entire_undrained_prefix() {
    for terminal in [false, true] {
        let mut c = collector();
        c.accept(finger(0, 1, Phase::Down)).unwrap();
        let mut history = finger(1, 1, if terminal { Phase::Up } else { Phase::Move });
        if let CaptureEvent::Finger {
            provenance,
            confidence,
            ..
        } = &mut history.event
        {
            *provenance = Provenance::Coalesced;
            *confidence = FingerConfidence::Unspecified;
        }
        assert_eq!(c.accept(history), Err(RecordFault::Invalid));
        let frame = c.take(DeliveryHealth::Ready).unwrap();
        assert!(frame.failed);
        assert!(frame.records.is_empty());
    }
}
#[test]
fn contact_enter_does_not_invent_down_and_metadata_only_terminal_clears_own_hold() {
    let mut c = collector();
    let mut enter = finger(0, 1, Phase::Enter);
    if let CaptureEvent::Finger { in_contact, .. } = &mut enter.event {
        *in_contact = true;
    }
    assert_eq!(route(&mut c, enter), FingerRoute::MetadataOnly);
    assert_eq!(
        route(&mut c, finger(1, 1, Phase::Down)),
        FingerRoute::Eligible
    );
    let mut up = finger(2, 1, Phase::Up);
    if let CaptureEvent::Finger { position, .. } = &mut up.event {
        *position = None;
    }
    assert_eq!(route(&mut c, up), FingerRoute::Eligible);
    assert_eq!(
        route(&mut c, finger(3, 1, Phase::Down)),
        FingerRoute::Eligible
    );
}
#[test]
fn prediction_has_no_lifecycle_or_clock_authority_and_history_is_geometry_only() {
    let mut c = collector();
    assert_eq!(
        route(&mut c, finger(0, 1, Phase::Down)),
        FingerRoute::Eligible
    );
    let mut prediction = finger(1, 1, Phase::Cancel);
    if let CaptureEvent::Finger {
        identity,
        time,
        provenance,
        ..
    } = &mut prediction.event
    {
        identity.kind = DeviceKind::Pen;
        time.epoch = 999;
        *provenance = Provenance::Predicted;
    }
    assert_eq!(route(&mut c, prediction), FingerRoute::MetadataOnly);
    let mut history = finger(2, 1, Phase::Move);
    if let CaptureEvent::Finger { provenance, .. } = &mut history.event {
        *provenance = Provenance::Coalesced;
    }
    assert_eq!(route(&mut c, history), FingerRoute::Eligible);
    assert_eq!(
        route(&mut c, finger(3, 1, Phase::Up)),
        FingerRoute::Eligible
    );
}
#[test]
fn held_primary_change_cancels_policy_without_promoting_other_contact() {
    let mut c = collector();
    assert_eq!(
        route(&mut c, finger(0, 2, Phase::Down)),
        FingerRoute::Eligible
    );
    let mut changed = finger(1, 2, Phase::Move);
    if let CaptureEvent::Finger { primary, .. } = &mut changed.event {
        *primary = true;
    }
    assert_eq!(route(&mut c, changed), FingerRoute::PolicyCancel);
}
#[test]
fn bounded_contact_capacity_refuses_without_publishing_a_prefix() {
    let mut c = collector();
    for n in 0..32 {
        c.accept(finger(n, n, Phase::Down)).unwrap();
    }
    assert_eq!(
        c.accept(finger(32, 32, Phase::Down)),
        Err(RecordFault::Capacity)
    );
    let frame = c.take(DeliveryHealth::Ready).unwrap();
    assert!(frame.failed && frame.records.is_empty());
}
#[test]
fn only_real_range_end_reclaims_clock_and_more_than_sixty_four_lifetimes_work() {
    let mut c = collector();
    for life in 0..128 {
        let down = finger(life * 3, life, Phase::Down);
        route(&mut c, down);
        route(&mut c, finger(life * 3 + 1, life, Phase::Up));
        let CaptureEvent::Finger { identity, time, .. } = down.event else {
            unreachable!()
        };
        c.accept(CaptureRecord {
            installation: 7,
            ordinal: life * 3 + 2,
            event: CaptureEvent::IdentityRetired {
                identity,
                time: Time {
                    micros: time.micros + 2,
                    ..time
                },
            },
        })
        .unwrap();
        c.take(DeliveryHealth::Ready).unwrap();
    }
    assert!(c.times.iter().all(Option::is_none));
}
#[test]
fn held_retirement_and_clock_regression_after_up_are_refused() {
    for held in [true, false] {
        let mut c = collector();
        let down = finger(0, 1, Phase::Down);
        route(&mut c, down);
        let CaptureEvent::Finger { identity, time, .. } = down.event else {
            unreachable!()
        };
        if held {
            assert_eq!(
                c.accept(CaptureRecord {
                    installation: 7,
                    ordinal: 1,
                    event: CaptureEvent::IdentityRetired { identity, time }
                }),
                Err(RecordFault::Invalid)
            );
        } else {
            route(&mut c, finger(1, 1, Phase::Up));
            let mut stale = finger(2, 1, Phase::Down);
            if let CaptureEvent::Finger { time: t, .. } = &mut stale.event {
                *t = time;
            }
            assert_eq!(c.accept(stale), Err(RecordFault::Invalid));
        }
    }
}
#[test]
fn clock_domain_and_epoch_cannot_rebase_a_completed_contact() {
    for wrong_domain in [true, false] {
        let mut c = collector();
        route(&mut c, finger(0, 1, Phase::Down));
        route(&mut c, finger(1, 1, Phase::Up));
        let mut bad = finger(2, 1, Phase::Down);
        if let CaptureEvent::Finger { time, .. } = &mut bad.event {
            if wrong_domain {
                time.clock = Clock::WindowsTicks;
            } else {
                time.epoch = 8;
            }
        }
        assert_eq!(c.accept(bad), Err(RecordFault::Invalid));
        assert!(c.take(DeliveryHealth::Ready).unwrap().records.is_empty());
    }
}
#[test]
fn generic_leave_never_reclaims_lifetime_clocks_and_malformed_contact_cannot_begin() {
    let mut c = collector();
    for life in 0..64 {
        route(&mut c, finger(life * 3, life, Phase::Down));
        route(&mut c, finger(life * 3 + 1, life, Phase::Up));
        route(&mut c, finger(life * 3 + 2, life, Phase::Leave));
    }
    assert_eq!(
        c.accept(finger(192, 64, Phase::Down)),
        Err(RecordFault::Capacity)
    );
    let mut c = collector();
    let mut malformed = finger(0, 1, Phase::Down);
    if let CaptureEvent::Finger { in_contact, .. } = &mut malformed.event {
        *in_contact = false;
    }
    assert_eq!(c.accept(malformed), Err(RecordFault::Invalid));
    assert!(c.fingers.0.iter().all(Option::is_none));
}

#[test]
fn internal_boundary_cannot_begin_end_or_change_native_finger_authority() {
    for phase in [Phase::Down, Phase::Move, Phase::Up, Phase::Cancel] {
        let mut c = collector();
        let prefix = finger(0, 1, Phase::Down);
        c.accept(prefix).unwrap();
        let mut boundary = finger(1, if phase == Phase::Down { 2 } else { 1 }, phase);
        if let CaptureEvent::Finger { provenance, .. } = &mut boundary.event {
            *provenance = Provenance::Boundary;
        }
        assert_eq!(c.accept(boundary), Err(RecordFault::Invalid));
        let frame = c.take(DeliveryHealth::Ready).unwrap();
        assert!(frame.failed && frame.records.is_empty());
        assert!(
            !c.fingers
                .0
                .iter()
                .flatten()
                .any(|contact| contact.identity.pointer == 2)
        );
        let CaptureEvent::Finger { identity, .. } = prefix.event else {
            unreachable!()
        };
        assert!(
            c.fingers.held(identity),
            "boundary cannot release the actual tail"
        );
    }
    // The real source Cancel remains distinct, preserving physical contact
    // until a later observed metadata-only terminal.
    let mut c = collector();
    route(&mut c, finger(0, 1, Phase::Down));
    let mut cancel = finger(1, 1, Phase::Cancel);
    if let CaptureEvent::Finger {
        in_contact,
        position,
        ..
    } = &mut cancel.event
    {
        *in_contact = true;
        *position = None;
    }
    assert_eq!(route(&mut c, cancel), FingerRoute::PolicyCancel);
    let mut up = finger(2, 1, Phase::Up);
    if let CaptureEvent::Finger { position, .. } = &mut up.event {
        *position = None;
    }
    assert_eq!(
        route(&mut c, up),
        FingerRoute::Denied(FingerDenial::RetiredContext)
    );
    assert_eq!(
        route(&mut c, finger(3, 1, Phase::Down)),
        FingerRoute::Eligible
    );
}
