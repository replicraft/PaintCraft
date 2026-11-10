use super::*;
use paintcraft_core::types::{
    LayerBlendMode, Rgba,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_raster::{Admission, BlendMode};
use std::cell::{Cell, RefCell};

fn scene() -> Document {
    let mut doc = paintcraft_document::new_document(
        768,
        1,
        Some(Rgba::TRANSPARENT),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.blend_mode = LayerBlendMode::Multiply;
    let mut admission = Admission::new(1 << 20);
    for x in [0, 256, 512] {
        layer
            .raster
            .blend_pixel(
                x,
                0,
                Rgba([20, 60, 100, 255]),
                BlendMode::Replace,
                &mut admission,
            )
            .unwrap();
    }
    // A retained tile can become all-default without leaving the resident map.
    layer
        .raster
        .blend_pixel(
            512,
            0,
            Rgba::TRANSPARENT,
            BlendMode::Replace,
            &mut admission,
        )
        .unwrap();
    assert_eq!(layer.raster.resident_tiles(), 3);
    doc
}

#[test]
fn immutable_resident_upper_bounds_cover_actual_encoded_records_not_table_capacity() {
    let mut doc = scene();
    // Shared storage still produces separate records for hidden layer intent.
    let mut hidden = doc.layers[&doc.roots[0]].clone();
    hidden.id = uuid::Uuid::from_u128(55);
    hidden.visible = false;
    hidden.opacity = 0;
    doc.roots.push(hidden.id);
    doc.layers.insert(hidden.id, hidden);
    let mut empty = doc.layers[&doc.roots[0]].clone();
    empty.id = uuid::Uuid::from_u128(56);
    empty.raster = paintcraft_raster::RasterLayer::new(768, 1, Rgba::TRANSPARENT);
    doc.roots.push(empty.id);
    doc.layers.insert(empty.id, empty);
    let budget = SharedBudget::new(8 << 20);
    let scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let check = || Ok(());
    let spend = |_| Ok(());
    let fact = count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
    let live = scope.footprint().live;
    let bounds = fact.plan_record_upper_bounds_checked().unwrap();
    assert_eq!(
        bounds,
        BlendPlanRecordUpperBounds {
            layers: 3,
            tiles: 6
        }
    );
    assert_eq!(scope.footprint().live, live);
    let encoded = fact.encode().unwrap();
    let checked = crate::pcnd::CheckedContainer::verify(encoded.as_bytes(), &check).unwrap();
    let plan = checked
        .portable_source8_plan_checked(Default::default(), &scope, &check, &spend)
        .unwrap();
    assert_eq!(plan.facts().layers, bounds.layers);
    assert_eq!(plan.facts().tiles, 4);
    assert!(plan.facts().tiles < bounds.tiles);
    drop(plan);
    drop(encoded);
    drop(fact);
    assert_eq!(scope.footprint().live, 0);
    drop(scope);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn query_storage_exact_and_one_short_use_only_the_original_scope() {
    for short in [0, 1] {
        let doc = scene();
        let budget = SharedBudget::new(8 << 20);
        let scope = budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let check = || Ok(());
        let spend = |_| Ok(());
        let fact =
            count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
        let mut held = scope.account();
        held.resize((4 << 20) - scope.footprint().live - QUERY_CONTROL + short)
            .unwrap();
        let live = scope.footprint().live;
        let result = fact.plan_record_upper_bounds_checked();
        if short == 0 {
            assert_eq!(result.unwrap().tiles, 3);
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.code, ErrorCode::ResourceLimit);
            assert!(error.allocation_denial().is_some());
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
fn original_callback_error_moves_unchanged_and_retry_keeps_paid_work() {
    let doc = scene();
    let budget = SharedBudget::new(8 << 20);
    let scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let pending = RefCell::new(None);
    let used = Cell::new(0u64);
    let check = || match pending.borrow_mut().take() {
        Some(error) => Err(error),
        None => Ok(()),
    };
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    let fact = count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
    let live = scope.footprint().live;
    let sentinel = Error::new(
        ErrorCode::OperationCancelled,
        "original sizing callback".repeat(41),
    )
    .at(23);
    let pointer = sentinel.message.as_ptr();
    *pending.borrow_mut() = Some(sentinel);
    let before = used.get();
    let error = fact.plan_record_upper_bounds_checked().unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(error.message.as_ptr(), pointer);
    assert_eq!(error.operation_index, Some(23));
    assert!(used.get() > before);
    assert_eq!(scope.footprint().live, live);
    let paid = used.get();
    assert_eq!(fact.plan_record_upper_bounds_checked().unwrap().tiles, 3);
    assert!(used.get() > paid);
    assert_eq!(scope.footprint().live, live);
}

#[test]
fn work_refusal_precedes_lookup_and_preserves_the_original_paid_prefix() {
    let doc = scene();
    let budget = SharedBudget::new(8 << 20);
    let scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
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
            .ok_or_else(|| limit("original sizing work"))?;
        used.set(next);
        Ok(())
    };
    let fact = count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
    let before = used.get();
    let callbacks = checks.get();
    let live = scope.footprint().live;
    ceiling.set(before + 64);
    assert_eq!(
        fact.plan_record_upper_bounds_checked().unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(used.get(), before + 64);
    assert_eq!(checks.get(), callbacks);
    assert_eq!(scope.footprint().live, live);
    assert_eq!(
        fact.plan_record_upper_bounds_checked().unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(used.get(), before + 64);
}
