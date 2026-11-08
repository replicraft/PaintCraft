use super::*;
#[test]
fn original_constructor_control4095_refuses_before_callbacks_with_global_headroom() {
    let icc = SharedBudget::new(1 << 20);
    let reservation = icc.reserve_capacity(1024).unwrap();
    let source = SharedBudget::new(16384);
    let scope = source
        .reserve_capacity(4095)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let guard_scope = source.scope(AllocationClass::PaintMetadata);
    let mut guard = guard_scope.account();
    guard.resize(512).unwrap();
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let spend = |_| {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let error =
        ReservedSourceMetadataReader::new(&reservation, &scope, &guard, 8 << 20, &check, &spend)
            .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(calls.get(), 0);
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn original_icc_exact_minus_one_never_borrows_parent_spare() {
    let row = rows().remove(2);
    let h = bytes(&row, "header_hex");
    let p = bytes(&row, "profile_hex");
    let exact = expected_charge(132);
    for capacity in [exact - 1, exact] {
        let icc = SharedBudget::new(1 << 20);
        let reservation = icc.reserve_capacity(capacity).unwrap();
        let source = SharedBudget::new(8192);
        let scope = source.scope(AllocationClass::PaintMetadata);
        let mut guard = scope.account();
        guard.resize(512).unwrap();
        let check = || Ok(());
        let spend = |_| Ok(());
        let reader = ReservedSourceMetadataReader::new(
            &reservation,
            &scope,
            &guard,
            8 << 20,
            &check,
            &spend,
        )
        .unwrap();
        let result = {
            let recipe = reader.inspect(&h, &p).unwrap();
            recipe.materialize()
        };
        if capacity == exact {
            drop(result.unwrap());
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
        }
        assert_eq!(reservation.available(), capacity);
        drop(reader);
        drop(guard);
        drop(scope);
        drop(reservation);
        assert_eq!(icc.usage().reserved, 0);
        assert_eq!(source.usage().reserved, 0);
    }
}
#[test]
fn exact_original_callback_error_and_paid_prefix_survive_borrowed_validation_cuts() {
    let row = rows().remove(2);
    let h = bytes(&row, "header_hex");
    let p = bytes(&row, "profile_hex");
    for stop in 1..=8 {
        let icc = SharedBudget::new(1 << 20);
        let reservation = icc.reserve_capacity(1024).unwrap();
        let source = SharedBudget::new(8192);
        let scope = source.scope(AllocationClass::PaintMetadata);
        let mut guard = scope.account();
        guard.resize(512).unwrap();
        let calls = Cell::new(0);
        let used = Cell::new(0u64);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(sentinel())
            } else {
                Ok(())
            }
        };
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        let result = ReservedSourceMetadataReader::new(
            &reservation,
            &scope,
            &guard,
            8 << 20,
            &check,
            &spend,
        );
        match result {
            Err(e) => assert_eq!(e, sentinel()),
            Ok(reader) => {
                assert_eq!(reader.inspect(&h, &p).unwrap_err(), sentinel());
            }
        }
        assert_eq!(calls.get(), stop);
        assert!(used.get() >= 512 * stop as u64);
        assert_eq!(reservation.available(), 1024);
        assert_eq!(scope.footprint().live, 512);
    }
}
#[test]
fn canceled_copy_and_publish_keep_original_debt_and_retire_vec_before_lease() {
    let row = rows().remove(2);
    let h = bytes(&row, "header_hex");
    let p = bytes(&row, "profile_hex");
    for stop in [1, 2, 4, 5] {
        let icc = SharedBudget::new(1 << 20);
        let reservation = icc.reserve_capacity(1024).unwrap();
        let source = SharedBudget::new(8192);
        let scope = source.scope(AllocationClass::PaintMetadata);
        let mut guard = scope.account();
        guard.resize(512).unwrap();
        let calls = Cell::new(0);
        let cut = Cell::new(0);
        let used = Cell::new(0u64);
        let check = || {
            calls.set(calls.get() + 1);
            if cut.get() != 0 && calls.get() == cut.get() {
                Err(sentinel())
            } else {
                Ok(())
            }
        };
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        let reader = ReservedSourceMetadataReader::new(
            &reservation,
            &scope,
            &guard,
            8 << 20,
            &check,
            &spend,
        )
        .unwrap();
        let recipe = reader.inspect(&h, &p).unwrap();
        let paid = used.get();
        calls.set(0);
        cut.set(stop);
        assert_eq!(recipe.materialize().unwrap_err(), sentinel());
        assert_eq!(reservation.available(), 1024);
        assert!(used.get() > paid);
        let after = used.get();
        cut.set(0);
        let metadata = recipe.materialize().unwrap();
        assert!(used.get() > after);
        assert_eq!(metadata.wire_header().as_slice(), h);
        drop(metadata);
        assert_eq!(reservation.available(), 1024);
    }
}

#[test]
fn each_canonical_debit_refuses_before_helper_and_retry_keeps_original_prefix() {
    let row = rows().remove(2);
    let header = bytes(&row, "header_hex");
    let profile = bytes(&row, "profile_hex");
    assert_eq!(profile.len(), 132);
    assert_eq!(CANONICAL_WORK, 384);
    for cut in [1, 2] {
        let icc = SharedBudget::new(1 << 20);
        let reservation = icc.reserve_capacity(1024).unwrap();
        let source = SharedBudget::new(8192);
        let scope = source
            .reserve_capacity(8192)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let mut guard = scope.account();
        guard.resize(512).unwrap();
        let used = Cell::new(0u64);
        let checks = Cell::new(0u64);
        let ordinal = Cell::new(0u64);
        let refuse = Cell::new(true);
        let expected = Error::new(ErrorCode::ResourceLimit, "canonical debit refused").at(31);
        let spend = |n: u64| {
            if n == 896 {
                ordinal.set(ordinal.get() + 1);
                if refuse.get() && ordinal.get() == cut {
                    return Err(expected.clone());
                }
            }
            let next = used
                .get()
                .checked_add(n)
                .filter(|n| *n <= 268435456)
                .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "original work ceiling"))?;
            used.set(next);
            Ok(())
        };
        let check = || {
            checks.set(checks.get() + 1);
            Ok(())
        };
        let reader = ReservedSourceMetadataReader::new(
            &reservation,
            &scope,
            &guard,
            8 << 20,
            &check,
            &spend,
        )
        .unwrap();
        assert_eq!(reader.inspect(&header, &profile).unwrap_err(), expected);
        assert_eq!(ordinal.get(), cut);
        assert_eq!(used.get(), if cut == 1 { 5192 } else { 7436 });
        assert_eq!(checks.get(), if cut == 1 { 9 } else { 12 });
        assert_eq!(
            reservation.available(),
            1024,
            "no retained ICC before recipe success"
        );
        assert_eq!(scope.footprint().live, 4608);
        let paid = used.get();
        let calls = checks.get();
        // Only reset a diagnostic phase ordinal; actual original debt stays paid.
        ordinal.set(0);
        refuse.set(false);
        {
            let recipe = reader.inspect(&header, &profile).unwrap();
            assert_eq!(ordinal.get(), 2);
            assert_eq!(used.get(), paid + 8460); // full9036 minus already paid constructor576
            assert_eq!(checks.get(), calls + 13);
            assert_eq!(reservation.available(), 1024);
            let metadata = recipe.materialize().unwrap();
            assert_eq!(metadata.wire_header().as_slice(), header);
            drop(metadata);
            assert_eq!(reservation.available(), 1024);
        }
        drop(reader);
        assert_eq!(scope.footprint().live, 512);
        drop(guard);
        drop(scope);
        drop(reservation);
        assert_eq!(source.usage().reserved, 0);
        assert_eq!(icc.usage().reserved, 0);
    }
}
