use super::*;

#[test]
fn fixed_original_control_minus_one_then_unblocked_retry_keep_debt() {
    let input = fixture("baseline");
    let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(4 << 20);
    let scope = budget
        .reserve_capacity(16384)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let used = Cell::new(0u64);
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    let check = || Ok(());
    let plan = checked
        .portable_source8_plan_checked(Default::default(), &scope, &check, &spend)
        .unwrap();
    assert_eq!(scope.footprint().live, expected(2712, 1584));
    let before = used.get();
    let mut blocker = scope.account();
    blocker.resize(expected(12633, 13865)).unwrap();
    let live = scope.footprint().live;
    assert_eq!(
        refused(plan.retained_source_layout_checked()).code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(used.get() - before, 64);
    assert_eq!(scope.footprint().live, live);
    drop(blocker);
    let recipe = plan.retained_source_layout_checked().unwrap();
    assert_eq!(used.get() - before, 64 + 13188);
    assert_eq!(plan.work_used(), used.get());
    drop(recipe);
    drop(plan);
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn exact_original_scope_includes_existing_header_helper_overlap() {
    let input = fixture("baseline");
    let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
    for capacity in [expected(13168, 9780) - 1, expected(13168, 9780)] {
        let budget = SharedBudget::new(4 << 20);
        let scope = budget
            .reserve_capacity(capacity)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let used = Cell::new(0u64);
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        let check = || Ok(());
        let plan = checked
            .portable_source8_plan_checked(Default::default(), &scope, &check, &spend)
            .unwrap();
        let before = used.get();
        let live = scope.footprint().live;
        if capacity == expected(13168, 9780) {
            let recipe = plan.retained_source_layout_checked().unwrap();
            assert_eq!(used.get() - before, 13188);
            assert_eq!(scope.footprint().peak, capacity);
            drop(recipe);
        } else {
            assert_eq!(
                refused(plan.retained_source_layout_checked()).code,
                ErrorCode::ResourceLimit
            );
            assert_eq!(used.get() - before, 760);
        }
        assert_eq!(scope.footprint().live, live);
        assert_eq!(plan.work_used(), used.get());
    }
}
#[test]
fn every_actual_baseline_callback_keeps_raw_typed_error_source_and_paid_prefix() {
    let input = fixture("baseline");
    let original = input.clone();
    let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
    for cut in 1..=24 {
        let scope = SharedBudget::new(4 << 20).scope(AllocationClass::PaintMetadata);
        let calls = Cell::new(0u64);
        let armed = Cell::new(false);
        let used = Cell::new(0u64);
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        let check = || {
            if armed.get() {
                calls.set(calls.get() + 1);
                if calls.get() == cut {
                    return Err(sentinel());
                }
            }
            Ok(())
        };
        let plan = checked
            .portable_source8_plan_checked(Default::default(), &scope, &check, &spend)
            .unwrap();
        let before = used.get();
        let live = scope.footprint().live;
        armed.set(true);
        assert_eq!(refused(plan.retained_source_layout_checked()), sentinel());
        assert_eq!(calls.get(), cut);
        const PREFIXES: [u64; 24] = [
            592, 1273, 1786, 2308, 2830, 3343, 3856, 4378, 5034, 5547, 6069, 6581, 7094, 7607,
            8120, 8642, 9164, 9686, 10208, 10721, 11243, 11884, 12428, 12972,
        ];
        assert_eq!(
            used.get() - before,
            PREFIXES[(cut - 1) as usize],
            "callback {cut}"
        );
        assert_eq!(scope.footprint().live, live);
        assert_eq!(plan.work_used(), used.get());
        let failed = used.get();
        armed.set(false);
        let recipe = plan.retained_source_layout_checked().unwrap();
        assert_eq!(used.get(), failed + 13188);
        assert_eq!(input, original);
        drop(recipe);
    }
}
#[test]
fn fixed_scalar_initialization_final_check_and_move_have_causal_refusal_prefixes() {
    let input = fixture("baseline");
    let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
    for (allowance, paid) in [
        (0, 0),
        (63, 0),
        (64, 64),
        (591, 64),
        (592, 592),
        (759, 592),
        (760, 760),
        (12428, 12428),
        (12971, 12428),
        (12972, 12972),
        (13187, 12972),
    ] {
        let scope = SharedBudget::new(4 << 20).scope(AllocationClass::PaintMetadata);
        let used = Cell::new(0u64);
        let ceiling = Cell::new(u64::MAX);
        let denied = Error::new(ErrorCode::ResourceLimit, "same host meter sentinel").at(31);
        let spend = |n| {
            let next = used
                .get()
                .checked_add(n)
                .filter(|n| *n <= ceiling.get())
                .ok_or_else(|| denied.clone())?;
            used.set(next);
            Ok(())
        };
        let check = || Ok(());
        let plan = checked
            .portable_source8_plan_checked(Default::default(), &scope, &check, &spend)
            .unwrap();
        let before = used.get();
        let live = scope.footprint().live;
        ceiling.set(before + allowance);
        assert_eq!(refused(plan.retained_source_layout_checked()), denied);
        assert_eq!(used.get() - before, paid, "allowance {allowance}");
        assert_eq!(plan.work_used(), used.get());
        assert_eq!(scope.footprint().live, live);
        assert_eq!(plan.facts().layers, 1);
    }
}
