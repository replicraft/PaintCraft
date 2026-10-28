//! New typed contracts fail closed until their matching consumers qualify.
use super::*;
use paintcraft_input::{Modifiers, tablet::Clock};
#[test]
fn new_native_contracts_never_alias_old_boundary_or_drop_successfully() {
    let time = Time {
        epoch: 7,
        micros: 500,
        clock: Clock::WindowsPerformance,
    };
    let identity = Identity {
        device: 11,
        pointer: 13,
        kind: paintcraft_input::tablet::DeviceKind::Finger,
    };
    for event in [
        CaptureEvent::Boundary {
            kind: Boundary::ViewInvalidated,
            time: Some(time),
        },
        CaptureEvent::IdentityRetired { identity, time },
        CaptureEvent::Finger {
            identity,
            phase: Phase::Down,
            time,
            provenance: Provenance::Actual,
            position: Some(WindowPosition::PhysicalPixels(10.0, 20.0)),
        },
    ] {
        let mut c = Collector::new(8, &SharedBudget::new(1 << 20)).unwrap();
        // Even an earlier valid queued prefix cannot escape a refused contract.
        c.accept(CaptureRecord {
            installation: 7,
            ordinal: 0,
            event: CaptureEvent::Mouse(MouseRecord {
                kind: MouseKind::Motion,
                position: WindowPosition::PhysicalPixels(10.0, 20.0),
                client: true,
                modifiers: Modifiers::default(),
                time,
            }),
        })
        .unwrap();
        assert_eq!(
            c.accept(CaptureRecord {
                installation: 7,
                ordinal: 1,
                event
            }),
            Err(RecordFault::Unsupported)
        );
        let frame = c.take(DeliveryHealth::Ready).unwrap();
        assert!(frame.failed);
        assert!(frame.records.is_empty());
        assert_eq!(
            c.accept(CaptureRecord {
                installation: 7,
                ordinal: 2,
                event
            }),
            Err(RecordFault::Retired)
        );
    }
    let mut c = Collector::new(8, &SharedBudget::new(1 << 20)).unwrap();
    assert_eq!(
        c.framework_boundary(Boundary::ViewInvalidated),
        Err(RecordFault::Unsupported)
    );
}
