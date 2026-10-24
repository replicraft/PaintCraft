use super::*;
use std::{
    sync::{Mutex, mpsc},
    time::{Duration, Instant},
};
pub(super) const PNG: [u8; 70] = [
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0,
    5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
fn completion(service: &mut PrintService) -> Result<PrintOutcome> {
    let started = Instant::now();
    loop {
        if let Some(result) = service.poll() {
            return result;
        }
        assert!(started.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn captured_payload_single_flight_and_completion_wake_are_owned() {
    let (ready_tx, ready_rx) = mpsc::channel();
    let (continue_tx, continue_rx) = mpsc::channel();
    let receiver = Mutex::new(continue_rx);
    let observed = Arc::new(Mutex::new(None));
    let output = observed.clone();
    let mut service = PrintService::with_runner(Arc::new(move |png, title| {
        *output.lock().unwrap() = Some((png, title));
        ready_tx.send(()).unwrap();
        receiver.lock().unwrap().recv().unwrap();
        Ok(PrintOutcome::Printed)
    }));
    let (wake_tx, wake_rx) = mpsc::channel();
    service
        .request(
            PNG.to_vec(),
            "Captured 日本語 document".into(),
            move || wake_tx.send(()).unwrap(),
        )
        .unwrap();
    ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(service.pending());
    assert!(
        service
            .request(PNG.to_vec(), "Cannot replace".into(), || {})
            .is_err()
    );
    assert!(service.poll().is_none());
    assert_eq!(
        *observed.lock().unwrap(),
        Some((PNG.to_vec(), "Captured 日本語 document".into()))
    );
    continue_tx.send(()).unwrap();
    wake_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(completion(&mut service), Ok(PrintOutcome::Printed));
    assert!(!service.pending());
    assert!(service.poll().is_none());
}
#[test]
fn cancellation_failure_and_panic_always_finish_without_false_success() {
    for expected in [Ok(PrintOutcome::Cancelled), Err("Spool failure".to_owned())] {
        let result = expected.clone();
        let mut service = PrintService::with_runner(Arc::new(move |_, _| result.clone()));
        service.request(PNG.to_vec(), "test".into(), || {}).unwrap();
        assert_eq!(completion(&mut service), expected);
        assert!(!service.pending());
    }
    let mut service = PrintService::with_runner(Arc::new(|_, _| panic!("test worker failure")));
    service.request(PNG.to_vec(), "test".into(), || {}).unwrap();
    assert!(
        completion(&mut service)
            .unwrap_err()
            .contains("worker failed")
    );
    assert!(!service.pending());
}
#[test]
fn malformed_oversized_png_and_invalid_title_do_not_start_a_worker() {
    let mut service =
        PrintService::with_runner(Arc::new(|_, _| panic!("invalid input must not launch")));
    for bytes in [vec![], vec![0; 33], PNG[..20].to_vec()] {
        assert!(service.request(bytes, "test".into(), || {}).is_err());
        assert!(!service.pending());
    }
    let mut encoded = PNG.to_vec();
    encoded.resize(ENCODED_BYTES_MAX + 1, 0);
    assert!(service.request(encoded, "test".into(), || {}).is_err());
    assert!(!service.pending());
    let mut oversized = PNG.to_vec();
    oversized[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(service.request(oversized, "test".into(), || {}).is_err());
    let mut zero = PNG.to_vec();
    zero[20..24].copy_from_slice(&0u32.to_be_bytes());
    assert!(service.request(zero, "test".into(), || {}).is_err());
    for title in ["\0".to_owned(), "x".repeat(4097)] {
        assert!(service.request(PNG.to_vec(), title, || {}).is_err());
    }
    assert!(!service.pending());
    assert!(service.poll().is_none());
}
