use super::*;
#[test]
fn full_declared_encoded_control_and_plan_overlap_exact_and_one_short() {
    for short in [0, 1] {
        let doc = source("baseline");
        let budget = SharedBudget::new(128 << 20);
        let scope = scope(&budget);
        let check = || Ok(());
        let spend = |_| Ok(());
        let fact =
            count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
        // Defining controls/literal counts only. Never a producer grant or usage calibration.
        let p = controls::peaks(1, 0).unwrap().peak;
        let sizes = blend_counted_snapshot_control_bytes();
        let declared = sizes.fact + QUERY_CONTROL + fact.wire_len() + sizes.bytes_owner + p;
        let mut held = scope.account();
        held.resize(MAIN_MAX - declared + short).unwrap();
        let live = scope.footprint().live;
        match fact.encode_with_source_forecast_checked() {
            Ok((bytes, f)) => {
                assert_eq!(short, 0);
                assert_eq!(f.bootstrap_p_peak, p);
                assert_eq!(
                    scope.footprint().live,
                    live + fact.wire_len() + sizes.bytes_owner
                );
                drop(bytes);
            }
            Err(e) => {
                assert_eq!(short, 1);
                assert_eq!(e.code, ErrorCode::ResourceLimit);
                assert_eq!(e.message, "Source8 Main source forecast allowance");
            }
        }
        assert_eq!(scope.footprint().live, live);
        drop(held);
        drop(fact);
        assert_eq!(scope.footprint().live, 0);
        drop(scope);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn original_first_error_identity_and_retry_keep_exact_paid_prefix() {
    let doc = source("baseline");
    let budget = SharedBudget::new(128 << 20);
    let scope = scope(&budget);
    let used = Cell::new(7u64);
    let pending = RefCell::new(None);
    let check = || match pending.borrow_mut().take() {
        Some(e) => Err(e),
        None => Ok(()),
    };
    let spend = |n| {
        used.set(
            used.get()
                .checked_add(n)
                .filter(|n| *n <= 268435456)
                .unwrap(),
        );
        Ok(())
    };
    let fact = count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
    let sentinel = Error::new(
        ErrorCode::OperationCancelled,
        "original forecast callback".repeat(41),
    )
    .at(23);
    let pointer = sentinel.message.as_ptr();
    *pending.borrow_mut() = Some(sentinel);
    let before = used.get();
    let live = scope.footprint().live;
    let e = fact.encode_with_source_forecast_checked().unwrap_err();
    assert_eq!(e.code, ErrorCode::OperationCancelled);
    assert_eq!(e.message.as_ptr(), pointer);
    assert_eq!(e.operation_index, Some(23));
    assert_eq!(used.get() - before, 84336);
    assert_eq!(scope.footprint().live, live);
    let paid = used.get();
    let (bytes, _) = fact.encode_with_source_forecast_checked().unwrap();
    assert!(used.get() > paid);
    drop(bytes);
    drop(fact);
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn work64_refusal_precedes_new_account_lookup_and_raw_callback_without_reset() {
    let doc = source("baseline");
    let budget = SharedBudget::new(128 << 20);
    let scope = scope(&budget);
    let used = Cell::new(0u64);
    let ceiling = Cell::new(u64::MAX);
    let checks = Cell::new(0);
    let check = || {
        checks.set(checks.get() + 1);
        Ok(())
    };
    let spend = |n| {
        let next = used
            .get()
            .checked_add(n)
            .filter(|n| *n <= ceiling.get())
            .ok_or_else(controls::exhausted)?;
        used.set(next);
        Ok(())
    };
    let fact = count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
    let before = used.get();
    let calls = checks.get();
    let live = scope.footprint().live;
    let footprint = scope.footprint();
    let usage = budget.usage();
    let activity = budget.activity();
    ceiling.set(before + 64);
    for _ in 0..2 {
        assert_eq!(
            fact.encode_with_source_forecast_checked().unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(used.get(), before + 64);
        assert_eq!(checks.get(), calls);
        assert_eq!(scope.footprint().live, live);
        assert_eq!(scope.footprint(), footprint);
        assert_eq!(budget.usage(), usage);
        assert_eq!(budget.activity(), activity);
    }
}
#[test]
fn unreserved_main_factory_refuses_without_encoding_or_new_root() {
    let doc = source("baseline");
    let budget = SharedBudget::new(128 << 20);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let check = || Ok(());
    let spend = |_| Ok(());
    let fact = count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
    let live = scope.footprint().live;
    let reserved = budget.usage().reserved;
    assert_eq!(
        fact.encode_with_source_forecast_checked().unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(scope.footprint().live, live);
    assert_eq!(budget.usage().reserved, reserved);
}
