use super::*;
#[test]
fn count_cancellation_boundaries_return_original_typed_error_without_fact() {
    let (source, _) = fixture("v7-filter-and-own-stored");
    for stop in [1, 2, 4, 8, 16] {
        let budget = SharedBudget::new(4 << 20);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let checks = Cell::new(0);
        let expected = Error::new(ErrorCode::OperationCancelled, "original sentinel").at(17);
        let check = || {
            checks.set(checks.get() + 1);
            if checks.get() == stop {
                Err(expected.clone())
            } else {
                Ok(())
            }
        };
        let failure = error(count_portable_normal_snapshot_rooted_checked(
            &source,
            &scope,
            &check,
            &|_| Ok(()),
        ));
        assert_eq!(failure, expected);
        assert_eq!(checks.get(), stop);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn allocation_denial_context_survives_writer_io_mapping() {
    let (source, _) = fixture("v1-default");
    let denied_budget = SharedBudget::new(0);
    let mut denied = denied_budget.account(AllocationClass::PaintMetadata);
    let expected = denied
        .resize(73)
        .unwrap_err()
        .with_message("admission sentinel")
        .at(9);
    let budget = SharedBudget::new(4 << 20);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let failure = error(count_portable_normal_snapshot_rooted_checked(
        &source,
        &scope,
        &|| Ok(()),
        &|_| Err(expected.clone()),
    ));
    assert_eq!(failure, expected);
    assert_eq!(failure.allocation_denial().unwrap().required_bytes(), 73);
    assert_eq!(
        failure.allocation_denial().unwrap().class(),
        AllocationClass::PaintMetadata
    );
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn encode_entry_and_writer_cancellation_preserve_fact_and_bytes_never_escape() {
    let (source, expected_bytes) = fixture("v1-default");
    let budget = SharedBudget::new(4 << 20);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let checks = Cell::new(0);
    let stop = Cell::new(u64::MAX);
    let expected = Error::new(ErrorCode::OperationCancelled, "real sentinel").at(3);
    let check = || {
        checks.set(checks.get() + 1);
        if checks.get() == stop.get() {
            Err(expected.clone())
        } else {
            Ok(())
        }
    };
    let spend = |_| Ok(());
    let fact =
        count_portable_normal_snapshot_rooted_checked(&source, &scope, &check, &spend).unwrap();
    for offset in [1, 4, 8] {
        stop.set(checks.get() + offset);
        assert_eq!(error(fact.encode()), expected);
        assert_eq!(
            scope.footprint().live,
            normal_counted_snapshot_control_bytes().fact
        );
    }
    stop.set(u64::MAX);
    let output = fact.encode().unwrap();
    assert_eq!(output.as_bytes(), expected_bytes);
    drop(output);
    drop(fact);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn count_and_real_trailer_cancel_keep_typed_first_error() {
    let (source, _) = fixture("v1-default");
    for cancel_count in [true, false] {
        let budget = SharedBudget::new(4 << 20);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let refuse = Cell::new(cancel_count);
        let seen = Cell::new(0);
        let expected = Error::new(ErrorCode::OperationCancelled, "trailer sentinel").at(32);
        let spend = |n| {
            if n == 64 {
                seen.set(seen.get() + 1);
                if refuse.get() {
                    return Err(expected.clone());
                }
            }
            Ok(())
        };
        let check = || Ok(());
        let result = count_portable_normal_snapshot_rooted_checked(&source, &scope, &check, &spend);
        if cancel_count {
            assert_eq!(error(result), expected);
        } else {
            let fact = result.unwrap();
            refuse.set(true);
            assert_eq!(error(fact.encode()), expected);
            assert_eq!(
                scope.footprint().live,
                normal_counted_snapshot_control_bytes().fact
            );
            drop(fact);
        }
        assert!(seen.get() > 0);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(budget.usage().reserved, 0);
    }
}
