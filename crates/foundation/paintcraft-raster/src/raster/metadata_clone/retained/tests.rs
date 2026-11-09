use super::*;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
use std::cell::{Cell, RefCell};
fn source(clear: usize) -> RasterLayer {
    let mut raster = RasterLayer::new(4096, 1, Rgba::TRANSPARENT);
    raster
        .blend_pixel(
            0,
            0,
            Rgba([7, 8, 9, 0]),
            BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    if clear != usize::MAX {
        let tile = Arc::clone(raster.tiles.values().next().unwrap());
        raster.tiles.try_reserve(14).unwrap();
        for tx in 1..15 {
            raster
                .tiles
                .insert_reserved(TileCoord { tx, ty: 0 }, Arc::clone(&tile))
                .unwrap();
        }
        for tx in 0..clear {
            raster.tiles.remove(&TileCoord {
                tx: tx as i64,
                ty: 0,
            });
        }
    }
    raster
}
fn funded(bytes: u64) -> (SharedBudget, BudgetAccount) {
    let budget = SharedBudget::new(bytes);
    let mut account = budget.account(AllocationClass::PaintMetadata);
    account.resize(bytes).unwrap();
    (budget, account)
}
#[test]
fn independently_frozen_prices_and_overflow() {
    assert_eq!(size_of::<TileCoord>(), 16);
    for (a, b, n, expected) in [
        (0, 0, 0, 64),
        (816, 32, 0, 64),
        (116, 4, 1, 209),
        (816, 32, 1, 937),
        (116, 4, 2, 234),
    ] {
        let q = RetainedTileTableQuote {
            allocation_upper: a,
            bucket_upper: b,
            live_entries: n,
            scan_work_upper: a + b as u64,
        };
        let layout = fresh_map_layout::<TileCoord, Arc<Tile>>(n).unwrap();
        assert_eq!(add(64, remaining(q, layout).unwrap()).unwrap(), expected);
    }
    let q = RetainedTileTableQuote {
        allocation_upper: 0,
        bucket_upper: 0,
        live_entries: 1,
        scan_work_upper: u64::MAX,
    };
    assert_eq!(
        remaining(q, fresh_map_layout::<TileCoord, Arc<Tile>>(1).unwrap())
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn fresh_and_tombstoned_source_use_literal_trace_and_share_versions() {
    for (clear, expected) in [
        (usize::MAX, vec![64, 145, 121, 116, 40]),
        (14, vec![64, 873, 121, 116, 40]),
    ] {
        let source = source(clear);
        let (budget, account) = funded(116);
        let trace = RefCell::new(Vec::new());
        let clone = source
            .compact_metadata_clone_retained_precharged_checked(&account, &|| Ok(()), &|n| {
                trace.borrow_mut().push(n);
                Ok(())
            })
            .unwrap();
        assert_eq!(*trace.borrow(), expected);
        assert_eq!(clone.resident_tiles(), 1);
        assert_eq!(clone.width(), source.width());
        assert_eq!(clone.default, source.default);
        assert!(Arc::ptr_eq(
            source.tiles.values().next().unwrap(),
            clone.tiles.values().next().unwrap()
        ));
        assert_eq!(clone.tiles.retained_quote().unwrap().bucket_upper, 4);
        assert_eq!(budget.usage().reserved, 116);
        drop(clone);
        drop(account);
        assert_eq!(budget.usage().reserved, 0);
        let old = RefCell::new(Vec::new());
        let (_, account) = funded(116);
        let legacy = source
            .compact_metadata_clone_precharged_checked(&account, &|| Ok(()), &|n| {
                old.borrow_mut().push(n);
                Ok(())
            })
            .unwrap();
        assert_eq!(*old.borrow(), [116, 40]);
        assert_eq!(legacy.get(0, 0), source.get(0, 0));
    }
}
#[test]
fn empty_sources_skip_all_iterator_destination_and_entry_work() {
    for source in [
        RasterLayer::new(1_000_000, 1_000_000, Rgba([1, 2, 3, 255])),
        source(15),
    ] {
        let (_, account) = funded(0);
        let trace = RefCell::new(Vec::new());
        let clone = source
            .compact_metadata_clone_retained_precharged_checked(&account, &|| Ok(()), &|n| {
                trace.borrow_mut().push(n);
                Ok(())
            })
            .unwrap();
        assert_eq!(
            source.compact_metadata_clone_retained_work_quote().unwrap(),
            64
        );
        assert_eq!(*trace.borrow(), [64]);
        assert_eq!(clone.resident_tiles(), 0);
        assert_eq!(clone.tiles.retained_quote().unwrap().allocation_upper, 0);
        assert_eq!(clone.default, source.default);
    }
}
#[test]
fn exact_original_counter_prefixes_refuse_without_reset_or_source_change() {
    let source = source(usize::MAX);
    for (ceiling, paid, success) in [
        (63, 0, false),
        (208, 64, false),
        (209, 209, false),
        (328, 209, false),
        (329, 209, false),
        (330, 330, false),
        (445, 330, false),
        (446, 446, false),
        (484, 446, false),
        (485, 446, false),
        (486, 486, true),
    ] {
        let (budget, account) = funded(116);
        let used = Cell::new(0u64);
        let result =
            source.compact_metadata_clone_retained_precharged_checked(&account, &|| Ok(()), &|n| {
                let next = used.get().checked_add(n).ok_or_else(limit)?;
                if next > ceiling {
                    return Err(limit());
                }
                used.set(next);
                Ok(())
            });
        assert_eq!(used.get(), paid);
        assert_eq!(result.is_ok(), success);
        if let Err(e) = &result {
            assert_eq!(e.code, ErrorCode::ResourceLimit);
        }
        assert_eq!(source.get(0, 0), Rgba([7, 8, 9, 0]));
        assert_eq!(source.resident_tiles(), 1);
        drop(result);
        drop(account);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn entry_cancel_drops_fresh_index_but_keeps_paid_work_and_original_storage() {
    let source = source(usize::MAX);
    let (budget, account) = funded(116);
    let checks = Cell::new(0);
    let paid = Cell::new(0);
    let error = Error::new(ErrorCode::OperationCancelled, "retained seed stop").at(19);
    let result = source.compact_metadata_clone_retained_precharged_checked(
        &account,
        &|| {
            checks.set(checks.get() + 1);
            if checks.get() == 2 {
                Err(error.clone())
            } else {
                Ok(())
            }
        },
        &|n| {
            paid.set(paid.get() + n);
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err(), error);
    assert_eq!(paid.get(), 446);
    assert_eq!(checks.get(), 2);
    assert_eq!(source.tiles.values().next().map(Arc::strong_count), Some(1));
    assert_eq!(budget.usage().reserved, 116);
    drop(account);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn denied_declaration_pays_scalar_and_first_cancel_adds_no_new_debt() {
    let source = source(usize::MAX);
    let (_, account) = funded(115);
    let paid = Cell::new(0);
    let checks = Cell::new(0);
    let result = source.compact_metadata_clone_retained_precharged_checked(
        &account,
        &|| {
            checks.set(checks.get() + 1);
            Ok(())
        },
        &|n| {
            paid.set(paid.get() + n);
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(paid.get(), 64);
    assert_eq!(checks.get(), 1);
    let (_, account) = funded(116);
    let result = source.compact_metadata_clone_retained_precharged_checked(
        &account,
        &|| Err(Error::new(ErrorCode::OperationCancelled, "first")),
        &|n| {
            paid.set(paid.get() + n);
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    assert_eq!(paid.get(), 64);
}

#[test]
fn two_entry_partial_cleanup_is_prepaid_once_under_original_account() {
    let mut source = source(usize::MAX);
    let tile = Arc::clone(source.tiles.values().next().unwrap());
    source.tiles.try_reserve(1).unwrap();
    source
        .tiles
        .insert_reserved(TileCoord { tx: 1, ty: 0 }, Arc::clone(&tile))
        .unwrap();
    drop(tile);
    assert_eq!(
        source.compact_metadata_clone_retained_work_quote().unwrap(),
        356
    );
    let (budget, account) = funded(116);
    let checks = Cell::new(0);
    let paid = Cell::new(0);
    let error = Error::new(ErrorCode::OperationCancelled, "partial second entry").at(19);
    let result = source.compact_metadata_clone_retained_precharged_checked(
        &account,
        &|| {
            checks.set(checks.get() + 1);
            if checks.get() == 3 {
                Err(error.clone())
            } else {
                Ok(())
            }
        },
        &|n| {
            paid.set(paid.get() + n);
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err(), error);
    assert_eq!(paid.get(), 512);
    assert_eq!(checks.get(), 3);
    assert_eq!(source.tiles.values().next().map(Arc::strong_count), Some(2));
    assert_eq!(budget.usage().reserved, 116);
    drop(account);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn scalar_refusal_wins_before_undersized_charge_layout_comparison() {
    let source = source(usize::MAX);
    let (_, account) = funded(115);
    let calls = Cell::new(0);
    let spent = Cell::new(0);
    let error = Error::new(ErrorCode::ResourceLimit, "scalar before layout").at(37);
    let result = source.compact_metadata_clone_retained_precharged_checked(
        &account,
        &|| {
            calls.set(calls.get() + 1);
            Ok(())
        },
        &|n| {
            assert_eq!(n, 64);
            spent.set(spent.get() + 1);
            Err(error.clone())
        },
    );
    assert_eq!(result.unwrap_err(), error);
    assert_eq!(calls.get(), 1);
    assert_eq!(spent.get(), 1);
    assert_eq!(source.resident_tiles(), 1);
    let (_, account) = funded(116);
    let paid = Cell::new(0);
    let cancel = Error::new(ErrorCode::OperationCancelled, "initial raw check").at(39);
    let result = source.compact_metadata_clone_retained_precharged_checked(
        &account,
        &|| Err(cancel.clone()),
        &|n| {
            paid.set(paid.get() + n);
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err(), cancel);
    assert_eq!(paid.get(), 0);
}
