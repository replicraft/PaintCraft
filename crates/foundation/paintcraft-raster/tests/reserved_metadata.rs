use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_raster::{Admission, BlendMode, PaintPlan, RasterLayer};

fn paint(admission: &Admission) -> PaintPlan {
    let mut plan = admission.paint_plan();
    for x in [0, 256] {
        plan.rect(
            Rect::new(x, 0, 1, 1).unwrap(),
            Rgba([7, 8, 9, 255]),
            BlendMode::Replace,
        )
        .unwrap();
    }
    plan
}
fn footprint() -> u64 {
    let metadata = SharedBudget::new(10000);
    let mut admission = Admission::new(2 * 65_536).with_paint_metadata_budget(metadata.clone());
    paint(&admission)
        .apply(
            &mut RasterLayer::new(257, 1, Rgba::TRANSPARENT),
            &mut admission,
        )
        .unwrap();
    assert_eq!(metadata.usage().reserved, 0);
    metadata.usage().peak
}

#[test]
fn reserved_descriptors_and_replay_index_succeed_when_every_global_byte_is_occupied() {
    let required = footprint();
    let metadata = SharedBudget::new(required + 16);
    let reservation = metadata.reserve_capacity(required).unwrap();
    let competitor = metadata.reserve(16).unwrap();
    let mut admission = Admission::new(2 * 65_536).with_reserved_paint_metadata(reservation);
    let plan = paint(&admission);
    let snapshot = plan.clone();
    let mut raster = RasterLayer::new(257, 1, Rgba::TRANSPARENT);
    plan.apply(&mut raster, &mut admission).unwrap();
    assert_eq!(raster.get(0, 0), Rgba([7, 8, 9, 255]));
    assert_eq!(raster.get(256, 0), Rgba([7, 8, 9, 255]));
    assert_eq!(raster.get(255, 0), Rgba::TRANSPARENT);
    assert_eq!(metadata.usage().reserved, required + 16);
    drop(admission);
    assert_eq!(
        metadata.usage().reserved,
        plan.descriptor_bytes() as u64 + 16
    );
    drop(plan);
    assert_eq!(
        metadata.usage().reserved,
        snapshot.descriptor_bytes() as u64 + 16
    );
    drop(snapshot);
    drop(competitor);
    assert_eq!(metadata.usage().reserved, 0);
}

#[test]
fn insufficient_reserved_metadata_never_borrows_free_global_capacity_or_allocates_tiles() {
    let required = footprint();
    let metadata = SharedBudget::new(10000);
    let mut admission = Admission::new(2 * 65_536)
        .with_reserved_paint_metadata(metadata.reserve_capacity(required - 1).unwrap());
    let plan = paint(&admission);
    let mut raster = RasterLayer::new(257, 1, Rgba::TRANSPARENT);
    let error = plan.apply(&mut raster, &mut admission).unwrap_err();
    let denial = error.allocation_denial().unwrap();
    assert_eq!(denial.class(), AllocationClass::PaintMetadata);
    assert_eq!(denial.required_bytes(), required);
    assert_eq!(raster.resident_tiles(), 0);
    assert_eq!(metadata.usage().reserved, required - 1);
    drop(admission);
    assert_eq!(metadata.usage().reserved, plan.descriptor_bytes() as u64);
    drop(plan);
    assert_eq!(metadata.usage().reserved, 0);
    assert!(denial.can_retry());
}

#[test]
fn retained_projection_keeps_only_owned_payload_after_factory_drop_and_cancellation_recycles() {
    let required = footprint();
    let metadata = SharedBudget::new(required);
    let admission = Admission::new(0)
        .with_reserved_paint_metadata(metadata.reserve_capacity(required).unwrap());
    let projection = paint(&admission)
        .into_projection_checked(Rect::new(0, 0, 257, 1).unwrap(), &|| Ok(()))
        .unwrap();
    let owned = projection.metadata_bytes() as u64;
    drop(admission);
    assert_eq!(metadata.usage().reserved, owned);
    let plan = projection.into_plan();
    assert_eq!(metadata.usage().reserved, plan.descriptor_bytes() as u64);
    drop(plan);
    assert_eq!(metadata.usage().reserved, 0);

    let admission = Admission::new(0)
        .with_reserved_paint_metadata(metadata.reserve_capacity(required).unwrap());
    let plan = paint(&admission);
    let error = plan
        .into_projection_checked(Rect::new(0, 0, 257, 1).unwrap(), &|| {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel projection",
            ))
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(metadata.usage().reserved, required);
    // Cancelled accounts returned their bytes to this still-live reservation.
    let projection = paint(&admission)
        .into_projection_checked(Rect::new(0, 0, 257, 1).unwrap(), &|| Ok(()))
        .unwrap();
    drop(projection);
    drop(admission);
    assert_eq!(metadata.usage().reserved, 0);
}
