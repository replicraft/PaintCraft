use super::*;

#[test]
fn reserved_accounts_transfer_recycle_and_release_without_pinning_unused_capacity() {
    let budget = SharedBudget::new(64);
    let reserved = budget.reserve_capacity(32).unwrap();
    let probe = reserved.clone();
    let scope = reserved.scope(AllocationClass::PaintMetadata);
    let other = scope.clone();
    let mut a = scope.account();
    let mut b = a.sibling();
    a.resize(12).unwrap();
    b.resize(8).unwrap();
    assert_eq!(probe.available(), 12);
    assert_eq!(budget.usage().reserved, 32);
    a.resize(4).unwrap();
    assert_eq!(probe.available(), 20);
    b.resize(24).unwrap();
    assert_eq!(probe.available(), 4);
    drop(probe);
    drop(scope);
    assert_eq!(budget.usage().reserved, 32);
    drop(other);
    assert_eq!(budget.usage().reserved, 28);
    drop(a);
    assert_eq!(budget.usage().reserved, 24);
    // A surviving allocation can grow after the factory has ended; this growth
    // competes for live global capacity rather than resurrecting a reservation.
    b.resize(30).unwrap();
    assert_eq!(budget.usage().reserved, 30);
    drop(b);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn reserved_scope_is_strict_and_denial_prices_live_prefix_plus_pending_growth() {
    let budget = SharedBudget::new(64);
    let scope = budget
        .reserve_capacity(16)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let mut a = scope.account();
    let mut b = scope.account();
    a.resize(8).unwrap();
    b.resize(8).unwrap();
    let error = b.resize(12).unwrap_err();
    let denial = error.allocation_denial().unwrap();
    assert_eq!(denial.class(), AllocationClass::PaintMetadata);
    assert_eq!(denial.required_bytes(), 20);
    assert_eq!(scope.footprint(), ScopeFootprint { live: 16, peak: 20 });
    assert_eq!(b.bytes(), 8);
    assert_eq!(budget.usage().reserved, 16);
    drop(a);
    b.resize(12).unwrap();
    assert_eq!(scope.footprint(), ScopeFootprint { live: 12, peak: 20 });
    drop(b);
    assert_eq!(budget.usage().reserved, 16);
    drop(scope);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn accounts_and_leases_share_one_capacity_without_double_admission() {
    let budget = SharedBudget::new(16);
    let reserved = budget.reserve_capacity(16).unwrap();
    let scope = reserved.clone().scope(AllocationClass::PaintMetadata);
    let mut account = scope.account();
    account.resize(8).unwrap();
    let lease = reserved.reserve(8).unwrap();
    assert_eq!(reserved.available(), 0);
    assert!(account.resize(9).is_err());
    drop(lease);
    account.resize(16).unwrap();
    assert_eq!(reserved.available(), 0);
    drop(account);
    assert_eq!(reserved.available(), 16);
    drop(scope);
    drop(reserved);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn reserved_account_release_callbacks_observe_storage_and_scope_without_lock_cycles() {
    use std::sync::atomic::{AtomicBool, Ordering};
    struct Owner {
        _storage: Arc<Vec<u8>>,
        _charge: BudgetAccount,
    }
    let budget = SharedBudget::new(16);
    let scope = budget
        .reserve_capacity(16)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let mut account = scope.account();
    account.resize(8).unwrap();
    let footprint = account.scope.clone().unwrap();
    let storage = Arc::new(vec![0; 8]);
    let weak = Arc::downgrade(&storage);
    let observed = Arc::new(AtomicBool::new(false));
    let recorded = observed.clone();
    let probe = budget.clone();
    let watch = budget
        .watch(1, move || {
            let live = footprint.lock().unwrap().live;
            if probe.usage().reserved == 0 {
                assert_eq!(live, 0);
                recorded.store(weak.upgrade().is_none(), Ordering::Release);
            }
        })
        .unwrap();
    watch.observe();
    drop(scope);
    watch.wait_for_release(true);
    watch.observe();
    drop(Owner {
        _storage: storage,
        _charge: account,
    });
    assert!(observed.load(Ordering::Acquire));
}

#[test]
fn concurrent_reserved_accounts_cannot_consume_the_same_headroom() {
    use std::{sync::mpsc, time::Duration};
    let budget = SharedBudget::new(16);
    let scope = budget
        .reserve_capacity(16)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let (send, receive) = mpsc::channel();
    let mut workers = Vec::new();
    let mut releases = Vec::new();
    for _ in 0..2 {
        let scope = scope.clone();
        let send = send.clone();
        let (release, wait) = mpsc::channel();
        releases.push(release);
        workers.push(std::thread::spawn(move || {
            let mut account = scope.account();
            send.send(account.resize(12).is_ok()).unwrap();
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
        }));
    }
    assert_eq!(
        (0..2)
            .filter(|_| receive.recv_timeout(Duration::from_secs(5)).unwrap())
            .count(),
        1
    );
    for release in releases {
        release.send(()).unwrap();
    }
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(scope.footprint().live, 0);
    drop(scope);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn concurrent_last_factory_and_account_drop_release_every_byte_exactly_once() {
    use std::sync::Barrier;
    for _ in 0..128 {
        let budget = SharedBudget::new(16);
        let scope = budget
            .reserve_capacity(16)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let mut account = scope.account();
        account.resize(8).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let other = barrier.clone();
        let worker = std::thread::spawn(move || {
            other.wait();
            drop(scope);
        });
        barrier.wait();
        drop(account);
        worker.join().unwrap();
        assert_eq!(budget.usage().reserved, 0);
    }
}
