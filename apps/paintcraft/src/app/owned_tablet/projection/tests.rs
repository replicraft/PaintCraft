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
