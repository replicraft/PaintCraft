use super::*;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};

#[test]
fn independent_target_work_counter_quotes_include_logical_arc_header() {
    let actual = WorkspaceAdmission::control_allocation_quote().unwrap();
    let expected = if cfg!(all(target_os = "macos", target_pointer_width = "64")) {
        Some(40)
    } else if cfg!(all(
        any(target_os = "linux", target_os = "windows"),
        target_pointer_width = "64"
    )) {
        Some(32)
    } else if cfg!(all(target_arch = "wasm32", target_os = "unknown")) {
        Some(24)
    } else {
        None
    };
    if let Some(expected) = expected {
        assert_eq!(actual, expected);
    }
    assert!(actual >= (2 * size_of::<usize>()) as u64);
    assert!(actual.is_multiple_of(align_of::<usize>() as u64));
}

#[test]
fn quote_preserves_original_byte_factory_and_shared_cumulative_work() {
    let budget = SharedBudget::new(1000);
    let reserved = budget.reserve_capacity(64).unwrap();
    let scope = reserved.clone().scope(AllocationClass::SharedBudget);
    let mut account = scope.account();
    account.resize(17).unwrap();
    let original = WorkspaceAdmission::new(scope.clone(), 64, 100);
    original.reserve_work(23).unwrap();
    let cloned = original.clone();
    let usage = budget.usage();
    let activity = budget.activity();
    let footprint = scope.footprint();
    let strong = std::sync::Arc::strong_count(&original.work_used);
    for _ in 0..2 {
        WorkspaceAdmission::control_allocation_quote().unwrap();
        assert_eq!(budget.usage(), usage);
        assert_eq!(budget.activity(), activity);
        assert_eq!(scope.footprint(), footprint);
        assert_eq!(reserved.available(), 47);
        assert_eq!(original.work_used(), 23);
        assert_eq!(cloned.work_used(), 23);
        assert!(original.same_job(&cloned));
        assert_eq!(std::sync::Arc::strong_count(&original.work_used), strong);
    }
    assert!(original.reserve_work(78).is_err());
    assert_eq!(cloned.work_used(), 23);
    cloned.reserve_work(77).unwrap();
    assert_eq!(original.work_used(), 100);
    assert!(original.reserve_work(1).is_err());
    drop(cloned);
    drop(original);
    drop(account);
    drop(scope);
    assert_eq!(budget.usage().reserved, 64);
    drop(reserved);
    assert_eq!(budget.usage().reserved, 0);
}
