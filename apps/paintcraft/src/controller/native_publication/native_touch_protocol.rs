use super::*;
use paintcraft_platform::tablet::{
    CaptureEvent, CaptureRecord, Collected, Collector, DeliveryHealth, FingerRoute, WindowPosition,
};
#[test]
fn collector_unknown_noncontact_history_stays_metadata_only_in_source() {
    let editor = Editor::new_document(8, 8).unwrap();
    let v = context(&editor);
    let budget = SharedBudget::new(8 << 20);
    let source = Source::new_native_touch(64, 64, &budget).unwrap();
    source.publish(v).unwrap();
    let mut collector = Collector::new(8, &budget).unwrap();
    let record = CaptureRecord {
        installation: 7,
        ordinal: 0,
        event: CaptureEvent::Finger {
            identity: Identity {
                device: 30,
                pointer: 1,
                kind: DeviceKind::Finger,
            },
            phase: Phase::Hover,
            time: Time {
                epoch: 7,
                micros: 100,
                clock: Clock::WindowsPerformance,
            },
            provenance: Provenance::Coalesced,
            position: Some(WindowPosition::PhysicalPixels(2., 2.)),
            confidence: FingerConfidence::Unspecified,
            in_contact: false,
            primary: false,
        },
    };
    collector.accept(record).unwrap();
    let frame = collector.take(DeliveryHealth::Ready).unwrap();
    let [
        Collected::Finger {
            record,
            route: FingerRoute::MetadataOnly,
        },
    ] = frame.records.as_slice()
    else {
        panic!("suppressed metadata")
    };
    let CaptureEvent::Finger {
        identity,
        phase,
        time,
        provenance,
        confidence,
        in_contact,
        primary,
        ..
    } = record.event
    else {
        unreachable!()
    };
    let mut metadata = NativeFingerMetadata {
        installation: record.installation,
        ordinal: record.ordinal,
        identity,
        phase,
        time,
        provenance,
        confidence,
        in_contact,
        primary,
    };
    assert!(matches!(
        source
            .submit_native_finger(metadata, NativeFingerAdmission::Suppressed, |_| panic!(
                "unknown suppressed history has no GUI conversion"
            ))
            .unwrap(),
        NativeTouchOutcome::Suppressed
    ));
    assert!(source.mouse_capture(true));
    assert!(source.mouse_capture(false));
    metadata.ordinal = 1;
    metadata.time.micros = 101;
    metadata.phase = Phase::Down;
    metadata.provenance = Provenance::Actual;
    metadata.confidence = FingerConfidence::High;
    metadata.in_contact = true;
    let NativeTouchOutcome::Admitted(batch) = source
        .submit_native_finger(metadata, NativeFingerAdmission::Candidate, |_| {
            Ok(Some((2., 2.)))
        })
        .unwrap()
    else {
        panic!("actual new contact")
    };
    assert!(!batch.captured_primary());
    assert!(source.native_touch_batch_is_current(&batch, v.owner));
}
