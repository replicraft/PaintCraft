use super::*;
use paintcraft_input::{
    Modifiers,
    tablet::{
        Buttons, Clock, DeviceKind, Identity, Invalid, Phase, Provenance, RawSample, Time,
        Transform,
    },
};
fn sample() -> RawSample {
    RawSample {
        identity: Identity {
            device: 5,
            pointer: 7,
            kind: DeviceKind::Pen,
        },
        sequence: 1,
        time: Time {
            epoch: 11,
            micros: 1500,
            clock: Clock::AppKitUptime,
        },
        phase: Phase::Down,
        logical_position: None,
        buttons: Buttons(1),
        modifiers: Modifiers::default(),
        pressure: Some(0.0),
        tilt: None,
        twist_degrees: None,
        contact: None,
        provenance: Provenance::Actual,
    }
}
#[test]
fn physical_and_surface_positions_use_captured_gui_scale_without_changing_source_data() {
    for (position, ppp) in [
        (WindowPosition::PhysicalPixels(300., 200.), 2.),
        (
            WindowPosition::SurfacePoints {
                x: 150.,
                y: 100.,
                physical_scale: 2.,
            },
            2.,
        ),
    ] {
        let original = sample();
        let logical = NativePacket {
            raw: original,
            position: Some(position),
            delivery: Delivery::ReceiverPointer,
        }
        .into_logical(ppp)
        .unwrap();
        assert_eq!(logical.logical_position, Some((150., 100.)));
        assert_eq!(logical.pressure, Some(0.0));
        assert_eq!(logical.time, original.time);
        let mapped = logical
            .at(Transform {
                owner: 29,
                epoch: 17,
                origin: (100., 50.),
                points_per_pixel: 2.,
            })
            .unwrap();
        assert_eq!(mapped.position, (25., 25.));
        assert_eq!(mapped.transform.owner, 29);
    }
}
#[test]
fn lifecycle_has_no_invented_position_and_bad_or_double_scaling_is_refused() {
    let mut raw = sample();
    raw.phase = Phase::Cancel;
    let packet = NativePacket {
        raw,
        position: None,
        delivery: Delivery::SystemPointer,
    };
    assert_eq!(
        packet.into_logical(f64::NAN).unwrap().logical_position,
        None
    );
    for position in [
        WindowPosition::PhysicalPixels(f64::NAN, 1.),
        WindowPosition::SurfacePoints {
            x: 1.,
            y: 1.,
            physical_scale: 0.,
        },
        WindowPosition::SurfacePoints {
            x: 1.,
            y: 1.,
            physical_scale: f64::INFINITY,
        },
    ] {
        assert_eq!(
            NativePacket {
                raw: sample(),
                position: Some(position),
                delivery: Delivery::SystemPointer
            }
            .into_logical(1.),
            Err(Invalid::Coordinates)
        );
    }
    let mut already = sample();
    already.logical_position = Some((1., 1.));
    assert_eq!(
        NativePacket {
            raw: already,
            position: None,
            delivery: Delivery::SystemPointer
        }
        .into_logical(1.),
        Err(Invalid::Coordinates)
    );
    for ppp in [0., -1., f64::NAN, f64::INFINITY] {
        assert_eq!(
            NativePacket {
                raw: sample(),
                position: Some(WindowPosition::PhysicalPixels(1., 2.)),
                delivery: Delivery::SystemPointer
            }
            .into_logical(ppp),
            Err(Invalid::Coordinates)
        );
    }
}
#[test]
fn advertised_axes_are_separate_from_observed_zero_and_callback_failures_are_contained() {
    let report = Report::new(Backend::AppKit);
    assert!(!report.snapshot().installed);
    assert!(report.device(Device {
        id: 5,
        kind: DeviceKind::Pen,
        advertised: Axes {
            pressure: true,
            ..Axes::default()
        },
        observed: Axes::default(),
        native_capabilities: 0x400
    }));
    assert!(!report.snapshot().devices[0].observed.pressure);
    let emitter = Emitter::new(
        Arc::new(|_| panic!("controlled sink failure")),
        Arc::new(|| panic!("controlled wake failure")),
        report.clone(),
    );
    assert!(emitter.emit(NativePacket {
        raw: sample(),
        position: None,
        delivery: Delivery::SystemPointer
    }));
    let snapshot = report.snapshot();
    assert_eq!(snapshot.callback_failures, 2);
    assert!(snapshot.devices[0].observed.pressure);
    assert_eq!(snapshot.samples, 1);
    assert!(!snapshot.installed);
    assert!(emitter.emit(NativePacket {
        raw: sample(),
        position: None,
        delivery: Delivery::SystemPointer
    }));
    assert_eq!(report.snapshot().callback_failures, 2);
}

#[test]
fn failed_partial_enqueue_cannot_escape_a_concurrent_safe_handoff() {
    use std::{sync::mpsc, time::Duration};
    let input = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink_input = input.clone();
    let (queued, wait_queued) = mpsc::channel();
    let (release, wait_release) = mpsc::channel();
    let wait_release = std::sync::Mutex::new(wait_release);
    let emitter = Emitter::new(
        Arc::new(move |packet| {
            sink_input.lock().unwrap().push(packet.raw);
            queued.send(()).unwrap();
            wait_release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            panic!("callback failed after partial enqueue");
        }),
        Arc::new(|| {}),
        Report::new(Backend::AppKit),
    );
    let gate = emitter.gate.clone();
    let emit = std::thread::spawn(move || {
        emitter.emit(NativePacket {
            raw: sample(),
            position: Some(WindowPosition::PhysicalPixels(1., 2.)),
            delivery: Delivery::ReceiverPointer,
        })
    });
    wait_queued.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(input.lock().unwrap().len(), 1);
    let (done, wait_done) = mpsc::channel();
    let poll = std::thread::spawn(move || {
        gate.with_delivery(|health| {
            let mut input = input.lock().unwrap();
            if health == DeliveryHealth::Failed {
                input.clear();
            }
            done.send((health, std::mem::take(&mut *input))).unwrap();
        })
    });
    assert!(matches!(
        wait_done.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    release.send(()).unwrap();
    let (health, output) = wait_done.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(health, DeliveryHealth::Failed);
    assert!(output.is_empty());
    assert!(emit.join().unwrap());
    poll.join().unwrap();
}
#[test]
fn wake_failure_does_not_invalidate_fully_delivered_input() {
    let report = Report::new(Backend::AppKit);
    let emitter = Emitter::new(
        Arc::new(|_| false),
        Arc::new(|| panic!("controlled wake failure")),
        report.clone(),
    );
    assert!(!emitter.emit(NativePacket {
        raw: sample(),
        position: None,
        delivery: Delivery::ReceiverPointer
    }));
    emitter
        .gate
        .with_delivery(|health| assert_eq!(health, DeliveryHealth::Ready));
    assert_eq!(report.snapshot().wake_failures, 1);
    assert_eq!(report.snapshot().sink_failures, 0);
}

#[test]
fn fatal_source_callback_reports_failure_and_wakes_without_faking_sink_cancellation() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let wakes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let sink_calls = calls.clone();
    let wake_calls = wakes.clone();
    let report = Report::new(Backend::AppKit);
    let emitter = Emitter::new(
        Arc::new(move |_| {
            sink_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            true
        }),
        Arc::new(move || {
            wake_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }),
        report.clone(),
    );
    emitter.source_failure();
    emitter
        .gate
        .with_delivery(|health| assert_eq!(health, DeliveryHealth::Failed));
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(wakes.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(report.snapshot().source_failures, 1);
    assert!(emitter.emit(NativePacket {
        raw: sample(),
        position: None,
        delivery: Delivery::ReceiverPointer
    }));
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
}
