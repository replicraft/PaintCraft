use super::super::tests::fixture;
use super::*;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
use std::cell::Cell;
#[test]
fn same_plan_materializes_enabled_disabled_empty_and_absent_without_parse_vectors() {
    for (name, enabled, value) in [
        ("old-own128", true, 128),
        ("old-disabled-own128", false, 128),
        ("old-enabled-own-zero", true, 0),
        ("old-zero-own-multiply", true, 0),
    ] {
        let bytes = fixture(name);
        let original = bytes.clone();
        let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(16 << 20);
        let plan_scope = budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let coverage_scope = budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::CoverageMasks);
        let mut guard = budget.account(AllocationClass::PaintMetadata);
        guard.resize(512).unwrap();
        let used = Cell::new(0u64);
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        let checks = Cell::new(0u64);
        let check = || {
            checks.set(checks.get() + 1);
            Ok(())
        };
        let plan = checked
            .portable_source8_plan_checked(Default::default(), &plan_scope, &check, &spend)
            .unwrap();
        let owner = plan.coverage_materializer(&coverage_scope, &guard).unwrap();
        assert!(owner.layer(0).unwrap().is_none());
        let result = owner.layer(1).unwrap();
        if name == "old-zero-own-multiply" {
            assert!(result.is_none());
        } else {
            let (actual_enabled, mask) = result.unwrap();
            assert_eq!(actual_enabled, enabled);
            assert_eq!(mask.domain(), Rect::from_origin_size(0, 0, 2, 1).unwrap());
            assert_eq!(mask.coverage(0, 0), value);
            assert_eq!(mask.coverage(1, 0), value);
            assert_eq!(mask.stats().bands, usize::from(value != 0));
            assert_eq!(mask.stats().runs, usize::from(value != 0));
            let before = coverage_scope.footprint().live;
            let (_, second) = owner.layer(1).unwrap().unwrap();
            if value != 0 {
                assert_ne!(
                    mask.storage_usage().unwrap().0,
                    second.storage_usage().unwrap().0
                );
                assert!(coverage_scope.footprint().live > before);
            } else {
                assert!(mask.storage_usage().is_none());
                assert!(second.storage_usage().is_none());
            }
            drop(second);
            drop(mask);
        }
        assert_eq!(plan.work_used(), used.get());
        assert_eq!(plan.check_calls(), checks.get());
        drop(owner);
        assert_eq!(coverage_scope.footprint().live, 0);
        drop(plan);
        assert_eq!(plan_scope.footprint().live, 0);
        assert_eq!(bytes, original);
    }
}
#[test]
fn original_coverage_zero_refuses_even_with_parent_headroom_and_keeps_same_work_debt() {
    let bytes = fixture("old-own128");
    let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(16 << 20);
    let plan_scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let coverage_scope = budget
        .reserve_capacity(0)
        .unwrap()
        .scope(AllocationClass::CoverageMasks);
    let mut guard = budget.account(AllocationClass::PaintMetadata);
    guard.resize(512).unwrap();
    let check = || Ok(());
    let used = Cell::new(0u64);
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    let plan = checked
        .portable_source8_plan_checked(Default::default(), &plan_scope, &check, &spend)
        .unwrap();
    for _ in 0..2 {
        let before = used.get();
        let error = plan
            .coverage_materializer(&coverage_scope, &guard)
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(used.get(), before + 64);
        assert_eq!(coverage_scope.footprint().live, 0);
    }
    assert_eq!(plan.work_used(), used.get());
}
#[test]
fn filter_indices_use_bound_plan_and_missing_storage_guard_never_selects_another() {
    let bytes = fixture("old-proof-filter-disabled");
    let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(16 << 20);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let guard = scope.account();
    let check = || Ok(());
    let spend = |_| Ok(());
    let plan = checked
        .portable_source8_plan_checked(Default::default(), &scope, &check, &spend)
        .unwrap();
    let owner = plan.coverage_materializer(&scope, &guard).unwrap();
    assert!(owner.filter(0, 0).unwrap().is_none());
    assert_eq!(owner.filter(0, 1).unwrap_err().code, ErrorCode::CodecFailed);
    drop(owner);
    drop(plan);
    assert_eq!(scope.footprint().live, 0);
    let bytes = fixture("old-own128");
    let checked = CheckedContainer::verify(&bytes, &check).unwrap();
    let plan = checked
        .portable_source8_plan_checked(Default::default(), &scope, &check, &spend)
        .unwrap();
    let owner = plan.coverage_materializer(&scope, &guard).unwrap();
    assert_eq!(owner.layer(1).unwrap_err().code, ErrorCode::ResourceLimit);
    drop(owner);
    drop(plan);
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn callback_refusal_and_retry_keep_same_plan_scope_and_original_counter() {
    let bytes = fixture("old-own128");
    let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(16 << 20);
    let plan_scope = budget.scope(AllocationClass::PaintMetadata);
    let coverage_scope = budget
        .reserve_capacity(1 << 20)
        .unwrap()
        .scope(AllocationClass::CoverageMasks);
    let mut guard = budget.account(AllocationClass::PaintMetadata);
    guard.resize(512).unwrap();
    let used = Cell::new(0u64);
    let fail = Cell::new(false);
    let sentinel = Error::new(ErrorCode::OperationCancelled, "bound original callback").at(63);
    let check = || {
        if fail.get() {
            Err(sentinel.clone())
        } else {
            Ok(())
        }
    };
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    let plan = checked
        .portable_source8_plan_checked(Default::default(), &plan_scope, &check, &spend)
        .unwrap();
    let owner = plan.coverage_materializer(&coverage_scope, &guard).unwrap();
    let original = coverage_scope.footprint().live;
    let before = used.get();
    fail.set(true);
    assert_eq!(owner.layer(1).unwrap_err(), sentinel);
    let refused = used.get();
    assert!(refused > before);
    assert_eq!(coverage_scope.footprint().live, original);
    fail.set(false);
    let (_, mask) = owner.layer(1).unwrap().unwrap();
    assert_eq!(mask.coverage(0, 0), 128);
    assert!(used.get() > refused);
    assert_eq!(plan.work_used(), used.get());
    drop(mask);
    drop(owner);
    assert_eq!(coverage_scope.footprint().live, 0);
}

mod filter_masks;
