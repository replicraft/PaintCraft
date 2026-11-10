use super::*;
use std::{sync::mpsc, thread, time::Duration};
const DEADLINE: Duration = Duration::from_secs(5);

fn driver(run: impl FnOnce(Arc<State>) + Send + 'static) -> (CompletionDriver, Arc<State>) {
    let state = Arc::new(State {
        identity: Arc::new(()),
        status: Mutex::new(Status::default()),
        changed: Condvar::new(),
    });
    let observed = state.clone();
    let worker = thread::spawn(move || run(observed));
    (
        CompletionDriver(Arc::new(Owner(state.clone(), Mutex::new(Some(worker))))),
        state,
    )
}
fn requested(state: &State) {
    let current = status(state);
    let (current, waited) = state
        .changed
        .wait_timeout_while(current, DEADLINE, |s| !s.closed)
        .unwrap();
    assert!(!waited.timed_out() && current.closed);
    assert!(!current.retired);
}
#[test]
fn ordinary_drop_requests_stop_without_joining_a_blocked_thread() {
    let (release, released) = mpsc::channel();
    let (done, finished) = mpsc::channel();
    let (driver, state) = driver(move |_| {
        released.recv_timeout(DEADLINE).unwrap();
        done.send(()).unwrap();
    });
    drop(driver);
    assert!(status(&state).closed);
    assert!(!status(&state).retired);
    assert!(matches!(
        finished.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    release.send(()).unwrap();
    finished.recv_timeout(DEADLINE).unwrap();
}
struct Capture(Arc<AtomicUsize>);
impl Drop for Capture {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn explicit_shutdown_waits_for_actual_thread_and_capture_retirement() {
    let drops = Arc::new(AtomicUsize::new(0));
    let capture = Capture(drops.clone());
    let (release, released) = mpsc::channel();
    let (driver, state) = driver(move |_| {
        let _capture = capture;
        released.recv_timeout(DEADLINE).unwrap();
    });
    let alias = driver.clone();
    let (done, finished) = mpsc::channel();
    let joiner = thread::spawn(move || done.send(driver.shutdown()).unwrap());
    requested(&state);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(alias.progress().closed);
    assert!(matches!(
        finished.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    release.send(()).unwrap();
    finished.recv_timeout(DEADLINE).unwrap().unwrap();
    joiner.join().unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(status(&state).retired);
    alias.shutdown().unwrap();
}
#[test]
fn simultaneous_alias_shutdowns_share_one_actual_join_ack() {
    let (release, released) = mpsc::channel();
    let (driver, state) = driver(move |_| {
        released.recv_timeout(DEADLINE).unwrap();
    });
    let first = driver.clone();
    let (done, finished) = mpsc::channel();
    let send = done.clone();
    let one = thread::spawn(move || send.send(first.shutdown()).unwrap());
    requested(&state);
    let two = thread::spawn(move || done.send(driver.shutdown()).unwrap());
    assert!(matches!(
        finished.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    release.send(()).unwrap();
    finished.recv_timeout(DEADLINE).unwrap().unwrap();
    finished.recv_timeout(DEADLINE).unwrap().unwrap();
    one.join().unwrap();
    two.join().unwrap();
    assert!(status(&state).retired);
}
#[test]
fn worker_panic_is_reported_only_after_actual_join() {
    let (driver, state) = driver(|_| panic!("test-only observer panic"));
    assert_eq!(
        driver.shutdown().unwrap_err().code,
        ErrorCode::InvariantViolation
    );
    assert!(status(&state).retired);
    assert!(status(&state).failed);
}
#[test]
fn own_thread_refusal_preserves_handle_for_external_join() {
    let (send, receiver) = mpsc::channel::<CompletionDriver>();
    let (done, finished) = mpsc::channel();
    let (driver, state) = driver(move |_| {
        let result = receiver.recv_timeout(DEADLINE).unwrap().shutdown();
        done.send(result).unwrap();
    });
    send.send(driver.clone()).unwrap();
    assert_eq!(
        finished.recv_timeout(DEADLINE).unwrap().unwrap_err().code,
        ErrorCode::InvariantViolation
    );
    assert!(!status(&state).retired);
    assert!(driver.0.1.lock().unwrap().is_some());
    assert_eq!(
        driver.shutdown().unwrap_err().code,
        ErrorCode::InvariantViolation
    );
    assert!(status(&state).retired);
}
#[test]
fn poisoned_status_still_joins_and_refuses_success() {
    let (release, released) = mpsc::channel();
    let (driver, state) = driver(move |_| {
        released.recv_timeout(DEADLINE).unwrap();
    });
    let poison = state.clone();
    assert!(
        thread::spawn(move || {
            let _guard = poison.status.lock().unwrap();
            panic!("test-only observer coordinator poison");
        })
        .join()
        .is_err()
    );
    release.send(()).unwrap();
    assert_eq!(
        driver.shutdown().unwrap_err().code,
        ErrorCode::InvariantViolation
    );
    assert!(status(&state).retired);
}
