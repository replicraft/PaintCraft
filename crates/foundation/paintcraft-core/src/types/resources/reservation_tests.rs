use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn reserved_capacity_transfers_recycles_and_outlives_only_through_actual_leases() {
    let budget = SharedBudget::new(100);
    let reservation = budget.reserve_capacity(100).unwrap();
    let cloned = reservation.clone();
    assert!(budget.reserve(1).is_err());
    let tile = reservation.reserve(40).unwrap();
    let reader = tile.clone();
    assert_eq!(cloned.available(), 60);
    let scratch = cloned.reserve(60).unwrap();
    assert!(cloned.reserve(1).is_err());
    drop(scratch);
    assert_eq!(reservation.available(), 60);
    assert_eq!(budget.usage().reserved, 100); // Freed scratch stays admitted for reuse.
    let scratch = reservation.reserve(60).unwrap();
    drop(scratch);
    drop(tile);
    assert_eq!(reservation.available(), 60); // A cloned reader still owns forty bytes.
    drop(reservation);
    assert_eq!(budget.usage().reserved, 100);
    drop(cloned);
    assert_eq!(budget.usage().reserved, 40); // Unused capacity returned independently.
    let competing = budget.reserve(60).unwrap();
    drop(reader);
    assert_eq!(budget.usage().reserved, 60);
    drop(competing);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn reservation_never_borrows_extra_capacity_and_failures_report_known_total_demand() {
    let budget = SharedBudget::new(100);
    let reservation = budget.reserve_capacity(40).unwrap();
    let retained = reservation.reserve(35).unwrap();
    let denied = reservation.reserve(10).unwrap_err();
    assert_eq!(denied.allocation_denial().unwrap().required_bytes(), 45);
    assert_eq!(reservation.available(), 5);
    assert_eq!(budget.usage().reserved, 40);
    drop(retained);
    assert_eq!(reservation.available(), 40);
    assert_eq!(reservation.capacity(), 40);
    let denied = budget.reserve_capacity(101).unwrap_err();
    assert!(!denied.allocation_denial().unwrap().is_possible());
    assert_eq!(budget.usage().reserved, 40);
    let zero = reservation.reserve(0).unwrap();
    drop((zero, reservation));
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn capacity_returns_notify_only_when_shared_capacity_actually_returns_and_allow_reentry() {
    let budget = SharedBudget::new(100);
    let wakes = Arc::new(AtomicUsize::new(0));
    let count = wakes.clone();
    let observe = budget.clone();
    let watch = budget
        .watch(10, move || {
            assert!(observe.usage().reserved <= 100);
            count.fetch_add(1, Ordering::AcqRel);
        })
        .unwrap();
    let reservation = budget.reserve_capacity(100).unwrap();
    assert_eq!(wakes.load(Ordering::Acquire), 1);
    watch.observe();
    let scratch = reservation.reserve(40).unwrap();
    let retained = reservation.reserve(60).unwrap();
    drop(scratch);
    assert_eq!(wakes.load(Ordering::Acquire), 1);
    drop(reservation);
    assert_eq!(wakes.load(Ordering::Acquire), 2);
    assert_eq!(watch.observe().reserved, 60);
    watch.wait_for_release(true);
    drop(retained);
    assert_eq!(wakes.load(Ordering::Acquire), 3);
    assert_eq!(watch.observe().reserved, 0);
}

#[test]
fn concurrent_final_reader_release_and_reservation_drop_preserve_exact_accounting() {
    for _ in 0..32 {
        let budget = SharedBudget::new(100);
        let reservation = budget.reserve_capacity(100).unwrap();
        let leases: Vec<_> = (0..8).map(|_| reservation.reserve(10).unwrap()).collect();
        let start = std::sync::Barrier::new(9);
        std::thread::scope(|scope| {
            for lease in leases {
                let start = &start;
                scope.spawn(move || {
                    start.wait();
                    drop(lease);
                });
            }
            start.wait();
            drop(reservation);
        });
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(budget.usage().peak, 100);
        assert!(budget.reserve(100).is_ok());
    }
}

#[test]
fn capacity_preflight_does_not_transfer_bytes_or_create_activity() {
    let budget = SharedBudget::new(16);
    let reservation = budget.reserve_capacity(16).unwrap();
    let activity = budget.activity();
    reservation.ensure_capacity(16).unwrap();
    assert_eq!(reservation.available(), 16);
    let held = reservation.reserve(4).unwrap();
    let error = reservation.ensure_capacity(13).unwrap_err();
    assert_eq!(error.allocation_denial().unwrap().required_bytes(), 17);
    assert_eq!(reservation.available(), 12);
    assert_eq!(budget.activity(), activity);
    drop(held);
    assert_eq!(budget.activity(), activity); // Recycling is not globally free capacity.
    drop(reservation);
    assert!(budget.activity() > activity);
}
#[test]
fn allocation_denial_activity_detects_equal_counter_replacement_but_not_denial_feedback() {
    let budget = SharedBudget::new(16);
    let held = budget.reserve(16).unwrap();
    let error = budget.reserve(8).unwrap_err();
    let denial = error.allocation_denial().unwrap();
    let usage = budget.usage();
    let activity = budget.activity();
    let watch = budget.watch(1, || {}).unwrap();
    watch.wait_for_capacity(Some(denial));
    assert!(budget.reserve(8).is_err());
    drop(budget.reserve(0).unwrap());
    assert_eq!(budget.activity(), activity);
    assert!(!denial.activity_changed());
    drop(held);
    let replaced = budget.reserve(16).unwrap();
    assert_eq!(budget.usage(), usage);
    assert!(denial.activity_changed());
    budget.0.lock().unwrap().activity = u64::MAX;
    let error = budget.reserve(8).unwrap_err();
    assert!(error.allocation_denial().unwrap().activity_changed()); // Saturation stays conservative.
    drop(replaced);
}
