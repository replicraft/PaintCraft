use super::*;
use std::cell::Cell;
#[test]
fn independent_fresh_indices_reserve_actual_records_not_geometry() {
    let budget = SharedBudget::new(4 << 20);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let mut account = scope.account();
    account.resize(4096).unwrap();
    for (n, b, a) in [
        (0, 0, 0),
        (1, 4, 116),
        (3, 4, 116),
        (4, 8, 216),
        (7, 8, 216),
        (8, 16, 416),
    ] {
        let minimum = (n.max(1) as u32) * 256;
        for width in [minimum, 1048576] {
            let paid = Cell::new(0u64);
            let checks = Cell::new(0);
            let r = RasterLayer::new_decode_index_precharged_checked(
                width,
                1,
                Rgba::TRANSPARENT,
                n,
                &account,
                &|| {
                    checks.set(checks.get() + 1);
                    Ok(())
                },
                &|v| {
                    paid.set(paid.get() + v);
                    Ok(())
                },
            )
            .unwrap();
            let q = r.retained_tile_table_quote().unwrap();
            assert_eq!(
                (q.allocation_upper, q.bucket_upper, q.live_entries),
                (a, b, 0)
            );
            assert_eq!(paid.get(), 64 + a + 2 * 512);
            assert_eq!(checks.get(), 2);
            assert_eq!(scope.footprint().live, 4096);
            drop(r); // Real map ends before the externally retained original account.
        }
    }
    drop(account);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn original_metadata116_exact_and115_short_never_select_parent_headroom() {
    for capacity in [115, 116] {
        let budget = SharedBudget::new(1 << 20);
        let reservation = budget.reserve_capacity(capacity).unwrap();
        let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
        let mut account = scope.account();
        account.resize(capacity).unwrap();
        let paid = Cell::new(0u64);
        let result = RasterLayer::new_decode_index_precharged_checked(
            256,
            1,
            Rgba::TRANSPARENT,
            1,
            &account,
            &|| Ok(()),
            &|n| {
                paid.set(paid.get() + n);
                Ok(())
            },
        );
        if capacity == 116 {
            let r = result.unwrap();
            assert_eq!(r.retained_tile_table_quote().unwrap().allocation_upper, 116);
            drop(r);
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
            assert_eq!(paid.get(), 576);
        }
        assert_eq!(account.bytes(), capacity);
        assert_eq!(scope.footprint().live, capacity);
        drop(account);
        assert_eq!(reservation.available(), capacity);
        drop(scope);
        drop(reservation);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn typed_callback_failure_precedes_allocation_and_retains_paid_debt() {
    let budget = SharedBudget::new(4096);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let mut account = scope.account();
    account.resize(116).unwrap();
    for stop in [1, 2] {
        let calls = Cell::new(0);
        let used = Cell::new(0u64);
        let expected = Error::new(ErrorCode::OperationCancelled, "fresh index sentinel").at(3);
        let result = RasterLayer::new_decode_index_precharged_checked(
            256,
            1,
            Rgba::TRANSPARENT,
            1,
            &account,
            &|| {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(expected.clone())
                } else {
                    Ok(())
                }
            },
            &|n| {
                used.set(used.get() + n);
                Ok(())
            },
        );
        assert_eq!(result.unwrap_err(), expected);
        assert_eq!(calls.get(), stop);
        assert_eq!(used.get(), if stop == 1 { 576 } else { 1204 });
        assert_eq!(scope.footprint().live, 116);
    }
    let result = RasterLayer::new_decode_index_precharged_checked(
        256,
        1,
        Rgba::TRANSPARENT,
        1,
        &account,
        &|| panic!("unpaid callback"),
        &|_| Err(Error::new(ErrorCode::ResourceLimit, "original counter")),
    );
    assert_eq!(result.unwrap_err().message, "original counter");
    drop(account);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn invalid_geometry_or_more_records_than_positions_creates_no_raster() {
    let budget = SharedBudget::new(4096);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let mut account = scope.account();
    account.resize(116).unwrap();
    for (w, h, n) in [(0, 1, 0), (1, 0, 0), (1048577, 1, 0), (1, 1, 2)] {
        assert_eq!(
            RasterLayer::new_decode_index_precharged_checked(
                w,
                h,
                Rgba::TRANSPARENT,
                n,
                &account,
                &|| Ok(()),
                &|_| Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
    }
    assert_eq!(scope.footprint().live, 116);
    drop(account);
    assert_eq!(budget.usage().reserved, 0);
}
