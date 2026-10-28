use super::*;
use paintcraft_input::{
    Modifiers,
    tablet::{Buttons, Clock, DeviceKind, Identity, Phase, Provenance, RawSample, Time, Transform},
};
fn view() -> View {
    View {
        owner: Owner {
            session: 1,
            state: 2,
            publication: 3,
            interface: 1,
        },
        transform: Transform {
            owner: 1,
            epoch: 1,
            origin: (0.0, 0.0),
            points_per_pixel: 1.0,
        },
        pixels_per_point: 2.0,
        canvas: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0)),
        exclusions: [None; 64],
        focused: true,
        blocked: false,
        ime: false,
    }
}
fn pen(ordinal: u64, phase: Phase) -> CaptureRecord {
    CaptureRecord {
        installation: 7,
        ordinal,
        event: CaptureEvent::Tablet(paintcraft_platform::tablet::NativePacket {
            button_edge: None,
            raw: RawSample {
                identity: Identity {
                    device: 1,
                    pointer: 2,
                    kind: DeviceKind::Pen,
                },
                sequence: ordinal,
                time: Time {
                    epoch: 7,
                    micros: ordinal + 1,
                    clock: Clock::AppKitUptime,
                },
                phase,
                logical_position: None,
                buttons: Buttons(if phase == Phase::Up { 0 } else { Buttons::TIP }),
                modifiers: Modifiers::default(),
                pressure: Some(0.5),
                tilt: None,
                twist_degrees: None,
                contact: None,
                provenance: Provenance::Actual,
            },
            position: Some(WindowPosition::PhysicalPixels(20.0, 20.0)),
            delivery: paintcraft_platform::tablet::Delivery::ReceiverPointer,
        }),
    }
}
fn invalidation(ordinal: u64) -> CaptureRecord {
    CaptureRecord {
        installation: 7,
        ordinal,
        event: CaptureEvent::Boundary {
            kind: Boundary::ViewInvalidated,
            time: None,
        },
    }
}
#[test]
fn source_backing_changed_and_restored_before_samples_cannot_revive_old_batch_and_real_lift_recovers()
 {
    let budget = SharedBudget::new(1 << 20);
    let mut projection = Projection::new(16, &budget).unwrap();
    projection.publish(view()).unwrap();
    projection.capture(pen(0, Phase::Down)).unwrap();
    let old = projection.take(DeliveryHealth::Ready).unwrap();
    projection.capture(invalidation(1)).unwrap();
    assert!(projection.view().is_none());
    let mut changed = view();
    changed.pixels_per_point = 3.0;
    projection.publish(changed).unwrap();
    projection.capture(invalidation(2)).unwrap();
    projection.publish(view()).unwrap();
    projection.capture(pen(3, Phase::Move)).unwrap();
    projection.capture(pen(4, Phase::Up)).unwrap();
    projection.capture(pen(5, Phase::Down)).unwrap();
    projection.capture(pen(6, Phase::Up)).unwrap();
    let fresh = projection.take(DeliveryHealth::Ready).unwrap();
    let old_batch = old
        .entries
        .iter()
        .find_map(|entry| {
            if let Pointer::Pen(batch) = &entry.pointer {
                Some(batch)
            } else {
                None
            }
        })
        .unwrap();
    assert!(!old.pen_is_current(old_batch, view().owner));
    let batches: Vec<_> = fresh
        .entries
        .iter()
        .filter_map(|entry| {
            if let Pointer::Pen(batch) = &entry.pointer {
                Some(batch)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(batches.len(), 1);
    assert!(fresh.pen_is_current(batches[0], view().owner));
    assert_eq!(
        batches[0]
            .frame
            .samples
            .iter()
            .map(|sample| sample.sequence)
            .collect::<Vec<_>>(),
        [5, 6]
    );
    let CaptureEvent::Tablet(packet) = pen(6, Phase::Up).event else {
        unreachable!()
    };
    projection
        .capture(CaptureRecord {
            installation: 7,
            ordinal: 7,
            event: CaptureEvent::IdentityRetired {
                identity: packet.raw.identity,
                time: Time {
                    micros: 8,
                    ..packet.raw.time
                },
            },
        })
        .unwrap();
    assert!(fresh.pen_is_current(batches[0], view().owner));
    drop(old);
    drop(fresh);
    drop(projection);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn tracking_entry_does_not_invent_motion_and_later_actual_motion_keeps_its_own_time() {
    let budget = SharedBudget::new(1 << 20);
    let mut projection = Projection::new(16, &budget).unwrap();
    projection.publish(view()).unwrap();
    for (ordinal, kind, micros) in [
        (0, MouseKind::Enter, 1000),
        (1, MouseKind::Enter, 999),
        (2, MouseKind::Motion, 900),
    ] {
        projection
            .capture(CaptureRecord {
                installation: 7,
                ordinal,
                event: CaptureEvent::Mouse(paintcraft_platform::tablet::MouseRecord {
                    kind,
                    position: WindowPosition::PhysicalPixels(20.0, 40.0),
                    client: true,
                    modifiers: Modifiers::default(),
                    time: Time {
                        epoch: 7,
                        micros,
                        clock: Clock::AppKitUptime,
                    },
                }),
            })
            .unwrap();
    }
    let frame = projection.take(DeliveryHealth::Ready).unwrap();
    assert_eq!(frame.entries.len(), 1);
    assert_eq!(frame.entries[0].ordinal, Some(2));
    assert!(matches!(frame.entries[0].pointer,
        Pointer::Gui(egui::Event::PointerMoved(at)) if at == egui::pos2(10.0, 20.0)));
}
#[test]
fn validated_finger_metadata_still_refuses_unqualified_source_and_gui_delivery() {
    let budget = SharedBudget::new(1 << 20);
    let mut projection = Projection::new(16, &budget).unwrap();
    projection.publish(view()).unwrap();
    let record = CaptureRecord {
        installation: 7,
        ordinal: 0,
        event: CaptureEvent::Finger {
            identity: Identity {
                device: 11,
                pointer: 13,
                kind: DeviceKind::Finger,
            },
            phase: Phase::Down,
            time: Time {
                epoch: 7,
                micros: 1,
                clock: Clock::WindowsPerformance,
            },
            provenance: Provenance::Actual,
            position: Some(WindowPosition::PhysicalPixels(20.0, 20.0)),
            confidence: paintcraft_input::tablet::FingerConfidence::High,
            in_contact: true,
            primary: true,
        },
    };
    assert_eq!(projection.capture(record), Err(RecordFault::Unsupported));
    assert!(projection.take(DeliveryHealth::Ready).is_err());
    assert!(projection.queue.is_empty());
}
