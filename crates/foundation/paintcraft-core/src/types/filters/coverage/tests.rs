use super::*;
#[test]
fn negative_domain_ordered_zero_fractional_and_shared_numeric_ownership() {
    let budget = SharedBudget::new(4096);
    let domain = Rect::new(-4, -3, 8, 7).unwrap();
    let mut b = A8InputBuilder::new(
        domain,
        2,
        3,
        &budget,
        MaskInputLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    b.push_band(-2, 0).unwrap();
    b.push_run(A8Run {
        x0: -3,
        x1: -1,
        coverage: 0,
    })
    .unwrap();
    b.push_run(A8Run {
        x0: 0,
        x1: 2,
        coverage: 128,
    })
    .unwrap();
    b.push_band(1, 3).unwrap();
    b.push_run(A8Run {
        x0: -1,
        x1: 4,
        coverage: 255,
    })
    .unwrap();
    let input = b.finish().unwrap();
    assert_eq!(input.domain(), domain);
    assert_eq!(input.band_count(), 2);
    assert_eq!(input.run_count(), 3);
    assert_eq!(input.bands().next().unwrap().runs[1].coverage, 128);
    let clone = input.clone();
    assert_eq!(input.storage_usage(), clone.storage_usage());
    let charged = budget.usage().reserved;
    assert!(charged > 0);
    drop(input);
    assert_eq!(budget.usage().reserved, charged);
    drop(clone);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn deny_before_allocation_and_cancel_partial_input_release_exact_realms() {
    let domain = Rect::new(0, 0, 8, 8).unwrap();
    let denied = SharedBudget::new(1);
    let error = A8InputBuilder::new(
        domain,
        1,
        1,
        &denied,
        MaskInputLimits::default(),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(
        error.allocation_denial().unwrap().class(),
        AllocationClass::CoverageMasks
    );
    assert_eq!(denied.usage().reserved, 0);
    let budget = SharedBudget::new(4096);
    let cancelled = std::cell::Cell::new(false);
    let check = || {
        if cancelled.get() {
            Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
        } else {
            Ok(())
        }
    };
    let mut b =
        A8InputBuilder::new(domain, 1, 1, &budget, MaskInputLimits::default(), &check).unwrap();
    b.push_band(0, 1).unwrap();
    cancelled.set(true);
    assert_eq!(
        b.push_run(A8Run {
            x0: 0,
            x1: 1,
            coverage: 1
        })
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    drop(b);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn malformed_order_bounds_counts_and_work_refuse_without_retained_storage() {
    let budget = SharedBudget::new(4096);
    let d = Rect::new(0, 0, 8, 8).unwrap();
    let mut b =
        A8InputBuilder::new(d, 1, 1, &budget, MaskInputLimits::default(), &|| Ok(())).unwrap();
    assert_eq!(
        b.push_run(A8Run {
            x0: 0,
            x1: 1,
            coverage: 1
        })
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
    b.push_band(0, 1).unwrap();
    assert_eq!(
        b.push_run(A8Run {
            x0: -1,
            x1: 1,
            coverage: 1
        })
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
    drop(b);
    let b = A8InputBuilder::new(d, 1, 0, &budget, MaskInputLimits::default(), &|| Ok(())).unwrap();
    assert_eq!(b.finish().unwrap_err().code, ErrorCode::InputSchema);
    let mut b = A8InputBuilder::new(
        d,
        1,
        0,
        &budget,
        MaskInputLimits {
            work: 0,
            ..Default::default()
        },
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        b.push_band(0, 1).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    drop(b);
    assert_eq!(budget.usage().reserved, 0);
}
