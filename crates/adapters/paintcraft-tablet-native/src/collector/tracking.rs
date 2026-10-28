//! Actual startup trace regression: AppKit tracking timestamps are not a
//! physical mouse clock baseline. Neither timestamps nor producer order change.
use super::*;
use paintcraft_input::{Modifiers, tablet::Clock};

fn record(ordinal: u64, kind: MouseKind, micros: u64) -> CaptureRecord {
    CaptureRecord {
        installation: 7,
        ordinal,
        event: CaptureEvent::Mouse(MouseRecord {
            kind,
            position: WindowPosition::PhysicalPixels(10.25, 11.75),
            client: true,
            modifiers: Modifiers::default(),
            time: Time {
                epoch: 7,
                micros,
                clock: Clock::AppKitUptime,
            },
        }),
    }
}

#[test]
fn native_tracking_trace_retains_original_time_and_dispatch_order_without_moving_the_clock() {
    let mut collector = Collector::new(16, &SharedBudget::new(1 << 20)).unwrap();
    let trace = [
        (MouseKind::Enter, 221036945567),
        (MouseKind::Enter, 221036979911),
        (MouseKind::Enter, 221036979901),
        (MouseKind::Motion, 221036969981),
        (MouseKind::Leave, 221036981227),
        (MouseKind::Leave, 221036981226),
        (MouseKind::Enter, 221036981219),
        (MouseKind::Leave, 221036978600),
        (MouseKind::Leave, 221036988589),
        (MouseKind::Motion, 221036988074),
    ];
    for (ordinal, (kind, micros)) in trace.into_iter().enumerate() {
        collector
            .accept(record(ordinal as u64, kind, micros))
            .unwrap();
    }
    let frame = collector.take(DeliveryHealth::Ready).unwrap();
    assert!(!frame.failed);
    assert_eq!(frame.records.len(), trace.len());
    for (ordinal, entry) in frame.records.iter().enumerate() {
        let Collected::Mouse { record: actual, .. } = entry else {
            panic!("expected native mouse record")
        };
        assert_eq!(
            *actual,
            record(ordinal as u64, trace[ordinal].0, trace[ordinal].1)
        );
    }
    assert_eq!(collector.mouse_time.unwrap().micros, trace[9].1);
}

#[test]
fn tracking_cannot_rebase_physical_motion_buttons_or_wheels() {
    for kind in [
        MouseKind::Motion,
        MouseKind::Button {
            number: 0,
            pressed: true,
        },
        MouseKind::Wheel {
            delta: (0.0, 1.0),
            precise: false,
            physical_scale: 1.0,
            phase: WheelPhase::Move,
            momentum: false,
        },
    ] {
        let mut collector = Collector::new(8, &SharedBudget::new(1 << 20)).unwrap();
        collector.accept(record(0, MouseKind::Enter, 1000)).unwrap();
        collector.accept(record(1, MouseKind::Motion, 900)).unwrap();
        collector.accept(record(2, MouseKind::Leave, 1)).unwrap();
        assert_eq!(
            collector.accept(record(3, kind, 899)),
            Err(RecordFault::Invalid)
        );
        let frame = collector.take(DeliveryHealth::Ready).unwrap();
        assert!(frame.failed);
        assert!(frame.records.is_empty());
    }
}

#[test]
fn tracking_still_requires_the_original_clock_domain_epoch_and_finite_native_position() {
    for change in 0..3 {
        let mut collector = Collector::new(8, &SharedBudget::new(1 << 20)).unwrap();
        collector.accept(record(0, MouseKind::Enter, 1000)).unwrap();
        let mut bad = record(1, MouseKind::Leave, 10);
        let CaptureEvent::Mouse(mouse) = &mut bad.event else {
            unreachable!()
        };
        match change {
            0 => mouse.time.clock = Clock::WindowsPerformance,
            1 => mouse.time.epoch += 1,
            _ => mouse.position = WindowPosition::PhysicalPixels(f64::NAN, 0.0),
        }
        assert_eq!(collector.accept(bad), Err(RecordFault::Invalid));
        assert!(
            collector
                .take(DeliveryHealth::Ready)
                .unwrap()
                .records
                .is_empty()
        );
    }
}
