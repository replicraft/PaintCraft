use super::*;
use crate::clipboard::Encoding;
use std::time::{Duration, Instant};
struct Fake {
    entered: mpsc::Sender<()>,
    gate: Option<mpsc::Receiver<()>>,
    dropped: mpsc::Sender<()>,
    fail_once: bool,
    panic: bool,
}
impl Reader for Fake {
    fn read(
        &mut self,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<EncodedImage, String> {
        self.entered.send(()).unwrap();
        if let Some(gate) = self.gate.take() {
            gate.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        assert!(!self.panic, "injected read panic");
        if std::mem::take(&mut self.fail_once) {
            return Err("injected failure".into());
        }
        let bytes = b"\x89PNG\r\n\x1a\nread transport".to_vec();
        EncodedImage::capture(Encoding::Png, bytes.len(), budget, check, || Ok(bytes))
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        let _ = self.dropped.send(());
    }
}
struct Harness {
    service: ReadService,
    entered: mpsc::Receiver<()>,
    gate: mpsc::Sender<()>,
    dropped: mpsc::Receiver<()>,
    initialized: mpsc::Receiver<std::thread::ThreadId>,
}
fn harness(fail_once: bool, panic: bool) -> Harness {
    let (entered, entries) = mpsc::channel();
    let (gate, gated) = mpsc::channel();
    let (dropped, drops) = mpsc::channel();
    let (init, initialized) = mpsc::channel();
    let mut fake = Some(Fake {
        entered,
        gate: Some(gated),
        dropped,
        fail_once,
        panic,
    });
    let service = ReadService::with_factory(Box::new(move || {
        init.send(std::thread::current().id()).unwrap();
        Ok(Box::new(fake.take().unwrap()))
    }))
    .unwrap();
    Harness {
        service,
        entered: entries,
        gate,
        dropped: drops,
        initialized,
    }
}
fn result(job: &Job<EncodedImage>) -> Result<EncodedImage, String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(value) = job.try_result() {
            return value;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}
fn recv<T>(receiver: &mpsc::Receiver<T>) -> T {
    receiver.recv_timeout(Duration::from_secs(5)).unwrap()
}
#[test]
fn read_is_lazy_off_thread_single_inflight_and_retains_payload_past_service() {
    let h = harness(false, false);
    let budget = SharedBudget::new(128);
    assert!(h.initialized.try_recv().is_err());
    let (wake, woken) = mpsc::channel();
    let job = h
        .service
        .submit(budget.clone(), move || {
            let _ = wake.send(());
        })
        .unwrap();
    assert_ne!(recv(&h.initialized), std::thread::current().id());
    recv(&h.entered);
    assert!(job.try_result().is_none());
    assert!(h.service.submit(budget.clone(), || {}).is_err());
    h.gate.send(()).unwrap();
    recv(&woken);
    assert!(
        h.service.submit(budget.clone(), || {}).is_err(),
        "unconsumed completion must keep its slot"
    );
    let image = result(&job).unwrap();
    assert!(job.try_result().is_none());
    assert_eq!(image.bytes(), b"\x89PNG\r\n\x1a\nread transport");
    let clone = image.clone();
    drop(image);
    drop(h.service);
    recv(&h.dropped);
    assert!(budget.usage().reserved > 0);
    drop(clone);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn cancellation_during_opaque_read_or_after_completion_suppresses_success() {
    for after in [false, true] {
        let h = harness(false, false);
        let budget = SharedBudget::new(128);
        let (wake, woken) = mpsc::channel();
        let job = h
            .service
            .submit(budget.clone(), move || {
                let _ = wake.send(());
            })
            .unwrap();
        recv(&h.entered);
        if !after {
            job.cancel();
        }
        h.gate.send(()).unwrap();
        recv(&woken);
        if after {
            job.cancel();
        }
        assert!(result(&job).unwrap_err().contains("cancelled"));
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn dropping_service_or_job_during_read_cannot_release_slot_early_or_publish() {
    let h = harness(false, false);
    let budget = SharedBudget::new(128);
    let job = h.service.submit(budget.clone(), || {}).unwrap();
    recv(&h.entered);
    drop(job);
    assert!(h.service.submit(budget.clone(), || {}).is_err());
    h.gate.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while h.service.state.busy.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    let job = h.service.submit(budget.clone(), || {}).unwrap();
    recv(&h.entered);
    assert!(result(&job).is_ok());
    drop(job);
    drop(h.service);
    recv(&h.dropped);
    assert_eq!(budget.usage().reserved, 0);
    let h = harness(false, false);
    let job = h.service.submit(budget.clone(), || {}).unwrap();
    recv(&h.entered);
    drop(h.service);
    h.gate.send(()).unwrap();
    assert!(result(&job).unwrap_err().contains("cancelled"));
    recv(&h.dropped);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn backend_failure_is_retryable_but_panics_poison_the_service() {
    for panic in [false, true] {
        let h = harness(true, panic);
        let budget = SharedBudget::new(128);
        let job = h.service.submit(budget.clone(), || {}).unwrap();
        recv(&h.entered);
        h.gate.send(()).unwrap();
        assert!(result(&job).is_err());
        if panic {
            assert!(h.service.submit(budget.clone(), || {}).is_err());
            recv(&h.dropped);
        } else {
            let job = h
                .service
                .submit(budget.clone(), || panic!("wake callback panic"))
                .unwrap();
            recv(&h.entered);
            assert!(result(&job).is_ok());
            let job = h.service.submit(budget.clone(), || {}).unwrap();
            recv(&h.entered);
            drop(result(&job).unwrap());
        }
        drop(job);
        drop(h.service);
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn cancellation_during_lazy_initialization_never_calls_the_reader() {
    let (start, started) = mpsc::channel();
    let (gate, gated) = mpsc::channel();
    let (enter, entered) = mpsc::channel();
    let (dropped, drops) = mpsc::channel();
    let mut fake = Some(Fake {
        entered: enter,
        gate: None,
        dropped,
        fail_once: false,
        panic: true,
    });
    let service = ReadService::with_factory(Box::new(move || {
        start.send(()).unwrap();
        gated.recv_timeout(Duration::from_secs(5)).unwrap();
        Ok(Box::new(fake.take().unwrap()))
    }))
    .unwrap();
    let job = service.submit(SharedBudget::new(128), || {}).unwrap();
    recv(&started);
    job.cancel();
    gate.send(()).unwrap();
    assert!(result(&job).unwrap_err().contains("cancelled"));
    assert!(
        entered.try_recv().is_err(),
        "cancelled initialization must not access image bytes"
    );
    drop(service);
    recv(&drops);
}
#[test]
fn initialization_failure_retries_and_initialization_panic_disables_future_reads() {
    for panic in [false, true] {
        let (enter, entered) = mpsc::channel();
        let (dropped, drops) = mpsc::channel();
        let mut fake = Some(Fake {
            entered: enter,
            gate: None,
            dropped,
            fail_once: false,
            panic: false,
        });
        let mut first = true;
        let service = ReadService::with_factory(Box::new(move || {
            if std::mem::take(&mut first) {
                assert!(!panic, "injected factory panic");
                return Err("initialization unavailable".into());
            }
            Ok(Box::new(fake.take().unwrap()))
        }))
        .unwrap();
        let budget = SharedBudget::new(128);
        let job = service.submit(budget.clone(), || {}).unwrap();
        assert!(result(&job).is_err());
        if panic {
            assert!(service.submit(budget.clone(), || {}).is_err());
        } else {
            let next = service.submit(budget.clone(), || {}).unwrap();
            recv(&entered);
            drop(result(&next).unwrap());
        }
        drop(job);
        drop(service);
        recv(&drops);
        assert_eq!(budget.usage().reserved, 0);
    }
}
