use super::*;
use paintcraft_core::types::{LayerBlendMode, Rgba, clipboard::Image, resources::AllocationClass};
use paintcraft_document::POLICY;
use paintcraft_raster::{Admission, RgbaPastePlan};
use std::cell::{Cell, RefCell};
fn raster(n: i64) -> RasterLayer {
    let mut adm = Admission::new(65536);
    let source = RasterLayer::from_rgba_checked(
        2,
        1,
        Rgba::TRANSPARENT,
        &[1, 2, 3, 255, 4, 5, 6, 255],
        &mut adm,
        &|| Ok(()),
    )
    .unwrap();
    let lease = source.tile_leases().next().unwrap().1;
    let mut raster = RasterLayer::new((n.max(1) as u32) * 256, 1, Rgba::TRANSPARENT);
    for tx in 0..n {
        raster
            .install_tile(TileCoord { tx, ty: 0 }, lease.clone())
            .unwrap();
    }
    raster
}
fn removed(clear: u32) -> RasterLayer {
    let source = raster(15);
    let image = Image::from_fn(
        clear * 256,
        1,
        &SharedBudget::new(4 << 20),
        &|| Ok(()),
        |_, _| Ok(Rgba::TRANSPARENT),
    )
    .unwrap();
    let metadata = SharedBudget::new(4 << 20);
    let admission = Admission::new(65536).with_paint_metadata_budget(metadata);
    let plan = RgbaPastePlan::prepare(
        &source,
        image,
        (0, 0),
        paintcraft_core::types::policy::BlendMode::Replace,
        None,
        &admission,
        &|| Ok(()),
    )
    .unwrap();
    plan.replay(&mut Admission::new(65536), &|| Ok(())).unwrap()
}
fn quote(a: u64, b: usize, n: usize) -> RetainedTileTableQuote {
    RetainedTileTableQuote {
        allocation_upper: a,
        bucket_upper: b,
        live_entries: n,
        scan_work_upper: a + b as u64,
    }
}
#[test]
fn independently_frozen_nonempty_math_and_zero_fast_path_prices() {
    assert_eq!(size_of::<(TileCoord, TileLease)>(), 24);
    assert_eq!(SCALAR_WORK, 64);
    for (q, extra) in [
        (quote(0, 0, 0), 0),
        (quote(816, 32, 0), 0),
        (quote(116, 4, 1), 146),
        (quote(816, 32, 1), 874),
        (quote(116, 4, 2), 388),
    ] {
        assert_eq!(remaining(q).unwrap(), extra);
    }
    assert_eq!(
        remaining(RetainedTileTableQuote {
            allocation_upper: u64::MAX,
            bucket_upper: 1,
            live_entries: usize::MAX,
            scan_work_upper: u64::MAX
        })
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
}
type CollectedBlocks = Vec<(u32, u32, Vec<u8>)>;
fn collect_trace(raster: &RasterLayer, rooted: bool) -> (Vec<u64>, CollectedBlocks) {
    let budget = SharedBudget::new(8 << 20);
    let reservation = budget.reserve_capacity(4 << 20).unwrap();
    let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
    let trace = RefCell::new(Vec::new());
    let mut blocks = Vec::new();
    let spend = |n| {
        trace.borrow_mut().push(n);
        Ok(())
    };
    let mut emit = |x, y, data: &[u8]| {
        blocks.push((x, y, data.to_vec()));
        Ok(())
    };
    if rooted {
        collect(raster, &scope, &|| Ok(()), &spend, &mut emit)
    } else {
        super::super::blocks(raster, &scope, &|| Ok(()), &spend, &mut emit)
    }
    .unwrap();
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(reservation.available(), 4 << 20);
    drop(scope);
    drop(reservation);
    assert_eq!(budget.usage().reserved, 0);
    (trace.into_inner(), blocks)
}
#[test]
fn exact_fresh_sparse_and_sort_debits_precede_unchanged_old_helper_trace() {
    let one = raster(1);
    let sparse = removed(14);
    let two = raster(2);
    assert_eq!(
        sparse.retained_tile_table_quote().unwrap(),
        quote(816, 32, 1)
    );
    for (source, extra) in [(&one, 146), (&sparse, 874), (&two, 388)] {
        let before = source.clone();
        let (old, expected) = collect_trace(source, false);
        let (new, actual) = collect_trace(source, true);
        assert_eq!(&new[..2], &[64, extra]);
        assert_eq!(&new[2..], old);
        assert_eq!(actual, expected);
        assert_eq!(source, &before);
        for (_, _, data) in actual {
            assert_eq!(&data[..8], &[1, 2, 3, 255, 4, 5, 6, 255]);
        }
    }
}
#[test]
fn zero_entries_never_create_collector_storage_or_invoke_emitter() {
    for source in [raster(0), removed(15)] {
        let budget = SharedBudget::new(0);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let trace = RefCell::new(Vec::new());
        let checks = Cell::new(0);
        collect(
            &source,
            &scope,
            &|| {
                checks.set(checks.get() + 1);
                Ok(())
            },
            &|n| {
                trace.borrow_mut().push(n);
                Ok(())
            },
            &mut |_, _, _| panic!("empty emitter"),
        )
        .unwrap();
        assert_eq!(&*trace.borrow(), &[64]);
        assert_eq!(checks.get(), 0);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(scope.footprint().peak, 0);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn original_counter_refuses_math_or_whole_scan_before_unpaid_iterator_work() {
    let source = raster(1);
    let before = source.clone();
    for (ceiling, paid, peak) in [(63, 0, 0), (209, 64, 0), (210, 210, 24)] {
        let budget = SharedBudget::new(8 << 20);
        let reservation = budget.reserve_capacity(4 << 20).unwrap();
        let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
        let used = Cell::new(0u64);
        let emitted = Cell::new(0);
        let error = collect(
            &source,
            &scope,
            &|| Ok(()),
            &|n| {
                let next = used
                    .get()
                    .checked_add(n)
                    .filter(|v| *v <= ceiling)
                    .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "original counter"))?;
                used.set(next);
                Ok(())
            },
            &mut |_, _, _| {
                emitted.set(emitted.get() + 1);
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(used.get(), paid);
        assert_eq!(emitted.get(), 0);
        assert_eq!(source, before);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(scope.footprint().peak, peak);
        assert_eq!(reservation.available(), 4 << 20);
        drop(scope);
        drop(reservation);
        assert_eq!(budget.usage().reserved, 0);
    }
}
fn doc(blend: bool) -> Document {
    let mut d =
        paintcraft_document::new_document(256, 1, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    d.layers.get_mut(&d.roots[0]).unwrap().raster = raster(1);
    if blend {
        d.layers.get_mut(&d.roots[0]).unwrap().blend_mode = LayerBlendMode::Multiply;
    }
    d
}
#[test]
fn two_rooted_call_sites_are_explicitly_repriced_and_ordinary_wire_contract_stays() {
    for blend in [false, true] {
        let source = doc(blend);
        let before = source.clone();
        let budget = SharedBudget::new(8 << 20);
        let scope = budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let mut bytes = Vec::new();
        let work = Cell::new(0u64);
        let seen = Cell::new(false);
        let previous = Cell::new(0u64);
        let spend = |n| {
            work.set(work.get() + n);
            if previous.get() == 64 && n == 146 {
                seen.set(true);
            }
            previous.set(n);
            Ok(())
        };
        if blend {
            write_portable_blend_snapshot_admitted_checked(
                &source,
                &mut bytes,
                &scope,
                &|| Ok(()),
                &spend,
            )
        } else {
            write_portable_normal_snapshot_rooted_admitted_checked(
                &source,
                &mut bytes,
                &scope,
                &|| Ok(()),
                &spend,
            )
        }
        .unwrap();
        assert!(seen.get());
        assert!(work.get() >= 210);
        assert!(source.same_semantics(&before));
        if !blend {
            let mut old = Vec::new();
            write_portable_snapshot_admitted_checked(
                &source,
                &mut old,
                &scope,
                &|| Ok(()),
                &|_| Ok(()),
            )
            .unwrap();
            assert_eq!(bytes, old);
        } else {
            assert_eq!(bytes[4], 8);
        }
        assert_eq!(scope.footprint().live, 0);
    }
}
#[test]
fn counted_complete_passes_keep_work_initialization_and_both_supplements_distinct() {
    let source = doc(false);
    let budget = SharedBudget::new(8 << 20);
    let scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let trace = RefCell::new(Vec::new());
    let used = Cell::new(0u64);
    let check = || Ok(());
    let spend = |n| {
        used.set(used.get() + n);
        trace.borrow_mut().push(n);
        Ok(())
    };
    let fact =
        count_portable_normal_snapshot_rooted_checked(&source, &scope, &check, &spend).unwrap();
    let first = trace.borrow().clone();
    let first_work = used.get();
    let length = fact.wire_len();
    assert!(first.windows(2).any(|w| w == [64, 146]));
    trace.borrow_mut().clear();
    let bytes = fact.encode().unwrap();
    let second = trace.borrow();
    assert_eq!(second[0], length);
    assert_eq!(&second[1..], first);
    assert_eq!(used.get(), 2 * first_work + length);
    drop(second);
    drop(bytes);
    drop(fact);
    assert_eq!(scope.footprint().live, 0);
}
