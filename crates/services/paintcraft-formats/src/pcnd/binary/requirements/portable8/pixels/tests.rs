use super::super::tests::fixture;
use super::*;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
use paintcraft_raster::{Admission, ReservedDecodeFactories};
use std::cell::Cell;
const T: u64 = 262144;
fn owner_bytes() -> u64 {
    if cfg!(target_pointer_width = "64") {
        408
    } else {
        300
    }
}
fn full_m() -> u64 {
    if cfg!(target_pointer_width = "64") {
        356
    } else {
        308
    }
}
fn admission() -> (ReservedDecodeAdmission, SharedBudget) {
    let q = SharedBudget::new(4 * T);
    let controls = SharedBudget::new(4096).scope(AllocationClass::PaintMetadata);
    let factories = ReservedDecodeFactories {
        selection_budget: SharedBudget::new(4096),
        coverage_budget: SharedBudget::new(4096),
        paint_metadata_scope: controls.clone(),
    };
    (
        Admission::with_reserved_decode_factories(
            2 * 65536,
            q.reserve_capacity(2 * T).unwrap(),
            factories,
            &controls,
            &|| Ok(()),
        )
        .unwrap(),
        q,
    )
}
#[test]
fn actual_same_plan_four_portable_codecs_have_independent_method_work_pixels_and_census() {
    assert_eq!(
        size_of::<PortablePixelMaterializer8<'static, 'static>>(),
        if cfg!(target_pointer_width = "64") {
            72
        } else {
            40
        }
    );
    for (name, native, wasm, checks, scratch) in [
        ("manual-raw", 419099, 419067, 284, 0),
        ("manual-runs", 420785, 420753, 287, 8),
        ("manual-lz4", 420820, 420788, 287, 8),
        ("manual-runs-lz4", 422536, 422504, 290, 14),
    ] {
        let bytes = fixture(name);
        let original = bytes.clone();
        let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(32 << 20);
        let ps = budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let sc = budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let mut m = budget.account(AllocationClass::PaintMetadata);
        m.resize(full_m()).unwrap();
        let (mut a, q) = admission();
        let used = Cell::new(0u64);
        let calls = Cell::new(0u64);
        let spend = |n| {
            used.set(
                used.get()
                    .checked_add(n)
                    .filter(|v| *v <= 268435456)
                    .ok_or_else(bound)?,
            );
            Ok(())
        };
        let check = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        let plan = checked
            .portable_source8_plan_checked(Default::default(), &ps, &check, &spend)
            .unwrap();
        let before = used.get();
        let before_check = calls.get();
        let mut owner = plan.pixel_materializer(&mut a, &sc, &m).unwrap();
        assert_eq!(used.get() - before, 592);
        assert_eq!(calls.get() - before_check, 1);
        assert_eq!(sc.footprint().live, owner_bytes());
        let before = used.get();
        let before_check = calls.get();
        let raster = owner.layer(0).unwrap();
        assert_eq!(
            used.get() - before,
            if cfg!(target_pointer_width = "64") {
                native
            } else {
                wasm
            }
        );
        assert_eq!(calls.get() - before_check, checks);
        assert_eq!(raster.get(0, 0), Rgba([7, 8, 9, 255]));
        assert_eq!(raster.get(1, 0), Rgba([7, 8, 9, 255]));
        assert_eq!(raster.resident_tiles(), 1);
        assert_eq!(sc.footprint().live, owner_bytes());
        assert_eq!(sc.footprint().peak, owner_bytes() + 4096 + scratch);
        assert_eq!(plan.work_used(), used.get());
        assert_eq!(plan.check_calls(), calls.get());
        drop(raster);
        drop(owner);
        assert_eq!(sc.footprint().live, 0);
        drop(plan);
        assert_eq!(ps.footprint().live, 0);
        drop(a);
        assert_eq!(q.usage().reserved, 0);
        assert_eq!(bytes, original);
    }
}
#[test]
fn original_bound_owner_scope_exact_minus_one_and_actual_header_union_are_distinct() {
    for short in [true, false] {
        let bytes = fixture("manual-raw");
        let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
        let b = SharedBudget::new(32 << 20);
        let ps = b.scope(AllocationClass::PaintMetadata);
        let sc = b
            .reserve_capacity(owner_bytes() - u64::from(short))
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let mut m = b.account(AllocationClass::PaintMetadata);
        m.resize(full_m()).unwrap();
        let (mut a, _q) = admission();
        let used = Cell::new(0u64);
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        let check = || Ok(());
        let plan = checked
            .portable_source8_plan_checked(Default::default(), &ps, &check, &spend)
            .unwrap();
        let before = used.get();
        let result = plan.pixel_materializer(&mut a, &sc, &m);
        if short {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
            assert_eq!(used.get() - before, 64);
        } else {
            drop(result.unwrap());
            assert_eq!(used.get() - before, 592);
        }
        assert_eq!(sc.footprint().live, 0);
    }
    let bytes = fixture("manual-raw");
    let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    let b = SharedBudget::new(32 << 20);
    let ps = b.scope(AllocationClass::PaintMetadata);
    let sc = b
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let mut m = b.account(AllocationClass::PaintMetadata);
    m.resize(full_m() - 1).unwrap();
    let (mut a, _q) = admission();
    let used = Cell::new(0u64);
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    let check = || Ok(());
    let plan = checked
        .portable_source8_plan_checked(Default::default(), &ps, &check, &spend)
        .unwrap();
    let mut owner = plan.pixel_materializer(&mut a, &sc, &m).unwrap();
    let before = used.get();
    assert_eq!(owner.layer(0).unwrap_err().code, ErrorCode::ResourceLimit);
    // headerL5 replay6747 +scalar544 +Rasterconstructor1300 +blockgetter/map1072 +common576
    assert_eq!(used.get() - before, 10239);
    assert_eq!(sc.footprint().live, owner_bytes());
    drop(owner);
    assert_eq!(a.remaining_pixels(), 2 * 65536);
}
#[test]
fn same_plan_typed_retry_keeps_original_counter_and_scope_and_code3_refuses() {
    let bytes = fixture("manual-raw");
    let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    let original = bytes.clone();
    let b = SharedBudget::new(32 << 20);
    let ps = b.scope(AllocationClass::PaintMetadata);
    let sc = b
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let mut m = b.account(AllocationClass::PaintMetadata);
    m.resize(full_m()).unwrap();
    let (mut a, _q) = admission();
    let used = Cell::new(0u64);
    let fail = Cell::new(false);
    let calls = Cell::new(0u64);
    let sentinel = Error::new(
        ErrorCode::OperationCancelled,
        "original bound pixels callback",
    )
    .at(82);
    let check = || {
        calls.set(calls.get() + 1);
        if fail.get() {
            Err(sentinel.clone())
        } else {
            Ok(())
        }
    };
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    let plan = checked
        .portable_source8_plan_checked(Default::default(), &ps, &check, &spend)
        .unwrap();
    let mut owner = plan.pixel_materializer(&mut a, &sc, &m).unwrap();
    let before = used.get();
    fail.set(true);
    assert_eq!(owner.layer(0).unwrap_err(), sentinel);
    assert_eq!(used.get() - before, 528);
    assert_eq!(sc.footprint().live, owner_bytes());
    fail.set(false);
    let paid = used.get();
    let raster = owner.layer(0).unwrap();
    assert_eq!(
        used.get() - paid,
        if cfg!(target_pointer_width = "64") {
            419099
        } else {
            419067
        }
    );
    assert_eq!(plan.work_used(), used.get());
    assert_eq!(plan.check_calls(), calls.get());
    drop(raster);
    assert_eq!(owner.layer(99).unwrap_err().code, ErrorCode::CodecFailed);
    drop(owner);
    drop(plan);
    assert_eq!(bytes, original);
    let bytes = fixture("frozen-code3");
    let checked = CheckedContainer::verify(&bytes, &|| Ok(())).unwrap();
    assert_eq!(
        checked
            .portable_source8_plan_checked(Default::default(), &ps, &check, &spend)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(sc.footprint().live, 0);
}
