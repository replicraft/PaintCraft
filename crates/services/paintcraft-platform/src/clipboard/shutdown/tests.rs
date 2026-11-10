use super::*;
use paintcraft_core::types::resources::SharedBudget;
use std::{
    sync::atomic::AtomicUsize,
    thread,
    time::{Duration, Instant},
};
const DEADLINE: Duration = Duration::from_secs(5);
struct Backend(Arc<AtomicUsize>);
impl Writer for Backend {
    fn write(&mut self, _: &Image) -> Result<(), String> {
        Ok(())
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn service(drops: &Arc<AtomicUsize>) -> ClipboardService {
    let drops = drops.clone();
    ClipboardService::with_factory(Box::new(move || Ok(Box::new(Backend(drops.clone()))))).unwrap()
}
fn image(budget: &SharedBudget) -> Image {
    Image::from_rgba(1, 1, vec![1, 2, 3, 255], budget, &|| Ok(())).unwrap()
}
fn result(job: &Job) -> Result<(), String> {
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
fn acknowledged_write_does_not_skip_blocked_wake_and_backend_retirement() {
    let drops = Arc::new(AtomicUsize::new(0));
    let service = service(&drops);
    let budget = SharedBudget::new(1024);
    let (entered, entry) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let job = service
        .submit(image(&budget), move || {
            entered.send(()).unwrap();
            released.recv_timeout(DEADLINE).unwrap();
        })
        .unwrap();
    entry.recv_timeout(DEADLINE).unwrap();
    result(&job).unwrap();
    drop(job);
    assert_eq!(budget.usage().reserved, 0);
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
}
#[test]
fn wake_panic_is_reported_after_worker_and_backend_retire() {
    let drops = Arc::new(AtomicUsize::new(0));
    let service = service(&drops);
    let (entered, entry) = mpsc::channel();
    let job = service
        .submit(image(&SharedBudget::new(1024)), move || {
            entered.send(()).unwrap();
            panic!("test-only write wake panic");
        })
        .unwrap();
    entry.recv_timeout(DEADLINE).unwrap();
    result(&job).unwrap();
    drop(job);
    assert!(service.shutdown().is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
#[test]
fn ordinary_drop_still_does_not_join_a_blocked_wake() {
    let drops = Arc::new(AtomicUsize::new(0));
    let service = service(&drops);
    let (entered, entry) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let job = service
        .submit(image(&SharedBudget::new(1024)), move || {
            entered.send(()).unwrap();
            released.recv_timeout(DEADLINE).unwrap();
        })
        .unwrap();
    entry.recv_timeout(DEADLINE).unwrap();
    drop(job);
    drop(service);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    release.send(()).unwrap();
    let end = Instant::now() + DEADLINE;
    while drops.load(Ordering::SeqCst) == 0 {
        assert!(Instant::now() < end);
        thread::yield_now();
    }
}
