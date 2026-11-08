use super::*;
#[test]
fn original_fact_and_counting_control_shortages_never_borrow_global_headroom() {
    let (source, _) = fixture("v1-default");
    let sizes = normal_counted_snapshot_control_bytes();
    for capacity in [sizes.fact - 1, sizes.fact + sizes.counting_sink - 1] {
        let budget = SharedBudget::new(8 << 20);
        let reservation = budget.reserve_capacity(capacity).unwrap();
        let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
        let paid = Cell::new(0u64);
        let failure = error(count_portable_normal_snapshot_rooted_checked(
            &source,
            &scope,
            &|| Ok(()),
            &|n| {
                paid.set(paid.get() + n);
                Ok(())
            },
        ));
        assert_eq!(failure.code, ErrorCode::ResourceLimit);
        assert_eq!(paid.get(), 0);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(reservation.available(), capacity);
        assert_eq!(budget.usage().reserved, capacity);
        drop(scope);
        drop(reservation);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn zeroing_and_next_writer_debit_keep_original_fact_and_paid_prefix_for_retry() {
    let (source, expected) = fixture("v1-default");
    let n = expected.len() as u64;
    let sizes = normal_counted_snapshot_control_bytes();
    let budget = SharedBudget::new(8 << 20);
    let scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let used = Cell::new(0u64);
    let mode = Cell::new(0);
    let boundary = Cell::new(0u64);
    let spend = |debit: u64| {
        if mode.get() == 1 || (mode.get() == 2 && used.get() > boundary.get()) {
            return Err(Error::new(ErrorCode::ResourceLimit, "same counter denied"));
        }
        used.set(used.get() + debit);
        Ok(())
    };
    let check = || Ok(());
    let fact =
        count_portable_normal_snapshot_rooted_checked(&source, &scope, &check, &spend).unwrap();
    let first = used.get();
    mode.set(1);
    assert_eq!(error(fact.encode()).message, "same counter denied");
    assert_eq!(used.get(), first);
    assert_eq!(scope.footprint().live, sizes.fact);
    mode.set(2);
    boundary.set(first);
    assert_eq!(error(fact.encode()).message, "same counter denied");
    assert_eq!(used.get(), first + n);
    assert_eq!(scope.footprint().live, sizes.fact);
    mode.set(0);
    let output = fact.encode().unwrap();
    assert_eq!(output.as_bytes(), expected);
    assert_eq!(used.get(), 2 * first + 2 * n);
    drop(output);
    drop(fact);
    assert_eq!(scope.footprint().live, 0);
    drop(scope);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn exact_n_minus_one_zeroing_budget_refuses_before_any_writer_debit() {
    let (source, expected) = fixture("v1-default");
    let budget = SharedBudget::new(4 << 20);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let used = Cell::new(0u64);
    let maximum = Cell::new(u64::MAX);
    let spend = |n| {
        let next = used.get().checked_add(n).unwrap();
        if next > maximum.get() {
            return Err(Error::new(ErrorCode::ResourceLimit, "fixed work ceiling"));
        }
        used.set(next);
        Ok(())
    };
    let check = || Ok(());
    let fact =
        count_portable_normal_snapshot_rooted_checked(&source, &scope, &check, &spend).unwrap();
    let paid = used.get();
    maximum.set(paid + expected.len() as u64 - 1);
    assert_eq!(error(fact.encode()).message, "fixed work ceiling");
    assert_eq!(used.get(), paid);
    assert_eq!(
        scope.footprint().live,
        normal_counted_snapshot_control_bytes().fact
    );
    drop(fact);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn real_output_scope_shortage_keeps_fact_and_assigned_blocker_without_spending() {
    let (source, expected) = fixture("v1-default");
    let sizes = normal_counted_snapshot_control_bytes();
    let capacity = 4 << 20;
    let budget = SharedBudget::new(8 << 20);
    let reservation = budget.reserve_capacity(capacity).unwrap();
    let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
    let used = Cell::new(0u64);
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    let check = || Ok(());
    let fact =
        count_portable_normal_snapshot_rooted_checked(&source, &scope, &check, &spend).unwrap();
    let pending = expected.len() as u64 + sizes.bytes_owner + sizes.fixed_sink;
    let mut blocker = scope.account();
    blocker.resize(capacity - sizes.fact - pending + 1).unwrap();
    assert_eq!(reservation.available(), pending - 1);
    let paid = used.get();
    assert_eq!(error(fact.encode()).code, ErrorCode::ResourceLimit);
    assert_eq!(used.get(), paid);
    assert_eq!(scope.footprint().live, sizes.fact + blocker.bytes());
    assert_eq!(reservation.available(), pending - 1);
    drop(blocker);
    let output = fact.encode().unwrap();
    assert_eq!(output.as_bytes(), expected);
    assert!(used.get() > paid);
    drop(output);
    drop(fact);
    assert_eq!(scope.footprint().live, 0);
    drop(scope);
    drop(reservation);
    assert_eq!(budget.usage().reserved, 0);
}
