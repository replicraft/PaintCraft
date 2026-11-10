use super::*;
use paintcraft_core::types::{
    json,
    resources::{AllocationClass, BudgetScope, SharedBudget},
};
use paintcraft_raster::{Admission, ReservedDecodeAdmission, ReservedDecodeFactories};
use std::cell::Cell;
fn target(native: u64, wasm: u64) -> u64 {
    if usize::BITS == 64 { native } else { wasm }
}
fn fixture() -> Vec<u8> {
    let rows = json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/retained-source8-layout-fixtures/oracle.json"
    )))
    .unwrap();
    let row = rows
        .get("cases")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("name").unwrap().as_str() == Some("baseline"))
        .unwrap();
    row.get("hex")
        .unwrap()
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
        .collect()
}
fn scope(budget: &SharedBudget, n: u64) -> BudgetScope {
    budget
        .reserve_capacity(n)
        .unwrap()
        .scope(AllocationClass::PaintMetadata)
}
fn debit(used: &Cell<u64>, n: u64) -> Result<()> {
    used.set(
        used.get()
            .checked_add(n)
            .filter(|n| *n <= 268435456)
            .ok_or_else(bound)?,
    );
    Ok(())
}
fn decoder(budget: &SharedBudget) -> (ReservedDecodeAdmission, BudgetScope) {
    let controls = scope(budget, 4096);
    let metadata = scope(budget, 4096);
    let factories = ReservedDecodeFactories {
        selection_budget: budget.clone(),
        coverage_budget: budget.clone(),
        paint_metadata_scope: metadata,
    };
    let q = budget.reserve_capacity(262144).unwrap();
    (
        Admission::with_reserved_decode_factories(65536, q, factories, &controls, &|| Ok(()))
            .unwrap(),
        controls,
    )
}
#[test]
fn independent_defining_targets_and_existing_plan_quote_abi_are_exact() {
    let q = portable_source8_materializer_control_bytes().unwrap();
    assert_eq!(
        q,
        PortableSource8MaterializerControlBytes {
            coverage: target(224, 164),
            document_stream: target(1552, 1380),
            pixels: target(408, 300),
            retained_layout: target(1040, 936)
        }
    );
    assert_eq!(size_of::<PortableSource8MaterializerControlBytes>(), 32);
    assert!(size_of::<Result<PortableSource8MaterializerControlBytes>>() <= 128);
    let plan = portable_source8_control_bytes();
    assert_eq!(
        (plan.plan, plan.scan, plan.layer_record, plan.block_record),
        (target(2656, 1528), target(9416, 7260), 56, 24)
    );
}
#[test]
fn quote_has_no_plan_callback_work_activity_or_original_admission_effect() {
    let bytes = fixture();
    let original = bytes.clone();
    let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(32 << 20);
    let ps = scope(&budget, 4 << 20);
    let used = Cell::new(0u64);
    let calls = Cell::new(0u64);
    let spend = |n| debit(&used, n);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let plan = checked
        .portable_source8_plan_checked(Default::default(), &ps, &check, &spend)
        .unwrap();
    let before = (
        used.get(),
        calls.get(),
        plan.work_used(),
        plan.check_calls(),
        budget.usage(),
        budget.activity(),
        ps.footprint(),
        ps.capacity_snapshot(),
    );
    for _ in 0..2 {
        let _facts = portable_source8_materializer_control_bytes().unwrap();
        assert_eq!(
            (
                used.get(),
                calls.get(),
                plan.work_used(),
                plan.check_calls(),
                budget.usage(),
                budget.activity(),
                ps.footprint(),
                ps.capacity_snapshot()
            ),
            before
        );
    }
    assert_eq!(bytes, original);
    drop(plan);
    assert_eq!(ps.footprint().live, 0);
}
#[test]
fn actual_coverage_constructor_uses_literal_exact_original_a_and_one_short() {
    for (capacity, ok) in [(target(223, 163), false), (target(224, 164), true)] {
        let bytes = fixture();
        let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(32 << 20);
        let ps = scope(&budget, 4 << 20);
        let a = scope(&budget, capacity);
        let ms = scope(&budget, 4096);
        let m = ms.account();
        let used = Cell::new(0);
        let calls = Cell::new(0);
        let spend = |n| debit(&used, n);
        let check = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        let plan = checked
            .portable_source8_plan_checked(Default::default(), &ps, &check, &spend)
            .unwrap();
        let before = (used.get(), calls.get());
        let result = plan.coverage_materializer(&a, &m);
        if ok {
            let owner = result.unwrap();
            assert_eq!(a.footprint().live, capacity);
            assert_eq!(
                a.footprint().live,
                portable_source8_materializer_control_bytes()
                    .unwrap()
                    .coverage
            );
            assert_eq!((used.get() - before.0, calls.get() - before.1), (592, 1));
            drop(owner);
        } else {
            assert_eq!(result.err().unwrap().code, ErrorCode::ResourceLimit);
            assert_eq!((used.get() - before.0, calls.get() - before.1), (64, 0));
        }
        assert_eq!(a.footprint().live, 0);
        assert_eq!(a.capacity_snapshot().ceiling, capacity);
    }
}
#[test]
fn actual_document_stream_shares_a_with_live_coverage_wrapper_at_exact_boundary() {
    for (capacity, ok) in [(target(1775, 1543), false), (target(1776, 1544), true)] {
        let bytes = fixture();
        let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(32 << 20);
        let ps = scope(&budget, 4 << 20);
        let a = scope(&budget, capacity);
        let ms = scope(&budget, 4096);
        let m = ms.account();
        let used = Cell::new(0);
        let calls = Cell::new(0);
        let spend = |n| debit(&used, n);
        let check = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        let plan = checked
            .portable_source8_plan_checked(Default::default(), &ps, &check, &spend)
            .unwrap();
        let owner = plan.coverage_materializer(&a, &m).unwrap();
        let before = (used.get(), calls.get());
        let result = owner.document_intentions_checked();
        if ok {
            let stream = result.unwrap();
            assert_eq!(a.footprint().live, capacity);
            let q = portable_source8_materializer_control_bytes().unwrap();
            assert_eq!(capacity, q.coverage + q.document_stream);
            assert_eq!(
                (used.get() - before.0, calls.get() - before.1),
                (target(696, 672), 1)
            );
            drop(stream);
        } else {
            assert_eq!(result.err().unwrap().code, ErrorCode::ResourceLimit);
            assert_eq!((used.get() - before.0, calls.get() - before.1), (64, 0));
        }
        assert_eq!(a.footprint().live, target(224, 164));
        drop(owner);
        assert_eq!(a.footprint().live, 0);
    }
}
#[test]
fn actual_pixel_wrapper_uses_literal_original_c_without_consuming_q() {
    for (capacity, ok) in [(target(407, 299), false), (target(408, 300), true)] {
        let bytes = fixture();
        let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(32 << 20);
        let ps = scope(&budget, 4 << 20);
        let c = scope(&budget, capacity);
        let ms = scope(&budget, 4096);
        let m = ms.account();
        let (mut decoder, _controls) = decoder(&budget);
        let used = Cell::new(0);
        let calls = Cell::new(0);
        let spend = |n| debit(&used, n);
        let check = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        let plan = checked
            .portable_source8_plan_checked(Default::default(), &ps, &check, &spend)
            .unwrap();
        let before = (used.get(), calls.get());
        let result = plan.pixel_materializer(&mut decoder, &c, &m);
        if ok {
            let owner = result.unwrap();
            assert_eq!(c.footprint().live, capacity);
            assert_eq!(
                capacity,
                portable_source8_materializer_control_bytes()
                    .unwrap()
                    .pixels
            );
            assert_eq!((used.get() - before.0, calls.get() - before.1), (592, 1));
            drop(owner);
        } else {
            assert_eq!(result.err().unwrap().code, ErrorCode::ResourceLimit);
            assert_eq!((used.get() - before.0, calls.get() - before.1), (64, 0));
        }
        assert_eq!(c.footprint().live, 0);
        assert_eq!(decoder.remaining_pixels(), 65536);
        assert_eq!(decoder.reservation_capacity(), 262144);
    }
}
#[test]
fn actual_retained_census_needs_existing_header_helper_overlap_in_p() {
    for (capacity, ok) in [(target(13167, 9779), false), (target(13168, 9780), true)] {
        let bytes = fixture();
        let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(32 << 20);
        let p = scope(&budget, capacity);
        let used = Cell::new(0);
        let spend = |n| debit(&used, n);
        let check = || Ok(());
        let plan = checked
            .portable_source8_plan_checked(Default::default(), &p, &check, &spend)
            .unwrap();
        let live = p.footprint().live;
        assert_eq!(live, target(2712, 1584));
        let before = used.get();
        let result = plan.retained_source_layout_checked();
        if ok {
            let recipe = result.unwrap();
            let q = portable_source8_materializer_control_bytes().unwrap();
            assert_eq!(p.footprint().live - live, q.retained_layout);
            assert_eq!(p.footprint().peak, capacity);
            assert_eq!(used.get() - before, 13188);
            drop(recipe);
        } else {
            assert_eq!(result.err().unwrap().code, ErrorCode::ResourceLimit);
            assert_eq!(used.get() - before, 760);
        }
        assert_eq!(p.footprint().live, live);
        drop(plan);
        assert_eq!(p.footprint().live, 0);
    }
}
