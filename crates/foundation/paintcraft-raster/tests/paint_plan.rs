use paintcraft_core::types::{Rect, Rgba, resources::SharedBudget};
use paintcraft_raster::{Admission, BlendMode, PaintPlan, RasterLayer};
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
#[test]
fn ordered_clipped_writes_match_incremental_scalar_storage_and_exact_replacement_coverage() {
    let mut base = RasterLayer::new(520, 8, Rgba([37, 19, 83, 0]));
    base.blend_pixel(
        255,
        2,
        Rgba([17, 88, 156, 255]),
        BlendMode::Replace,
        &mut Admission::new(65_536),
    )
    .unwrap();
    let mut planned = base.clone();
    let mut scalar = base.clone();
    let mut plan = PaintPlan::default();
    let writes = [
        (
            rect(-8, -2, 280, 7),
            Rgba([220, 40, 90, 128]),
            BlendMode::SourceOver,
        ),
        (rect(250, 1, 270, 6), Rgba([1, 2, 3, 0]), BlendMode::Replace),
        (
            rect(255, 2, 260, 4),
            Rgba([0, 0, 255, 73]),
            BlendMode::SourceOver,
        ),
    ];
    for (area, color, mode) in writes {
        plan.rect(area, color, mode).unwrap();
        scalar
            .fill_rect(&area, color, mode, &mut Admission::new(8 * 65_536))
            .unwrap();
    }
    let area = rect(254, 2, 6, 3);
    let ground = Rgba([251, 139, 37, 255]);
    plan.coverage_rect(area, ground, 7).unwrap();
    for y in area.y0..area.y1 {
        for x in area.x0..area.x1 {
            let dst = scalar.get(x, y);
            let pixel = Rgba(std::array::from_fn(|i| {
                ((u32::from(ground.0[i]) * 7 + u32::from(dst.0[i]) * 9 + 8) / 16) as u8
            }));
            scalar
                .blend_pixel(x, y, pixel, BlendMode::Replace, &mut Admission::new(65_536))
                .unwrap();
        }
    }
    plan.apply(&mut planned, &mut Admission::new(8 * 65_536))
        .unwrap();
    for y in 0..8 {
        for x in 0..520 {
            assert_eq!(planned.get(x, y), scalar.get(x, y), "{x},{y}");
        }
    }
    assert_eq!(base.get(255, 2), Rgba([17, 88, 156, 255]));
    assert_eq!(base.get(256, 2), Rgba([37, 19, 83, 0]));
}
#[test]
fn quota_denial_precedes_the_entire_tile_prefix_and_success_releases_unused_operation_capacity() {
    let budget = SharedBudget::new(2 * BYTES);
    let blocker = budget.reserve(BYTES).unwrap();
    let mut layer = RasterLayer::new(300, 1, Rgba([255; 4]));
    let mut admission = Admission::with_residency(2 * 65_536, budget.clone());
    let mut plan = PaintPlan::default();
    plan.rect(rect(0, 0, 300, 1), Rgba([0, 0, 0, 255]), BlendMode::Replace)
        .unwrap();
    let error = plan.apply(&mut layer, &mut admission).unwrap_err();
    assert_eq!(
        error.allocation_denial().unwrap().required_bytes(),
        2 * BYTES
    );
    assert_eq!(layer.resident_tiles(), 0);
    assert_eq!(budget.usage().peak, BYTES);
    assert_eq!(layer.get(0, 0), Rgba([255; 4]));
    drop(blocker);
    plan.apply(&mut layer, &mut admission).unwrap();
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    for x in 0..300 {
        assert_eq!(layer.get(x, 0), Rgba([0, 0, 0, 255]));
    }
    drop(layer);
    assert_eq!(budget.usage().reserved, 0); // The parent admission pins no per-operation reservation.
}
#[test]
fn weak_only_tiles_transfer_without_headroom_but_real_readers_require_cow_admission() {
    let budget = SharedBudget::new(BYTES);
    let mut admission = Admission::with_residency(4 * 65_536, budget.clone());
    let mut layer = RasterLayer::new(8, 8, Rgba::TRANSPARENT);
    layer
        .blend_pixel(
            1,
            1,
            Rgba([0, 0, 0, 255]),
            BlendMode::Replace,
            &mut admission,
        )
        .unwrap();
    let weak = layer
        .tile_versions(layer.bounds())
        .unwrap()
        .next()
        .unwrap()
        .1;
    let mut plan = PaintPlan::default();
    plan.rect(rect(1, 1, 1, 1), Rgba([255, 0, 0, 255]), BlendMode::Replace)
        .unwrap();
    plan.apply(&mut layer, &mut admission).unwrap();
    assert!(weak.try_lease().is_none());
    assert_eq!(budget.usage().peak, BYTES);
    let reader = layer.tile_leases().next().unwrap().1;
    let mut plan = PaintPlan::default();
    plan.rect(rect(1, 1, 1, 1), Rgba([0, 255, 0, 255]), BlendMode::Replace)
        .unwrap();
    assert!(plan.apply(&mut layer, &mut admission).is_err());
    assert_eq!(layer.get(1, 1), Rgba([255, 0, 0, 255]));
    drop(reader);
    plan.apply(&mut layer, &mut admission).unwrap();
    assert_eq!(layer.get(1, 1), Rgba([0, 255, 0, 255]));
    assert_eq!(budget.usage().reserved, BYTES);
}
#[test]
fn sequential_operation_denial_keeps_the_whole_attempt_footprint_after_rollback() {
    let budget = SharedBudget::new(2 * BYTES);
    let blocker = budget.reserve(BYTES).unwrap();
    let mut layer = RasterLayer::new(300, 1, Rgba::TRANSPARENT);
    let mut admission = Admission::with_residency(2 * 65_536, budget.clone());
    let mut first = PaintPlan::default();
    first
        .rect(rect(0, 0, 1, 1), Rgba([0, 0, 0, 255]), BlendMode::Replace)
        .unwrap();
    first.apply(&mut layer, &mut admission).unwrap();
    let mut second = PaintPlan::default();
    second
        .rect(rect(256, 0, 1, 1), Rgba([0, 0, 0, 255]), BlendMode::Replace)
        .unwrap();
    let error = second.apply(&mut layer, &mut admission).unwrap_err();
    assert_eq!(
        error.allocation_denial().unwrap().required_bytes(),
        2 * BYTES
    );
    assert_eq!(layer.get(256, 0), Rgba::TRANSPARENT);
    drop(layer);
    assert!(!error.allocation_denial().unwrap().can_retry());
    drop(blocker);
    assert!(error.allocation_denial().unwrap().can_retry());
}

#[test]
fn oversized_sparse_plans_stop_at_the_staged_limit_without_reserving_payload() {
    let budget = SharedBudget::new(2 * BYTES);
    let mut admission = Admission::with_residency(65_536, budget.clone());
    let mut layer = RasterLayer::new(1_048_576, 1_048_576, Rgba::TRANSPARENT);
    let mut plan = PaintPlan::default();
    plan.rect(layer.bounds(), Rgba([1, 2, 3, 255]), BlendMode::Replace)
        .unwrap();
    let error = plan.apply(&mut layer, &mut admission).unwrap_err();
    assert!(error.allocation_denial().is_none());
    assert_eq!(layer.resident_tiles(), 0);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(budget.activity(), 0);
}
