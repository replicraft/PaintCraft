use super::*;
use paintcraft_core::types::resources::SharedBudget;
use std::sync::Mutex;
use std::time::{Duration, Instant};

fn image() -> Image {
    Image::from_rgba(
        2,
        1,
        vec![91, 20, 7, 0, 1, 2, 3, 128],
        &SharedBudget::new(1024),
        &|| Ok(()),
    )
    .unwrap()
}
fn result(job: &Job) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = job.try_result() {
            return result;
        }
        assert!(Instant::now() < deadline, "worker timed out");
        std::thread::yield_now();
    }
}
fn recv<T>(receiver: &mpsc::Receiver<T>) -> T {
    receiver.recv_timeout(Duration::from_secs(5)).unwrap()
}

struct Fake {
    writes: mpsc::Sender<Vec<u8>>,
    gate: Option<mpsc::Receiver<()>>,
    dropped: mpsc::Sender<()>,
    fail_once: bool,
    panic: bool,
}
impl Writer for Fake {
    fn write(&mut self, image: &Image) -> Result<(), String> {
        assert_eq!((image.width(), image.height()), (2, 1));
        self.writes.send(image.rgba().to_vec()).unwrap();
        if let Some(gate) = self.gate.take() {
            recv(&gate);
        }
        assert!(!self.panic, "fake backend panic");
        if std::mem::take(&mut self.fail_once) {
            Err("native failure".into())
        } else {
            Ok(())
        }
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        let _ = self.dropped.send(());
    }
}
struct Harness {
    service: ClipboardService,
    writes: mpsc::Receiver<Vec<u8>>,
    gate: mpsc::Sender<()>,
    dropped: mpsc::Receiver<()>,
    initialized: mpsc::Receiver<std::thread::ThreadId>,
}
fn harness(fail_once: bool, panic: bool) -> Harness {
    let (writes, reads) = mpsc::channel();
    let (gate, gated) = mpsc::channel();
    let (dropped, drops) = mpsc::channel();
    let (init, initialized) = mpsc::channel();
    let mut fake = Some(Fake {
        writes,
        gate: Some(gated),
        dropped,
        fail_once,
        panic,
    });
    let service = ClipboardService::with_factory(Box::new(move || {
        init.send(std::thread::current().id()).unwrap();
        Ok(Box::new(fake.take().unwrap()))
    }))
    .unwrap();
    Harness {
        service,
        writes: reads,
        gate,
        dropped: drops,
        initialized,
    }
}

#[test]
fn acknowledges_native_result_and_retains_backend_ownership() {
    let h = harness(false, false);
    assert!(
        h.initialized.try_recv().is_err(),
        "initialization must be lazy"
    );
    let (wake, woke) = mpsc::channel();
    let job = h
        .service
        .submit(image(), move || {
            wake.send(()).unwrap();
        })
        .unwrap();
    assert_ne!(recv(&h.initialized), std::thread::current().id());
    assert_eq!(recv(&h.writes), image().rgba());
    assert!(job.try_result().is_none());
    assert!(h.service.submit(image(), || {}).is_err());
    h.gate.send(()).unwrap();
    recv(&woke);
    assert!(
        h.service.submit(image(), || {}).is_err(),
        "unconsumed acknowledgement retains slot"
    );
    result(&job).unwrap();
    assert!(job.try_result().is_none());
    drop(job);
    assert!(h.dropped.try_recv().is_err());
    let next = h.service.submit(image(), || {}).unwrap();
    recv(&h.writes);
    result(&next).unwrap();
    assert!(
        h.initialized.try_recv().is_err(),
        "same backend must be reused"
    );
    drop(h.service);
    recv(&h.dropped);
}

#[test]
fn errors_are_acknowledged_and_retry_uses_existing_writer() {
    let h = harness(true, false);
    let job = h.service.submit(image(), || {}).unwrap();
    recv(&h.writes);
    h.gate.send(()).unwrap();
    assert_eq!(result(&job), Err("native failure".into()));
    let retry = h.service.submit(image(), || {}).unwrap();
    recv(&h.writes);
    result(&retry).unwrap();
}

#[test]
fn cancellation_during_os_call_retains_slot_until_completion() {
    let h = harness(false, false);
    let job = h.service.submit(image(), || {}).unwrap();
    recv(&h.writes);
    job.cancel();
    assert!(job.try_result().is_none());
    assert!(h.service.submit(image(), || {}).is_err());
    h.gate.send(()).unwrap();
    assert!(result(&job).unwrap_err().contains("cancelled"));
    let next = h.service.submit(image(), || {}).unwrap();
    recv(&h.writes);
    result(&next).unwrap();
}

#[test]
fn cancellation_after_native_success_suppresses_acknowledgement() {
    let h = harness(false, false);
    let (wake, woke) = mpsc::channel();
    let job = h
        .service
        .submit(image(), move || {
            wake.send(()).unwrap();
        })
        .unwrap();
    recv(&h.writes);
    h.gate.send(()).unwrap();
    recv(&woke);
    job.cancel();
    assert!(result(&job).unwrap_err().contains("cancelled"));
}

#[test]
fn dropping_job_does_not_release_busy_while_os_call_is_running() {
    let h = harness(false, false);
    let (wake, woke) = mpsc::channel();
    let job = h
        .service
        .submit(image(), move || {
            wake.send(()).unwrap();
        })
        .unwrap();
    recv(&h.writes);
    drop(job);
    assert!(h.service.submit(image(), || {}).is_err());
    h.gate.send(()).unwrap();
    recv(&woke);
    assert!(h.dropped.try_recv().is_err());
    let next = h.service.submit(image(), || {}).unwrap();
    recv(&h.writes);
    result(&next).unwrap();
}

#[test]
fn shutdown_is_nonblocking_and_prevents_successful_acknowledgement() {
    let h = harness(false, false);
    let job = h.service.submit(image(), || {}).unwrap();
    recv(&h.writes);
    drop(h.service);
    h.gate.send(()).unwrap();
    assert!(result(&job).unwrap_err().contains("cancelled"));
    recv(&h.dropped);
}

#[test]
fn backend_panic_poison_is_permanent() {
    let h = harness(false, true);
    let job = h.service.submit(image(), || {}).unwrap();
    recv(&h.writes);
    h.gate.send(()).unwrap();
    assert!(result(&job).unwrap_err().contains("panicked"));
    assert!(h.service.submit(image(), || {}).is_err());
    recv(&h.dropped);
}

#[test]
fn factory_failure_retries_but_factory_panic_poison_is_permanent() {
    let calls = Arc::new(Mutex::new(0));
    let count = calls.clone();
    let service = ClipboardService::with_factory(Box::new(move || {
        *count.lock().unwrap() += 1;
        Err("no display".into())
    }))
    .unwrap();
    for _ in 0..2 {
        let job = service.submit(image(), || {}).unwrap();
        assert_eq!(result(&job), Err("no display".into()));
    }
    assert_eq!(*calls.lock().unwrap(), 2);
    let service = ClipboardService::with_factory(Box::new(|| panic!("factory"))).unwrap();
    let job = service.submit(image(), || {}).unwrap();
    assert!(result(&job).unwrap_err().contains("panicked"));
    assert!(service.submit(image(), || {}).is_err());
}

#[test]
fn wake_panic_does_not_poison_or_lose_backend() {
    let h = harness(false, false);
    let job = h.service.submit(image(), || panic!("wake")).unwrap();
    recv(&h.writes);
    h.gate.send(()).unwrap();
    result(&job).unwrap();
    let next = h.service.submit(image(), || {}).unwrap();
    recv(&h.writes);
    result(&next).unwrap();
}

#[test]
fn cancelled_during_initialization_never_calls_writer() {
    let (started, initialization) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let (writes, written) = mpsc::channel();
    let (dropped, drops) = mpsc::channel();
    let mut blocked = Some(blocked);
    let service = ClipboardService::with_factory(Box::new(move || {
        started.send(()).unwrap();
        recv(&blocked.take().unwrap());
        Ok(Box::new(Fake {
            writes: writes.clone(),
            gate: None,
            dropped: dropped.clone(),
            fail_once: false,
            panic: false,
        }))
    }))
    .unwrap();
    let job = service.submit(image(), || {}).unwrap();
    recv(&initialization);
    job.cancel();
    release.send(()).unwrap();
    assert!(result(&job).unwrap_err().contains("cancelled"));
    assert!(written.try_recv().is_err());
    drop(service);
    recv(&drops);
}

#[test]
fn dropped_inflight_job_keeps_capture_reserved_until_worker_finishes() {
    let h = harness(false, false);
    let budget = SharedBudget::new(8);
    let pixels = Image::from_rgba(2, 1, vec![0; 8], &budget, &|| Ok(())).unwrap();
    let (wake, woke) = mpsc::channel();
    let job = h
        .service
        .submit(pixels, move || {
            wake.send(()).unwrap();
        })
        .unwrap();
    recv(&h.writes);
    drop(job);
    assert!(budget.reserve(1).is_err());
    h.gate.send(()).unwrap();
    recv(&woke);
    assert!(budget.reserve(8).is_ok());
}

#[test]
fn retirement_and_completion_race_releases_slot_once() {
    for _ in 0..100 {
        let state = Arc::new(State::default());
        state.busy.store(true, Ordering::Release);
        let lease = Arc::new(Lease {
            state: state.clone(),
            cancelled: AtomicBool::new(false),
            done: AtomicBool::new(false),
            retired: AtomicBool::new(false),
            released: AtomicBool::new(false),
        });
        let other = lease.clone();
        let thread = std::thread::spawn(move || other.finish());
        lease.retire();
        thread.join().unwrap();
        assert!(!state.busy.load(Ordering::Acquire));
        state.busy.store(true, Ordering::Release);
        lease.retire();
        lease.finish();
        assert!(
            state.busy.load(Ordering::Acquire),
            "old job must not clear newer pending slot"
        );
    }
}
