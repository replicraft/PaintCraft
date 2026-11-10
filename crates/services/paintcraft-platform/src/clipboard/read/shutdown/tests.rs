use super::*;
use crate::clipboard::Encoding;
use std::{
    sync::atomic::AtomicUsize,
    thread,
    time::{Duration, Instant},
};
const DEADLINE: Duration = Duration::from_secs(5);
struct Backend(Arc<AtomicUsize>);
impl Reader for Backend {
    fn read(
        &mut self,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<EncodedImage, String> {
        let bytes = b"\x89PNG\r\n\x1a\ntest".to_vec();
        EncodedImage::capture(Encoding::Png, bytes.len(), budget, check, || Ok(bytes))
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn service(drops: &Arc<AtomicUsize>) -> ReadService {
    let drops = drops.clone();
    ReadService::with_factory(Box::new(move || Ok(Box::new(Backend(drops.clone()))))).unwrap()
}
fn result(job: &Job<EncodedImage>) -> Result<EncodedImage, String> {
    let end = Instant::now() + DEADLINE;
    loop {
        if let Some(r) = job.try_result() {
            return r;
        }
        assert!(Instant::now() < end);
        thread::yield_now();
    }
}
#[test]
fn acknowledged_encoded_result_cannot_bypass_wake_join_or_free_escaped_original_bytes() {
    let drops = Arc::new(AtomicUsize::new(0));
    let service = service(&drops);
    let budget = SharedBudget::new(1024);
    let (entered, entry) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let job = service
        .submit(budget.clone(), move || {
            entered.send(()).unwrap();
            released.recv_timeout(DEADLINE).unwrap();
        })
        .unwrap();
    entry.recv_timeout(DEADLINE).unwrap();
    let image = result(&job).unwrap();
    drop(job);
    assert_eq!(budget.usage().reserved, 12);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let state = service.state.clone();
    let (done, finished) = mpsc::channel();
    let join = thread::spawn(move || done.send(service.shutdown()).unwrap());
    let limit = Instant::now() + DEADLINE;
    while !state.closed.load(Ordering::Acquire) {
        assert!(Instant::now() < limit);
        thread::yield_now();
    }
    assert!(matches!(
        finished.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    release.send(()).unwrap();
    finished.recv_timeout(DEADLINE).unwrap().unwrap();
    join.join().unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(image.bytes(), b"\x89PNG\r\n\x1a\ntest");
    assert_eq!(budget.usage().reserved, 12);
    drop(image);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn encoded_reader_wake_panic_refuses_success_after_actual_backend_drop() {
    let drops = Arc::new(AtomicUsize::new(0));
    let service = service(&drops);
    let (entered, entry) = mpsc::channel();
    let job = service
        .submit(SharedBudget::new(1024), move || {
            entered.send(()).unwrap();
            panic!("test-only read wake panic");
        })
        .unwrap();
    entry.recv_timeout(DEADLINE).unwrap();
    drop(result(&job).unwrap());
    drop(job);
    assert!(service.shutdown().is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
