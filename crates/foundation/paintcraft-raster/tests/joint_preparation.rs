//! Real original-scope payload lifetimes, without observed-usage-derived grants.
use paintcraft_core::types::{
    ErrorCode, Rect, Rgba,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_raster::{Admission, BlendMode, RasterLayer, TilePlanner, flood};
use std::mem::size_of;

const TILE_BYTES: u64 = RasterLayer::TILE_BYTES as u64;
fn area(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn classify(
    scope: paintcraft_core::types::resources::BudgetScope,
    limit: usize,
) -> paintcraft_core::types::Result<flood::AdmittedSpans> {
    flood::region_from_source_masked_admitted_checked(
        |_, _| Ok(Rgba([7, 9, 11, 0])),
        |_, _| Ok(true),
        (0, 0),
        &area(0, 0, 1, 1),
        0,
        (limit, scope),
        &|| Ok(()),
    )
}
#[test]
fn flood_same_reserved_account_moves_to_output_and_overlaps_descriptors() {
    // One bitmap word, first frontier growth four pairs, first output growth
    // four spans. These are defining capacities, not a measured grant.
    let output = 4 * size_of::<paintcraft_raster::shape::Span>();
    let peak = size_of::<u64>() + 4 * size_of::<(i64, i64)>() + output;
    let budget = SharedBudget::new(peak as u64);
    let reservation = budget.reserve_capacity(peak as u64).unwrap();
    let scope = reservation.scope(AllocationClass::PaintMetadata);
    let spans = classify(scope.clone(), peak).unwrap();
    assert_eq!(spans.as_slice().len(), 1);
    assert_eq!(scope.footprint().live, output as u64);
    assert_eq!(scope.footprint().peak, peak as u64);
    // The output still owns the original factory after the incoming copy retires.
    drop(scope);
    assert_eq!(budget.usage().reserved, peak as u64);
    drop(spans);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn held_flood_output_competes_in_original_scope_and_fixed_policy_stays_terminal() {
    let output = 4 * size_of::<paintcraft_raster::shape::Span>();
    let peak = size_of::<u64>() + 4 * size_of::<(i64, i64)>() + output;
    let budget = SharedBudget::new(peak as u64);
    let scope = budget
        .reserve_capacity(peak as u64)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let first = classify(scope.clone(), peak).unwrap();
    let error = classify(scope.clone(), peak).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(
        error.allocation_denial().unwrap().class(),
        AllocationClass::PaintMetadata
    );
    assert_eq!(scope.footprint().live, output as u64);
    drop(first);
    drop(classify(scope.clone(), peak).unwrap());
    let minimum =
        size_of::<u64>() + size_of::<(i64, i64)>() + size_of::<paintcraft_raster::shape::Span>();
    let error = classify(scope.clone(), minimum - 1).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(error.allocation_denial().is_none());
    assert_eq!(scope.footprint().live, 0);
    drop(scope);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn prepared_paint_reuses_its_actual_index_under_complete_metadata_pressure() {
    let metadata = SharedBudget::new(1 << 20);
    let decoded = SharedBudget::new(TILE_BYTES);
    let mut admission = Admission::with_residency(65_536, decoded.clone())
        .with_paint_metadata_budget(metadata.clone());
    let mut layer = RasterLayer::new(2, 1, Rgba([7, 9, 11, 0]));
    let mut plan = admission.paint_plan();
    plan.rect(
        area(0, 0, 2, 1),
        Rgba([20, 60, 100, 128]),
        BlendMode::SourceOver,
    )
    .unwrap();
    let planner = TilePlanner::new(&admission).unwrap();
    let mut model = planner.layer(&layer, &|| Ok(())).unwrap();
    let prepared = model.prepare_checked(&plan, 0, &|| Ok(())).unwrap();
    assert!(!prepared.is_empty());
    drop(model);
    drop(planner);
    drop(plan);
    // Pressure only: fill remaining budget after production already selected its
    // immutable 1MiB ceiling. This value does NOT size any admitted owner/grant.
    let blocker = metadata
        .reserve((1 << 20) - metadata.usage().reserved)
        .unwrap();
    admission.reserve_tiles(1).unwrap();
    prepared
        .replay_checked(&mut layer, &mut admission, &|| Ok(()))
        .unwrap();
    assert_eq!(layer.get(0, 0), Rgba([20, 60, 100, 128]));
    assert_eq!(layer.get(1, 0), Rgba([20, 60, 100, 128]));
    drop(blocker);
    drop(admission);
    drop(layer);
    assert_eq!(metadata.usage().reserved, 0);
    assert_eq!(decoded.usage().reserved, 0);
}
#[test]
fn prepared_paint_rejects_changed_missing_tile_classification_before_writes() {
    let metadata = SharedBudget::new(1 << 20);
    let decoded = SharedBudget::new(4 * TILE_BYTES);
    let mut admission = Admission::with_residency(4 * 65_536, decoded.clone())
        .with_paint_metadata_budget(metadata.clone());
    let original = RasterLayer::new(512, 1, Rgba([7, 9, 11, 0]));
    let mut plan = admission.paint_plan();
    plan.rect(area(0, 0, 1, 1), Rgba([7, 9, 11, 0]), BlendMode::Replace)
        .unwrap();
    plan.rect(area(256, 0, 1, 1), Rgba([255; 4]), BlendMode::Replace)
        .unwrap();
    let planner = TilePlanner::new(&admission).unwrap();
    let mut model = planner.layer(&original, &|| Ok(())).unwrap();
    let prepared = model.prepare_checked(&plan, 0, &|| Ok(())).unwrap();
    let mut changed = original.clone();
    changed
        .blend_pixel(0, 0, Rgba([1, 2, 3, 4]), BlendMode::Replace, &mut admission)
        .unwrap();
    let before = changed.get(0, 0);
    let reserved = decoded.usage().reserved;
    let error = prepared
        .replay_checked(&mut changed, &mut admission, &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvariantViolation);
    assert_eq!(changed.get(0, 0), before);
    assert_eq!(changed.get(256, 0), Rgba([7, 9, 11, 0]));
    assert_eq!(decoded.usage().reserved, reserved);
}
#[test]
fn admitted_stroke_failure_clears_all_rows_before_returning_an_owner() {
    let metadata = SharedBudget::new(1);
    let scope = metadata.scope(AllocationClass::PaintMetadata);
    let mut set =
        paintcraft_raster::shape::CheckedSpanSet::with_metadata_scope(1 << 16, scope.clone());
    let error = set
        .add_stamp(0, 0, 1, false, &area(0, 0, 1, 1), &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(metadata.usage().reserved, 0);
    let mut emitted = 0;
    set.visit_spans(&|| Ok(()), |_| {
        emitted += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(emitted, 0);
}

#[test]
fn admitted_stroke_emit_error_retires_rows_and_heap_in_original_scope() {
    let metadata = SharedBudget::new(1 << 20);
    let scope = metadata.scope(AllocationClass::PaintMetadata);
    let mut set =
        paintcraft_raster::shape::CheckedSpanSet::with_metadata_scope(1 << 16, scope.clone());
    for x in (0..1100).rev() {
        set.add_stamp(x * 2, 0, 1, false, &area(0, 0, 4096, 1), &|| Ok(()))
            .unwrap();
    }
    assert!(scope.footprint().live > 0);
    let retained_rows = scope.footprint().live;
    let heap_bytes = 2 * size_of::<std::cmp::Reverse<(i64, i64, usize, usize)>>();
    let mut callbacks = 0;
    let error = set
        .visit_spans(&|| Ok(()), |_| {
            callbacks += 1;
            assert!(scope.footprint().live > 0);
            assert_eq!(scope.footprint().live, retained_rows + heap_bytes as u64);
            Err(paintcraft_core::types::Error::new(
                ErrorCode::OperationCancelled,
                "original emitter cancellation",
            ))
        })
        .unwrap_err();
    assert_eq!(callbacks, 1);
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(error.message, "original emitter cancellation");
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(metadata.usage().reserved, 0);
}

#[test]
fn huge_empty_default_prepared_noop_keeps_constant_work_and_hidden_rgb() {
    use std::cell::Cell;
    let metadata = SharedBudget::new(1 << 20);
    let mut admission =
        Admission::with_residency(0, SharedBudget::new(0)).with_paint_metadata_budget(metadata);
    let mut layer = RasterLayer::new(1_048_576, 1_048_576, Rgba([7, 9, 11, 0]));
    let mut plan = admission.paint_plan();
    plan.rect(
        area(0, 0, 1_048_576, 1_048_576),
        Rgba([7, 9, 11, 0]),
        BlendMode::Replace,
    )
    .unwrap();
    let planner = TilePlanner::new(&admission).unwrap();
    let mut model = planner.layer(&layer, &|| Ok(())).unwrap();
    let prepared = model.prepare_checked(&plan, 0, &|| Ok(())).unwrap();
    assert!(prepared.is_empty());
    let calls = Cell::new(0);
    prepared
        .replay_checked(&mut layer, &mut admission, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    assert!(
        calls.get() <= 8,
        "no tile walk for huge default-only empty source"
    );
    assert_eq!(layer.get(1_048_575, 1_048_575), Rgba([7, 9, 11, 0]));
}
#[test]
fn default_only_index_from_resident_model_rejects_blank_target() {
    let metadata = SharedBudget::new(1 << 20);
    let mut admission = Admission::with_residency(2 * 65_536, SharedBudget::new(2 * TILE_BYTES))
        .with_paint_metadata_budget(metadata);
    let mut source = RasterLayer::new(2, 1, Rgba([7, 9, 11, 0]));
    source
        .blend_pixel(0, 0, Rgba([255; 4]), BlendMode::Replace, &mut admission)
        .unwrap();
    let mut plan = admission.paint_plan();
    plan.rect(area(0, 0, 2, 1), Rgba([7, 9, 11, 0]), BlendMode::Replace)
        .unwrap();
    let planner = TilePlanner::new(&admission).unwrap();
    let mut model = planner.layer(&source, &|| Ok(())).unwrap();
    let prepared = model.prepare_checked(&plan, 0, &|| Ok(())).unwrap();
    assert!(!prepared.is_empty());
    let mut blank = RasterLayer::new(2, 1, Rgba([7, 9, 11, 0]));
    let error = prepared
        .replay_checked(&mut blank, &mut admission, &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvariantViolation);
    assert_eq!(blank.get(0, 0), Rgba([7, 9, 11, 0]));
}

#[test]
fn exact_program_reuses_without_temporary_index_and_different_factory_refuses() {
    let metadata = SharedBudget::new(1 << 20);
    let original = Admission::with_residency(2 * 65_536, SharedBudget::new(2 * TILE_BYTES))
        .with_paint_metadata_budget(metadata.clone());
    let other = Admission::with_residency(2 * 65_536, SharedBudget::new(2 * TILE_BYTES))
        .with_paint_metadata_budget(metadata.clone());
    assert!(
        !original
            .paint_metadata_scope()
            .same_factory(&other.paint_metadata_scope())
    );
    let layer = RasterLayer::new(2, 1, Rgba([7, 9, 11, 0]));
    let mut plan = original.paint_plan();
    plan.rect(
        area(0, 0, 2, 1),
        Rgba([17, 88, 156, 19]),
        BlendMode::SourceOver,
    )
    .unwrap();
    let planner = TilePlanner::new(&original).unwrap();
    let other_planner = TilePlanner::new(&other).unwrap();
    let mut model = planner.layer(&layer, &|| Ok(())).unwrap();
    let mut other_model = other_planner.layer(&layer, &|| Ok(())).unwrap();
    let prepared = model.prepare_checked(&plan, 0, &|| Ok(())).unwrap();
    let before = original.paint_metadata_scope().footprint().live;
    // Fill unused original-policy capacity solely as test interference, not grant sizing.
    let blocker = metadata
        .reserve((1 << 20) - metadata.usage().reserved)
        .unwrap();
    let same = model
        .prepare_reusing_checked(&plan, 1, Some(&prepared), &|| Ok(()))
        .unwrap();
    assert_eq!(original.paint_metadata_scope().footprint().live, before);
    let error = other_model
        .prepare_reusing_checked(&plan, 1, Some(&prepared), &|| Ok(()))
        .unwrap_err();
    assert_eq!(
        error.allocation_denial().unwrap().class(),
        AllocationClass::PaintMetadata
    );
    drop(same);
    drop(blocker);
    drop(prepared);
    drop(model);
    drop(other_model);
    drop(planner);
    drop(other_planner);
    drop(plan);
    drop(original);
    drop(other);
    assert_eq!(metadata.usage().reserved, 0);
}
#[test]
fn unique_program_control_is_admitted_before_formation_and_checked_after_it() {
    let metadata = SharedBudget::new(1 << 20);
    let admission = Admission::with_residency(0, SharedBudget::new(0))
        .with_paint_metadata_budget(metadata.clone());
    let scope = admission.paint_metadata_scope();
    let layer = RasterLayer::new(2, 1, Rgba([7, 9, 11, 0]));
    let mut plan = admission.paint_plan();
    plan.rect(area(0, 0, 2, 1), Rgba([7, 9, 11, 0]), BlendMode::Replace)
        .unwrap();
    let planner = TilePlanner::new(&admission).unwrap();
    let mut model = planner.layer(&layer, &|| Ok(())).unwrap();
    let baseline = scope.footprint().live;
    // Empty/default indexing needs no map/vector payload. Only the new owning
    // program control can ask for metadata beyond this already-selected policy.
    let blocker = metadata
        .reserve((1 << 20) - metadata.usage().reserved)
        .unwrap();
    let error = model.prepare_checked(&plan, 0, &|| Ok(())).unwrap_err();
    assert_eq!(
        error.allocation_denial().unwrap().class(),
        AllocationClass::PaintMetadata
    );
    assert_eq!(scope.footprint().live, baseline);
    drop(blocker);
    let error = model
        .prepare_checked(&plan, 0, &|| {
            if scope.footprint().live > baseline {
                Err(paintcraft_core::types::Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel after unique program formation",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(error.message, "cancel after unique program formation");
    assert_eq!(scope.footprint().live, baseline);
    drop(model);
    drop(planner);
    drop(plan);
    drop(scope);
    drop(admission);
    assert_eq!(metadata.usage().reserved, 0);
}

#[test]
fn equal_writes_cannot_reuse_a_program_past_tighter_incoming_plan_limit() {
    let metadata = SharedBudget::new(1 << 20);
    let admission = Admission::with_residency(2 * 65_536, SharedBudget::new(2 * TILE_BYTES))
        .with_paint_metadata_budget(metadata.clone());
    let layer = RasterLayer::new(2, 1, Rgba([7, 9, 11, 0]));
    let mut plan = admission.paint_plan();
    plan.rect(
        area(0, 0, 2, 1),
        Rgba([17, 88, 156, 19]),
        BlendMode::SourceOver,
    )
    .unwrap();
    let planner = TilePlanner::new(&admission).unwrap();
    let mut model = planner.layer(&layer, &|| Ok(())).unwrap();
    let prepared = model.prepare_checked(&plan, 0, &|| Ok(())).unwrap();
    // This is the actual descriptor owner size, a fixed per-plan POLICY only;
    // no budget/root/reservation is created from runtime usage or availability.
    let mut tight =
        paintcraft_raster::PaintPlan::with_metadata_budget(plan.descriptor_bytes(), metadata);
    tight
        .rect(
            area(0, 0, 2, 1),
            Rgba([17, 88, 156, 19]),
            BlendMode::SourceOver,
        )
        .unwrap();
    let reused = model
        .prepare_reusing_checked(&tight, 1, Some(&prepared), &|| Ok(()))
        .unwrap_err();
    let fresh = model.prepare_checked(&tight, 1, &|| Ok(())).unwrap_err();
    assert_eq!(reused.code, ErrorCode::ResourceLimit);
    assert_eq!(fresh.code, ErrorCode::ResourceLimit);
    assert!(reused.allocation_denial().is_none());
    assert!(fresh.allocation_denial().is_none());
}

#[test]
fn legacy_bool_empty_zero_plan_limit_needs_no_owned_program_and_keeps_final_check() {
    use std::cell::Cell;
    let metadata = SharedBudget::new(1 << 20);
    let decoded = SharedBudget::new(0);
    let admission =
        Admission::with_residency(0, decoded.clone()).with_paint_metadata_budget(metadata.clone());
    let scope = admission.paint_metadata_scope();
    let layer = RasterLayer::new(2, 1, Rgba([7, 9, 11, 0]));
    let planner = TilePlanner::new(&admission).unwrap();
    let mut model = planner.layer(&layer, &|| Ok(())).unwrap();
    let before = scope.footprint();
    let empty = paintcraft_raster::PaintPlan::with_metadata_limit(0);
    let calls = Cell::new(0);
    let result = model
        .apply_checked(&empty, 0, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    assert!(!result);
    // Defining old path: index entry check, index exit check, model exit check.
    assert_eq!(calls.get(), 3);
    assert_eq!(scope.footprint(), before);
    assert_eq!(decoded.usage().reserved, 0);
    assert_eq!(decoded.usage().peak, 0);
    calls.set(0);
    let error = model
        .apply_checked(&empty, 0, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                Err(paintcraft_core::types::Error::new(
                    ErrorCode::OperationCancelled,
                    "original final bool checkpoint",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(error.message, "original final bool checkpoint");
    assert_eq!(scope.footprint(), before);
    drop(model);
    drop(planner);
    drop(admission);
    drop(scope);
    assert_eq!(metadata.usage().reserved, 0);
}
