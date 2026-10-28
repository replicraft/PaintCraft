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
fn mouse_button(ordinal: u64, number: u8, pressed: bool) -> CaptureRecord {
    CaptureRecord {
        installation: 7,
        ordinal,
        event: CaptureEvent::Mouse(paintcraft_platform::tablet::MouseRecord {
            kind: MouseKind::Button { number, pressed },
            position: WindowPosition::PhysicalPixels(20.0, 20.0),
            client: true,
            modifiers: Modifiers::default(),
            time: Time {
                epoch: 7,
                micros: ordinal + 1,
                clock: Clock::AppKitUptime,
            },
        }),
    }
}

#[test]
fn rejected_mouse_chord_after_pen_release_cannot_capture_the_next_pen() {
    let budget = SharedBudget::new(1 << 20);
    let mut projection = Projection::new(16, &budget).unwrap();
    projection.publish(view()).unwrap();
    projection.capture(pen(0, Phase::Down)).unwrap();
    let held = projection.take(DeliveryHealth::Ready).unwrap();
    projection.capture(mouse_button(1, 0, true)).unwrap();
    projection.capture(pen(2, Phase::Up)).unwrap();
    let completed = projection.take(DeliveryHealth::Ready).unwrap();
    assert!(
        !completed
            .entries
            .iter()
            .any(|entry| matches!(entry.pointer, Pointer::Gui(_)))
    );
    // This second mouse button belongs to the same rejected physical tail.
    // The pen has lifted, so the receiver alone cannot reject that chord.
    projection.capture(mouse_button(3, 1, true)).unwrap();
    projection.capture(pen(4, Phase::Down)).unwrap();
    projection.capture(pen(5, Phase::Up)).unwrap();
    let fresh = projection.take(DeliveryHealth::Ready).unwrap();
    let batches: Vec<_> = fresh
        .entries
        .iter()
        .filter_map(|entry| match &entry.pointer {
            Pointer::Pen(batch) => Some(batch),
            _ => None,
        })
        .collect();
    assert_eq!(batches.len(), 1);
    assert_eq!(
        batches[0]
            .frame
            .samples
            .iter()
            .map(|sample| sample.sequence)
            .collect::<Vec<_>>(),
        [4, 5]
    );
    assert!(fresh.pen_is_current(batches[0], view().owner));
    projection.capture(mouse_button(6, 0, false)).unwrap();
    projection.capture(mouse_button(7, 1, false)).unwrap();
    assert!(
        projection
            .take(DeliveryHealth::Ready)
            .unwrap()
            .entries
            .is_empty()
    );
    drop((held, completed, fresh, projection));
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn admitted_mouse_chord_keeps_client_or_native_chrome_priority_until_all_buttons_lift() {
    for client in [true, false] {
        let budget = SharedBudget::new(1 << 20);
        let mut projection = Projection::new(16, &budget).unwrap();
        projection.publish(view()).unwrap();
        let mouse = |ordinal, number, pressed| {
            let mut record = mouse_button(ordinal, number, pressed);
            let CaptureEvent::Mouse(event) = &mut record.event else {
                unreachable!()
            };
            event.client = client;
            record
        };
        projection.capture(mouse(0, 0, true)).unwrap();
        projection.capture(mouse(1, 1, true)).unwrap();
        projection.capture(pen(2, Phase::Down)).unwrap();
        projection.capture(pen(3, Phase::Up)).unwrap();
        let first = projection.take(DeliveryHealth::Ready).unwrap();
        assert!(
            !first
                .entries
                .iter()
                .any(|entry| matches!(entry.pointer, Pointer::Pen(_)))
        );
        projection.capture(mouse(4, 0, false)).unwrap();
        projection.capture(pen(5, Phase::Down)).unwrap();
        projection.capture(pen(6, Phase::Up)).unwrap();
        let partial = projection.take(DeliveryHealth::Ready).unwrap();
        assert!(
            !partial
                .entries
                .iter()
                .any(|entry| matches!(entry.pointer, Pointer::Pen(_)))
        );
        projection.capture(mouse(7, 1, false)).unwrap();
        projection.capture(pen(8, Phase::Down)).unwrap();
        projection.capture(pen(9, Phase::Up)).unwrap();
        let released = projection.take(DeliveryHealth::Ready).unwrap();
        let samples: Vec<_> = released
            .entries
            .iter()
            .filter_map(|entry| match &entry.pointer {
                Pointer::Pen(batch) => Some(&batch.frame.samples),
                _ => None,
            })
            .flatten()
            .map(|sample| sample.sequence)
            .collect();
        assert_eq!(samples, [8, 9]);
        drop((first, partial, released, projection));
        assert_eq!(budget.usage().reserved, 0);
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
