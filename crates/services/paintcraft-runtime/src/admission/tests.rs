use super::*;
use std::time::{Duration, Instant};
fn limits() -> Limits {
    Limits {
        workers: 1,
        pending_jobs: 2,
        transient_bytes: 64,
    }
}
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}
#[test]
fn heterogeneous_pools_share_one_cap_until_each_completion_is_consumed() {
    let budget = SharedBudget::new(32);
    let mut first = Executor::<u32>::with_budget(limits(), budget.clone()).unwrap();
    let mut second = Executor::<String>::with_budget(limits(), budget.clone()).unwrap();
    let (done, wait) = mpsc::channel();
    let a = first
        .submit(
            24,
            |_| Ok(7),
            move || {
                done.send(()).unwrap();
            },
        )
        .unwrap();
    wait.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(!second.can_submit(9));
    assert_eq!(
        second
            .submit(9, |_| Ok(String::new()), || {})
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(second.reserved_bytes(), 0);
    let b = second
        .submit(8, |_| Ok("result".to_owned()), || {})
        .unwrap();
    assert_eq!(budget.usage().reserved, 32);
    until(|| second.poll_for(b).is_some());
    assert_eq!(budget.usage().reserved, 24);
    assert_eq!(first.poll_for(a).unwrap().unwrap(), 7);
    assert_eq!(
        budget.usage(),
        BudgetUsage {
            limit: 32,
            reserved: 0,
            peak: 32
        }
    );
}
#[test]
fn dropping_a_running_executor_does_not_release_its_workers_charge() {
    let budget = SharedBudget::new(32);
    let mut executor = Executor::<Vec<u8>>::with_budget(limits(), budget.clone()).unwrap();
    let (started, wait_started) = mpsc::channel();
    let (release, wait_release) = mpsc::channel();
    executor
        .submit(
            32,
            move |_| {
                started.send(()).unwrap();
                wait_release.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok(vec![1; 32])
            },
            || {},
        )
        .unwrap();
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(executor);
    let mut other = Executor::<u32>::with_budget(limits(), budget.clone()).unwrap();
    assert_eq!(budget.usage().reserved, 32);
    assert!(!other.can_submit(1));
    release.send(()).unwrap();
    until(|| budget.usage().reserved == 0);
    let handle = other.submit(32, |_| Ok(1), || {}).unwrap();
    until(|| other.poll_for(handle).is_some());
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn abandonment_and_panics_release_shared_admission_only_after_completion() {
    let budget = SharedBudget::new(32);
    let mut executor = Executor::<u32>::with_budget(limits(), budget.clone()).unwrap();
    let (started, wait_started) = mpsc::channel();
    let (release, wait_release) = mpsc::channel();
    let handle = executor
        .submit(
            32,
            move |_| {
                started.send(()).unwrap();
                wait_release.recv_timeout(Duration::from_secs(5)).unwrap();
                panic!("injected shared-budget failure");
            },
            || {},
        )
        .unwrap();
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(executor.abandon(handle));
    assert_eq!(budget.usage().reserved, 32);
    release.send(()).unwrap();
    until(|| {
        executor.reap_abandoned();
        budget.usage().reserved == 0
    });
    assert_eq!(executor.pending(), 0);
}

#[test]
fn overflow_and_local_rejection_never_change_another_pools_charge() {
    let budget = SharedBudget::new(u64::MAX);
    let pool_limits = Limits {
        workers: 1,
        pending_jobs: 2,
        transient_bytes: u64::MAX,
    };
    let mut first = Executor::<u32>::with_budget(pool_limits, budget.clone()).unwrap();
    let mut second = Executor::<u32>::with_budget(pool_limits, budget.clone()).unwrap();
    let handle = first.submit(u64::MAX, |_| Ok(1), || {}).unwrap();
    assert!(!first.can_submit(1));
    assert!(first.submit(1, |_| Ok(2), || {}).is_err());
    assert!(!second.can_submit(1));
    assert!(second.submit(1, |_| Ok(2), || {}).is_err());
    assert_eq!(budget.usage().reserved, u64::MAX);
    until(|| first.poll_for(handle).is_some());
    assert_eq!(budget.usage().reserved, 0);
    let mut small = Executor::<u32>::with_budget(limits(), budget.clone()).unwrap();
    assert!(!small.request_fits(65));
    assert!(small.submit(65, |_| Ok(2), || {}).is_err());
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn discarded_ready_output_is_destroyed_before_its_charge_is_released() {
    struct Probe(SharedBudget, Arc<AtomicBool>);
    impl Drop for Probe {
        fn drop(&mut self) {
            assert_eq!(self.0.usage().reserved, 32);
            self.1.store(true, Ordering::Release);
        }
    }
    let budget = SharedBudget::new(32);
    let dropped = Arc::new(AtomicBool::new(false));
    let probe = Probe(budget.clone(), dropped.clone());
    let mut executor = Executor::<Probe>::with_budget(limits(), budget.clone()).unwrap();
    let (done, wait_done) = mpsc::channel();
    executor
        .submit(
            32,
            |_| Ok(probe),
            move || {
                done.send(()).unwrap();
            },
        )
        .unwrap();
    wait_done.recv_timeout(Duration::from_secs(5)).unwrap();
    executor.reap_abandoned(); // Buffer ready output without consuming it.
    drop(executor);
    assert!(dropped.load(Ordering::Acquire));
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn racing_pools_cannot_each_admit_the_same_remaining_bytes() {
    use std::sync::{Barrier, atomic::AtomicUsize};
    let budget = SharedBudget::new(32);
    let start = Barrier::new(9);
    let admitted = Barrier::new(9);
    let finish = Barrier::new(9);
    let successes = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let budget = budget.clone();
            let (start, admitted, finish, successes) = (&start, &admitted, &finish, &successes);
            scope.spawn(move || {
                let mut executor = Executor::<u8>::with_budget(limits(), budget).unwrap();
                start.wait();
                let handle = executor.submit(24, |_| Ok(1), || {}).ok();
                if handle.is_some() {
                    successes.fetch_add(1, Ordering::AcqRel);
                }
                admitted.wait();
                finish.wait();
                if let Some(handle) = handle {
                    until(|| executor.poll_for(handle).is_some());
                }
            });
        }
        start.wait();
        admitted.wait();
        let count = successes.load(Ordering::Acquire);
        let reserved = budget.usage().reserved;
        finish.wait();
        assert_eq!(count, 1);
        assert_eq!(reserved, 24);
    });
    assert_eq!(
        budget.usage(),
        BudgetUsage {
            limit: 32,
            reserved: 0,
            peak: 24
        }
    );
}
