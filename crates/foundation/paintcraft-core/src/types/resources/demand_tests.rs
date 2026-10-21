use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn shared_demand_uses_maximum_exact_realm_requirement_and_clears_without_owning_bytes() {
    let budget = SharedBudget::new(100);
    let held = budget.reserve(30).unwrap();
    let first = budget.watch(10, || {}).unwrap();
    let second = budget.watch(10, || {}).unwrap();
    let a = budget.reserve(90).unwrap_err();
    let b = budget.reserve(80).unwrap_err();
    first.wait_for_capacity(a.allocation_denial());
    second.wait_for_capacity(b.allocation_denial());
    let pressure = second.observe_pressure();
    assert_eq!(pressure.requested_capacity, 90); // Concurrent attempts are not summed.
    assert_eq!(pressure.usage.reserved, 30); // Demand never reserves payload capacity.
    first.wait_for_capacity(None);
    assert_eq!(first.observe_pressure().requested_capacity, 80);
    let foreign = SharedBudget::new(100).reserve(101).unwrap_err();
    second.wait_for_capacity(foreign.allocation_denial());
    assert_eq!(first.observe_pressure().requested_capacity, 0);
    // A feasible diagnostic from a different realm is also rejected.
    let other = SharedBudget::new(100);
    let other_held = other.reserve(100).unwrap();
    let foreign = other.reserve(90).unwrap_err();
    second.wait_for_capacity(foreign.allocation_denial());
    assert_eq!(first.observe_pressure().requested_capacity, 0);
    let impossible = budget.reserve(101).unwrap_err();
    second.wait_for_capacity(impossible.allocation_denial());
    assert_eq!(first.observe_pressure().requested_capacity, 0);
    second.wait_for_capacity(b.allocation_denial());
    drop(second);
    assert_eq!(first.observe_pressure().requested_capacity, 0);
    drop((held, other_held));
    assert_eq!(first.observe_pressure().usage.reserved, 0);
}

#[test]
fn demand_wakes_other_consumers_and_tracks_releases_above_the_static_threshold() {
    let budget = SharedBudget::new(100);
    let held = budget.reserve(30).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let wake = count.clone();
    let reentrant = budget.clone();
    let observer = budget
        .watch(10, move || {
            assert!(reentrant.usage().reserved <= 100);
            wake.fetch_add(1, Ordering::AcqRel);
        })
        .unwrap();
    let requester = budget.watch(10, || {}).unwrap();
    let error = budget.reserve(90).unwrap_err();
    observer.observe_pressure();
    let before = count.load(Ordering::Acquire);
    requester.wait_for_capacity(error.allocation_denial());
    assert_eq!(count.load(Ordering::Acquire), before + 1);
    assert_eq!(observer.observe_pressure().requested_capacity, 90);
    requester.wait_for_capacity(error.allocation_denial());
    assert_eq!(count.load(Ordering::Acquire), before + 1); // Unchanged interest is silent.
    let late = budget.watch(10, || {}).unwrap();
    assert_eq!(late.observe_pressure().requested_capacity, 90);
    drop(held);
    assert_eq!(count.load(Ordering::Acquire), before + 2);
    assert_eq!(observer.observe_pressure().usage.reserved, 0);
    requester.wait_for_capacity(None);
    assert_eq!(count.load(Ordering::Acquire), before + 3);
    assert_eq!(observer.observe_pressure().requested_capacity, 0);
}

#[test]
fn concurrent_request_owners_unregister_without_retaining_demand_or_charges() {
    let budget = SharedBudget::new(100);
    let held = budget.reserve(100).unwrap();
    let observer = budget.watch(10, || {}).unwrap();
    let ready = std::sync::Barrier::new(9);
    let release = std::sync::Barrier::new(9);
    std::thread::scope(|scope| {
        for bytes in 1..=8 {
            let budget = budget.clone();
            let (ready, release) = (&ready, &release);
            scope.spawn(move || {
                let watch = budget.watch(10, || {}).unwrap();
                let error = budget.reserve(bytes * 10).unwrap_err();
                watch.wait_for_capacity(error.allocation_denial());
                ready.wait();
                release.wait();
                drop(watch);
            });
        }
        ready.wait();
        assert_eq!(observer.observe_pressure().requested_capacity, 80);
        release.wait();
    });
    assert_eq!(observer.observe_pressure().requested_capacity, 0);
    assert_eq!(observer.observe_pressure().usage.reserved, 100);
    drop(held);
    assert_eq!(budget.usage().reserved, 0);
}
