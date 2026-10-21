use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
fn joined<T>(thread: std::thread::JoinHandle<T>) -> T {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !thread.is_finished() {
        assert!(
            std::time::Instant::now() < deadline,
            "budget callback stalled"
        );
        std::thread::yield_now();
    }
    thread.join().unwrap()
}
fn watcher(budget: &SharedBudget, headroom: u64) -> (BudgetWatch, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let wake = count.clone();
    (
        budget
            .watch(headroom, move || {
                wake.fetch_add(1, Ordering::AcqRel);
            })
            .unwrap(),
        count,
    )
}
#[test]
fn pressure_edges_and_waiting_releases_coalesce_until_the_consumer_observes() {
    let budget = SharedBudget::new(100);
    let (watch, count) = watcher(&budget, 20);
    let first = budget.reserve(81).unwrap();
    let second = budget.reserve(1).unwrap();
    drop(second);
    assert_eq!(count.load(Ordering::Acquire), 1);
    assert_eq!(watch.observe().reserved, 81);
    drop(first);
    assert_eq!(count.load(Ordering::Acquire), 2);
    assert_eq!(watch.observe().reserved, 0);
    let small = budget.reserve(1).unwrap();
    drop(small);
    assert_eq!(count.load(Ordering::Acquire), 2);
    watch.wait_for_release(true);
    let small = budget.reserve(1).unwrap();
    drop(small);
    assert_eq!(count.load(Ordering::Acquire), 3);
    assert_eq!(watch.observe().reserved, 0);
    watch.wait_for_release(false);
    let small = budget.reserve(1).unwrap();
    drop(small);
    assert_eq!(count.load(Ordering::Acquire), 3);
}
#[test]
fn initial_pressure_and_feasible_denial_wake_but_impossible_requests_do_not() {
    let budget = SharedBudget::new(32);
    let held = budget.reserve(32).unwrap();
    let (watch, count) = watcher(&budget, 4);
    assert_eq!(count.load(Ordering::Acquire), 1);
    assert!(budget.reserve(1).is_err());
    assert_eq!(count.load(Ordering::Acquire), 1);
    watch.observe();
    assert!(budget.reserve(1).is_err());
    assert_eq!(count.load(Ordering::Acquire), 2);
    watch.observe();
    assert!(budget.reserve(33).is_err());
    assert_eq!(count.load(Ordering::Acquire), 2);
    drop(held);
    assert_eq!(count.load(Ordering::Acquire), 3);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn callbacks_can_reenter_usage_and_panics_do_not_poison_counters_or_other_watchers() {
    joined(std::thread::spawn(|| {
        let budget = SharedBudget::new(32);
        let seen = Arc::new(AtomicUsize::new(0));
        let observed = seen.clone();
        let broken = budget.watch(16, || panic!("injected wake panic")).unwrap();
        let reentrant = budget.clone();
        let good = budget
            .watch(16, move || {
                observed.store(reentrant.usage().reserved as usize, Ordering::Release);
            })
            .unwrap();
        let held = budget.reserve(24).unwrap();
        assert_eq!(seen.load(Ordering::Acquire), 24);
        broken.observe();
        good.observe();
        drop(held);
        assert_eq!(seen.load(Ordering::Acquire), 0);
        assert_eq!(
            budget.usage(),
            BudgetUsage {
                limit: 32,
                reserved: 0,
                peak: 24
            }
        );
    }));
}
#[test]
fn dropped_registrations_do_not_leak_callbacks_and_fanout_is_bounded() {
    let budget = SharedBudget::new(100);
    assert_eq!(
        budget.watch(0, || {}).unwrap_err().code,
        ErrorCode::InputSchema
    );
    let mut watches = Vec::new();
    for _ in 0..64 {
        watches.push(budget.watch(1, || {}).unwrap());
    }
    assert_eq!(
        budget.watch(1, || {}).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    drop(watches);
    let (watch, count) = watcher(&budget, 1);
    let held = budget.reserve(100).unwrap();
    assert_eq!(count.load(Ordering::Acquire), 1);
    drop(watch);
    drop(held);
    let held = budget.reserve(100).unwrap();
    drop(held);
    assert_eq!(count.load(Ordering::Acquire), 1);
}
#[test]
fn concurrent_releases_produce_one_wake_per_observation() {
    use std::sync::Barrier;
    let budget = SharedBudget::new(100);
    let held = budget.reserve(90).unwrap();
    let (watch, count) = watcher(&budget, 20);
    watch.observe();
    let start = Barrier::new(9);
    let release = Barrier::new(9);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let budget = budget.clone();
            let (start, release) = (&start, &release);
            scope.spawn(move || {
                let lease = budget.reserve(1).unwrap();
                start.wait();
                release.wait();
                drop(lease);
            });
        }
        start.wait();
        release.wait();
    });
    assert_eq!(count.load(Ordering::Acquire), 2);
    assert_eq!(watch.observe().reserved, 90);
    drop(held);
    assert_eq!(count.load(Ordering::Acquire), 3);
}
#[test]
fn unregistering_during_a_wake_releases_callback_captures_after_unlocked_dispatch() {
    use std::{
        sync::{Mutex, mpsc},
        time::Duration,
    };
    struct Probe(SharedBudget, Arc<AtomicUsize>);
    impl Drop for Probe {
        fn drop(&mut self) {
            let _ = self.0.usage();
            self.1.fetch_add(1, Ordering::AcqRel);
        }
    }
    let budget = SharedBudget::new(32);
    let dropped = Arc::new(AtomicUsize::new(0));
    let probe = Probe(budget.clone(), dropped.clone());
    let (started, wait_started) = mpsc::channel();
    let (release, wait_release) = mpsc::channel();
    let wait_release = Mutex::new(wait_release);
    let watch = budget
        .watch(16, move || {
            std::hint::black_box(&probe);
            started.send(()).unwrap();
            wait_release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        })
        .unwrap();
    let producer = std::thread::spawn(move || {
        let held = budget.reserve(24).unwrap();
        drop(held);
    });
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(watch);
    release.send(()).unwrap();
    joined(producer);
    assert_eq!(dropped.load(Ordering::Acquire), 1);
}

#[test]
fn denial_context_is_live_does_not_pin_capacity_and_preserves_legacy_error_equality() {
    let budget = SharedBudget::new(16);
    let held = budget.reserve(16).unwrap();
    let error = budget.reserve(8).unwrap_err().at(3);
    let legacy = Error::new(ErrorCode::ResourceLimit, "shared byte budget is full").at(3);
    assert_eq!(error, legacy);
    let mut redirected = error.clone();
    redirected.code = ErrorCode::CodecFailed;
    assert!(redirected.allocation_denial().is_none());
    assert_eq!(error.to_string(), legacy.to_string());
    let denial = error.allocation_denial().unwrap();
    assert_eq!(denial.class(), AllocationClass::SharedBudget);
    assert_eq!(denial.required_bytes(), 8);
    assert!(!denial.can_retry());
    drop(held);
    assert_eq!(budget.usage().reserved, 0);
    assert!(denial.can_retry());
    let impossible = budget.reserve(17).unwrap_err();
    assert!(!impossible.allocation_denial().unwrap().is_possible());
    assert!(!impossible.allocation_denial().unwrap().can_retry());
}
