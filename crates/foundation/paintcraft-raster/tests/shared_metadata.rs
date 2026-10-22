use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_raster::{Admission, BlendMode, PaintPlan, RasterLayer};
fn rect(x: i64) -> Rect {
    Rect::new(x, 0, 1, 1).unwrap()
}
fn plan(budget: &SharedBudget) -> PaintPlan {
    let mut p = PaintPlan::with_metadata_budget(4096, budget.clone());
    p.rect(rect(0), Rgba([17, 88, 156, 128]), BlendMode::SourceOver)
        .unwrap();
    p
}
#[test]
fn clones_pin_one_descriptor_allocation_and_writes_copy_with_a_new_charge() {
    let budget = SharedBudget::new(10000);
    let mut original = plan(&budget);
    let bytes = original.descriptor_bytes() as u64;
    let snapshot = original.clone();
    assert_eq!(budget.usage().reserved, bytes);
    original
        .rect(rect(1), Rgba([17, 88, 156, 128]), BlendMode::SourceOver)
        .unwrap();
    assert_eq!(
        budget.usage().reserved,
        bytes + original.descriptor_bytes() as u64
    );
    let mut a = RasterLayer::new(2, 1, Rgba::TRANSPARENT);
    let mut b = a.clone();
    let mut admission = Admission::new(2 * 65_536).with_paint_metadata_budget(budget.clone());
    original.apply(&mut a, &mut admission).unwrap();
    snapshot.apply(&mut b, &mut admission).unwrap();
    assert_eq!(a.get(1, 0), Rgba([17, 88, 156, 128]));
    assert_eq!(b.get(1, 0), Rgba::TRANSPARENT);
    drop(original);
    assert_eq!(budget.usage().reserved, bytes);
    drop(snapshot);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn aggregate_projection_pressure_releases_failed_indices_and_recovery_keeps_descriptors() {
    let budget = SharedBudget::new(512);
    let bounds = Rect::new(0, 0, 8, 8).unwrap();
    let first = plan(&budget)
        .into_projection_checked(bounds, &|| Ok(()))
        .unwrap();
    let held = first.metadata_bytes() as u64;
    assert_eq!(budget.usage().reserved, held);
    let error = plan(&budget)
        .into_projection_checked(bounds, &|| Ok(()))
        .unwrap_err();
    assert_eq!(
        error.allocation_denial().unwrap().class(),
        AllocationClass::PaintMetadata
    );
    assert_eq!(
        budget.usage().reserved,
        held,
        "failed construction must free every private charge"
    );
    let recovered = first.into_plan();
    assert_eq!(budget.usage().reserved, recovered.descriptor_bytes() as u64);
    drop(recovered);
    assert_eq!(budget.usage().reserved, 0);
    let retry = plan(&budget)
        .into_projection_checked(bounds, &|| Ok(()))
        .unwrap();
    drop(retry);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn cancelled_projection_and_append_discard_owned_charges_but_keep_shared_snapshots() {
    let budget = SharedBudget::new(10000);
    let bounds = Rect::new(0, 0, 300, 1).unwrap();
    let p = plan(&budget);
    let descriptor = p.descriptor_bytes() as u64;
    let error = p
        .into_projection_checked(bounds, &|| {
            if budget.usage().reserved > descriptor {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel index"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(budget.usage().reserved, 0);
    let p = plan(&budget);
    let snapshot = p.clone();
    let projection = p.into_projection_checked(bounds, &|| Ok(())).unwrap();
    let incoming = plan(&budget);
    let error = projection
        .append_checked(incoming, &|| {
            Err(Error::new(ErrorCode::OperationCancelled, "cancel append"))
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(budget.usage().reserved, snapshot.descriptor_bytes() as u64);
    drop(snapshot);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn cross_realm_append_retains_foreign_snapshots_without_double_charging_storage() {
    let a = SharedBudget::new(10000);
    let b = SharedBudget::new(10000);
    let bounds = Rect::new(0, 0, 8, 8).unwrap();
    let p = plan(&a);
    let old = p.clone();
    let program = p.into_projection_checked(bounds, &|| Ok(())).unwrap();
    let incoming = plan(&b);
    let foreign = incoming.clone();
    let combined = program.append_checked(incoming, &|| Ok(())).unwrap();
    assert_eq!(
        a.usage().reserved,
        combined.metadata_bytes() as u64 + old.descriptor_bytes() as u64
    );
    assert_eq!(b.usage().reserved, foreign.descriptor_bytes() as u64);
    drop(combined);
    assert_eq!(a.usage().reserved, old.descriptor_bytes() as u64);
    drop(old);
    drop(foreign);
    assert_eq!(a.usage().reserved, 0);
    assert_eq!(b.usage().reserved, 0);
}
#[test]
fn cloned_admissions_build_independent_plans_in_the_same_metadata_realm() {
    let metadata = SharedBudget::new(10000);
    let admission = Admission::new(65_536).with_paint_metadata_budget(metadata.clone());
    let copy = admission.clone();
    let mut a = admission.paint_plan();
    let mut b = copy.paint_plan();
    a.rect(rect(0), Rgba([1; 4]), BlendMode::Replace).unwrap();
    b.rect(rect(0), Rgba([2; 4]), BlendMode::Replace).unwrap();
    assert_eq!(
        metadata.usage().reserved,
        (a.descriptor_bytes() + b.descriptor_bytes()) as u64
    );
    drop(a);
    drop(b);
    assert_eq!(metadata.usage().reserved, 0);
}

#[test]
fn failed_copy_on_write_keeps_both_shared_plan_values_and_the_original_charge() {
    let probe = plan(&SharedBudget::new(10000));
    let bytes = probe.descriptor_bytes() as u64;
    drop(probe);
    let budget = SharedBudget::new(bytes);
    let mut original = plan(&budget);
    let snapshot = original.clone();
    let error = original
        .rect(rect(1), Rgba([17, 88, 156, 128]), BlendMode::SourceOver)
        .unwrap_err();
    assert_eq!(
        error.allocation_denial().unwrap().class(),
        AllocationClass::PaintMetadata
    );
    assert_eq!(budget.usage().reserved, bytes);
    for p in [&original, &snapshot] {
        let mut layer = RasterLayer::new(2, 1, Rgba::TRANSPARENT);
        p.apply(&mut layer, &mut Admission::new(65_536)).unwrap();
        assert_eq!(layer.get(0, 0), Rgba([17, 88, 156, 128]));
        assert_eq!(layer.get(1, 0), Rgba::TRANSPARENT);
    }
    drop(original);
    drop(snapshot);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn cancellation_during_descriptor_copy_releases_the_private_copy_and_program_index() {
    use std::cell::Cell;
    let budget = SharedBudget::new(1 << 20);
    let mut p = PaintPlan::with_metadata_budget(1 << 20, budget.clone());
    for _ in 0..4096 {
        p.rect(rect(0), Rgba([17, 88, 156, 128]), BlendMode::SourceOver)
            .unwrap();
    }
    let snapshot = p.clone();
    let projection = p
        .into_projection_checked(Rect::new(0, 0, 8, 8).unwrap(), &|| Ok(()))
        .unwrap();
    let incoming = plan(&budget);
    let calls = Cell::new(0);
    let error = projection
        .append_checked(incoming, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel descriptor copy",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(calls.get(), 3);
    assert_eq!(budget.usage().reserved, snapshot.descriptor_bytes() as u64);
    drop(snapshot);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
#[cfg(target_pointer_width = "64")]
fn copying_a_clamped_program_preserves_capacity_instead_of_exhausting_the_local_quota() {
    let budget = SharedBudget::new(10000);
    let bounds = Rect::new(0, 0, 8, 8).unwrap();
    let mut program = PaintPlan::with_metadata_budget(800, budget.clone())
        .into_projection_checked(bounds, &|| Ok(()))
        .unwrap();
    for _ in 0..10 {
        program = program.append_checked(plan(&budget), &|| Ok(())).unwrap();
    }
    let p = program.into_plan();
    let bytes = p.descriptor_bytes();
    let snapshot = p.clone();
    let program = p.into_projection_checked(bounds, &|| Ok(())).unwrap();
    let mut small = PaintPlan::with_metadata_budget(40, budget.clone());
    small
        .rect(rect(0), Rgba([1; 4]), BlendMode::Replace)
        .unwrap();
    let combined = program.append_checked(small, &|| Ok(())).unwrap();
    assert!(combined.metadata_bytes() <= 800);
    assert_eq!(snapshot.descriptor_bytes(), bytes);
    drop(combined);
    drop(snapshot);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn admission_denial_includes_descriptors_and_pending_index_growth_in_its_attempt_demand() {
    let probe = plan(&SharedBudget::new(10000))
        .into_projection_checked(Rect::new(0, 0, 8, 8).unwrap(), &|| Ok(()))
        .unwrap();
    let required = probe.metadata_bytes() as u64;
    drop(probe);
    let metadata = SharedBudget::new(512);
    let blocker = metadata.reserve(512 - required + 12).unwrap();
    let mut admission = Admission::new(65_536).with_paint_metadata_budget(metadata.clone());
    let mut p = admission.paint_plan();
    p.rect(rect(0), Rgba([1; 4]), BlendMode::Replace).unwrap();
    let mut raster = RasterLayer::new(8, 8, Rgba::TRANSPARENT);
    let error = p.apply(&mut raster, &mut admission).unwrap_err();
    let denial = error.allocation_denial().unwrap();
    assert_eq!(denial.class(), AllocationClass::PaintMetadata);
    assert_eq!(denial.required_bytes(), required);
    assert!(!denial.can_retry());
    assert_eq!(raster.resident_tiles(), 0);
    drop(p);
    assert!(
        !denial.can_retry(),
        "headroom below the known attempt cannot trigger a retry"
    );
    drop(blocker);
    assert!(denial.can_retry());
    assert_eq!(metadata.usage().reserved, 0);
}
