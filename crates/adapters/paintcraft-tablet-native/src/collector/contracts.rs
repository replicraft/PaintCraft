//! Collector metadata validation is distinct from Source/GUI authorization.
use super::*;
use paintcraft_input::{Modifiers, tablet::Clock};
#[test]
fn finger_metadata_remains_distinct_from_boundary_and_gui_admission() {
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
    let event = CaptureEvent::Finger {
        confidence: paintcraft_input::tablet::FingerConfidence::High,
        in_contact: true,
        primary: true,
        identity,
        phase: Phase::Down,
        time,
        provenance: Provenance::Actual,
        position: Some(WindowPosition::PhysicalPixels(10.0, 20.0)),
    };
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
    c.accept(CaptureRecord {
        installation: 7,
        ordinal: 1,
        event,
    })
    .unwrap();
    let frame = c.take(DeliveryHealth::Ready).unwrap();
    assert!(!frame.failed);
    assert!(matches!(
        frame.records[1],
        Collected::Finger {
            route: FingerRoute::Eligible,
            ..
        }
    ));
    let mut c = Collector::new(8, &SharedBudget::new(1 << 20)).unwrap();
    c.framework_boundary(Boundary::ViewInvalidated).unwrap();
    let frame = c.take(DeliveryHealth::Ready).unwrap();
    assert!(matches!(
        frame.records.as_slice(),
        [Collected::FrameworkBoundary(Boundary::ViewInvalidated)]
    ));
}
