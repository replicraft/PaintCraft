use super::*;
use std::sync::atomic::AtomicUsize;
use std::time::{Duration, Instant};
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}
fn executor(budget: SharedBudget, transient_bytes: u64) -> Executor<u8> {
    Executor::with_budget(
        Limits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes,
        },
        budget,
    )
    .unwrap()
}
#[test]
fn shared_capacity_race_returns_both_closures_for_exactly_once_resubmission() {
    let budget = SharedBudget::new(32);
    let mut executor = executor(budget.clone(), 32);
    assert!(executor.can_submit(32));
    let blocker = budget.reserve(32).unwrap();
    let executions = Arc::new(AtomicUsize::new(0));
    let wakes = Arc::new(AtomicUsize::new(0));
    let runs = executions.clone();
    let wake = wakes.clone();
    let rejected = executor
        .submit_retained(
            32,
            move |_| {
                runs.fetch_add(1, Ordering::AcqRel);
                Ok(17)
            },
            move || {
                wake.fetch_add(1, Ordering::AcqRel);
            },
        )
        .unwrap_err();
    assert_eq!(rejected.error.code, ErrorCode::ResourceLimit);
    assert_eq!(executions.load(Ordering::Acquire), 0);
    assert_eq!(wakes.load(Ordering::Acquire), 0);
    assert_eq!(executor.pending(), 0);
    assert_eq!(executor.reserved_bytes(), 0);
    drop(blocker);
    let handle = executor
        .submit_retained(32, rejected.task, rejected.wake)
        .unwrap();
    until(|| wakes.load(Ordering::Acquire) == 1);
    assert_eq!(budget.usage().reserved, 32); // Unread result still owns its charge.
    assert_eq!(executor.poll_for(handle).unwrap().unwrap(), 17);
    assert_eq!(executions.load(Ordering::Acquire), 1);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn queue_rejection_does_not_drop_task_resources_until_the_caller_discards_them() {
    struct Probe(Arc<AtomicUsize>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::AcqRel);
        }
    }
    let budget = SharedBudget::new(32);
    let mut executor = executor(budget.clone(), 32);
    let (done, wait) = mpsc::channel();
    let held = executor
        .submit(32, |_| Ok(1), move || done.send(()).unwrap())
        .unwrap();
    wait.recv_timeout(Duration::from_secs(5)).unwrap();
    let dropped = Arc::new(AtomicUsize::new(0));
    let task_probe = Probe(dropped.clone());
    let wake_probe = Probe(dropped.clone());
    let rejected = executor
        .submit_retained(
            1,
            move |_| {
                std::hint::black_box(&task_probe);
                Ok(2)
            },
            move || {
                std::hint::black_box(&wake_probe);
            },
        )
        .unwrap_err();
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    assert_eq!(executor.pending(), 1);
    assert_eq!(executor.reserved_bytes(), 32);
    drop(rejected);
    assert_eq!(dropped.load(Ordering::Acquire), 2);
    assert_eq!(executor.poll_for(held).unwrap().unwrap(), 1);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn overflowing_and_impossible_requests_return_the_original_task_without_partial_admission() {
    let budget = SharedBudget::new(u64::MAX);
    let mut executor = executor(budget.clone(), u64::MAX);
    let (done, wait) = mpsc::channel();
    let held = executor
        .submit(u64::MAX, |_| Ok(1), move || done.send(()).unwrap())
        .unwrap();
    wait.recv_timeout(Duration::from_secs(5)).unwrap();
    let rejected = executor.submit_retained(1, |_| Ok(7), || {}).unwrap_err();
    assert!(rejected.error.message.contains("overflows"));
    assert_eq!(executor.pending(), 1);
    assert_eq!(executor.poll_for(held).unwrap().unwrap(), 1);
    let handle = executor
        .submit_retained(1, rejected.task, rejected.wake)
        .unwrap();
    until(|| executor.poll_for(handle).is_some());
    assert_eq!(budget.usage().reserved, 0);
    let mut small = super::Executor::<u8>::new(Limits {
        workers: 1,
        pending_jobs: 1,
        transient_bytes: 1,
    })
    .unwrap();
    let rejected = small.submit_retained(2, |_| Ok(9), || {}).unwrap_err();
    assert_eq!(small.pending(), 0);
    assert_eq!(small.reserved_bytes(), 0);
    let handle = small
        .submit_retained(1, rejected.task, rejected.wake)
        .unwrap();
    until(|| small.poll_for(handle).is_some());
}
