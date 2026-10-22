use super::*;
#[test]
fn scoped_denial_prices_all_live_accounts_and_keeps_the_observed_peak_after_release() {
    let budget = SharedBudget::new(20);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let mut a = scope.account();
    a.resize(8).unwrap();
    let mut b = a.sibling();
    b.resize(8).unwrap();
    let blocker = budget.reserve(4).unwrap();
    let error = b.resize(12).unwrap_err();
    assert_eq!(error.allocation_denial().unwrap().required_bytes(), 20);
    assert_eq!(scope.footprint(), ScopeFootprint { live: 16, peak: 20 });
    assert_eq!(b.bytes(), 8);
    drop(a);
    assert_eq!(scope.footprint(), ScopeFootprint { live: 8, peak: 20 });
    drop(b);
    assert_eq!(scope.footprint(), ScopeFootprint { live: 0, peak: 20 });
    drop(blocker);
    assert!(error.allocation_denial().unwrap().can_retry());
}
#[test]
fn cloned_factories_share_declarations_without_pinning_global_capacity() {
    let budget = SharedBudget::new(32);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let other = scope.clone();
    let mut a = scope.account();
    a.resize(12).unwrap();
    let mut b = other.account();
    b.resize(8).unwrap();
    assert_eq!(scope.footprint(), ScopeFootprint { live: 20, peak: 20 });
    drop(a);
    drop(b);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(other.footprint().live, 0);
}
#[test]
fn scoped_callbacks_can_reenter_footprint_observation_without_a_scope_lock_cycle() {
    let budget = SharedBudget::new(8);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let observations = Arc::new(Mutex::new(Vec::new()));
    let recorded = observations.clone();
    let observed = scope.clone();
    let watch = budget
        .watch(1, move || {
            recorded.lock().unwrap().push(observed.footprint())
        })
        .unwrap();
    let mut account = scope.account();
    account.resize(8).unwrap();
    watch.observe();
    drop(account);
    let values = observations.lock().unwrap();
    assert!(values.contains(&ScopeFootprint { live: 8, peak: 8 }));
    assert!(values.contains(&ScopeFootprint { live: 0, peak: 8 }));
}
