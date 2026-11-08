use super::*;
#[test]
fn fixed_control_shortages_refuse_before_check_or_debit_with_parent_headroom() {
    let input = fixture("manual-raw");
    let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
    let f = size_of::<PortableSource8Plan<'static>>() as u64;
    for capacity in [f - 1, f + census::control_bytes() - 1] {
        let budget = SharedBudget::new(8 << 20);
        let reservation = budget.reserve_capacity(capacity).unwrap();
        let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
        let checks = Cell::new(0u64);
        let used = Cell::new(0u64);
        let check = || {
            checks.set(checks.get() + 1);
            Ok(())
        };
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        assert_eq!(
            error(checked.portable_source8_plan_checked(
                Default::default(),
                &scope,
                &check,
                &spend
            ))
            .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(checks.get(), 0);
        assert_eq!(used.get(), 0);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(reservation.available(), capacity);
        drop(scope);
        drop(reservation);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn check_cancellation_keeps_typed_original_error_and_all_paid_prefixes() {
    let input = fixture("manual-raw");
    let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
    for cut in 1..=10 {
        let scope = SharedBudget::new(4 << 20).scope(AllocationClass::PaintMetadata);
        let calls = Cell::new(0u64);
        let used = Cell::new(0u64);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == cut {
                Err(stop())
            } else {
                Ok(())
            }
        };
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        assert_eq!(
            error(checked.portable_source8_plan_checked(
                Default::default(),
                &scope,
                &check,
                &spend
            )),
            stop()
        );
        assert_eq!(calls.get(), cut);
        assert!(used.get() >= 512 * cut);
        assert_eq!(scope.footprint().live, 0);
    }
}
#[test]
fn exact_first_primitive522_and_minus1_use_original_counter_on_retry() {
    let input = fixture("manual-raw");
    let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
    for limit in [521u64, 522] {
        let scope = SharedBudget::new(4 << 20).scope(AllocationClass::PaintMetadata);
        let used = Cell::new(0u64);
        let checks = Cell::new(0u64);
        let spend = |n| {
            let next = used
                .get()
                .checked_add(n)
                .filter(|n| *n <= limit)
                .ok_or_else(bound)?;
            used.set(next);
            Ok(())
        };
        let check = || {
            checks.set(checks.get() + 1);
            Ok(())
        };
        for _ in 0..2 {
            assert_eq!(
                error(checked.portable_source8_plan_checked(
                    Default::default(),
                    &scope,
                    &check,
                    &spend
                ))
                .code,
                ErrorCode::ResourceLimit
            );
            assert_eq!(used.get(), if limit == 522 { 522 } else { 0 });
            assert_eq!(checks.get(), u64::from(limit == 522));
            assert_eq!(scope.footprint().live, 0);
        }
    }
}
#[test]
fn cursor_refusal_keeps_plan_and_original_scope_amid_a_different_tracker() {
    let input = fixture("manual-raw");
    let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(8 << 20);
    let reservation = budget.reserve_capacity(4 << 20).unwrap();
    let original = reservation.clone().scope(AllocationClass::PaintMetadata);
    let other = reservation.clone().scope(AllocationClass::PaintMetadata);
    let used = Cell::new(0u64);
    let stop_now = Cell::new(false);
    let check = || if stop_now.get() { Err(stop()) } else { Ok(()) };
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    let p = checked
        .portable_source8_plan_checked(Default::default(), &original, &check, &spend)
        .unwrap();
    let retained = original.footprint().live;
    assert_eq!(other.footprint().live, 0);
    let mut blocker = other.account();
    blocker.resize(reservation.available()).unwrap();
    let paid = used.get();
    assert_eq!(error(p.header()).code, ErrorCode::ResourceLimit);
    assert_eq!(used.get(), paid);
    assert_eq!(original.footprint().live, retained);
    drop(blocker);
    stop_now.set(true);
    assert_eq!(error(p.header()), stop());
    assert!(used.get() > paid);
    assert_eq!(original.footprint().live, retained);
    stop_now.set(false);
    p.header().unwrap();
    assert_eq!(other.footprint().live, 0);
    drop(p);
    assert_eq!(original.footprint().live, 0);
    drop(original);
    drop(other);
    drop(reservation);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn spend_error_identity_precedes_any_helper_check_and_no_scope_switch_occurs() {
    let input = fixture("manual-raw");
    let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
    let scope = SharedBudget::new(4 << 20).scope(AllocationClass::PaintMetadata);
    let calls = Cell::new(0u64);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    assert_eq!(
        error(
            checked.portable_source8_plan_checked(Default::default(), &scope, &check, &|_| Err(
                stop()
            ))
        ),
        stop()
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(scope.footprint().live, 0);
}
