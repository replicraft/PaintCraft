use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_raster::{Admission, BlendMode, PaintPlan, RasterLayer, TilePlanner};
use std::cell::Cell;
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
fn paint(admission: &Admission, x: i64) -> PaintPlan {
    let mut plan = admission.paint_plan();
    plan.rect(
        Rect::new(x, 0, 1, 1).unwrap(),
        Rgba([255; 4]),
        BlendMode::Replace,
    )
    .unwrap();
    plan
}
fn admission(pixels: u64) -> Admission {
    Admission::with_residency(pixels, SharedBudget::new(0))
        .with_paint_metadata_budget(SharedBudget::new(1 << 20))
}

#[test]
fn duplication_cow_and_deletion_match_real_peak_while_cumulative_work_remains_distinct() {
    let adm = admission(3 * 65_536);
    let planner = TilePlanner::new(&adm).unwrap();
    let mut real = RasterLayer::new(257, 1, Rgba::TRANSPARENT);
    let mut model = planner.layer(&real, &|| Ok(())).unwrap();
    let decoded = SharedBudget::new(8 * BYTES);
    let mut actual = Admission::with_residency(3 * 65_536, decoded.clone());
    let first = paint(&adm, 0);
    model.apply_checked(&first, 0, &|| Ok(())).unwrap();
    first.apply(&mut real, &mut actual).unwrap();
    let mut copy = real.clone();
    let mut copied = model.duplicate_checked(&|| Ok(())).unwrap();
    let changed = paint(&adm, 1);
    model.apply_checked(&changed, 2, &|| Ok(())).unwrap();
    changed.apply(&mut real, &mut actual).unwrap();
    copied.apply_checked(&changed, 3, &|| Ok(())).unwrap();
    changed.apply(&mut copy, &mut actual).unwrap();
    assert_eq!(planner.footprint().allocated, 2);
    assert_eq!(planner.footprint().live, 2);
    drop(model);
    drop(real);
    assert_eq!(planner.footprint().live, 1);
    let second = paint(&adm, 256);
    copied.apply_checked(&second, 5, &|| Ok(())).unwrap();
    second.apply(&mut copy, &mut actual).unwrap();
    let f = planner.footprint();
    assert_eq!(
        (f.live, f.peak, f.allocated, f.peak_operation),
        (2, 2, 3, 2)
    );
    assert_eq!(decoded.usage().peak, f.peak * BYTES);
    assert_eq!(decoded.usage().reserved, f.live * BYTES);
    drop(copied);
    drop(copy);
    assert_eq!(planner.footprint().live, 0);
    assert_eq!(decoded.usage().reserved, 0);
}

#[test]
fn immutable_base_tiles_require_one_new_version_and_expansion_keeps_edge_tile_ownership() {
    let decoded = SharedBudget::new(4 * BYTES);
    let mut actual = Admission::with_residency(4 * 65_536, decoded.clone());
    let mut base = RasterLayer::new(3, 1, Rgba([13, 29, 47, 0]));
    paint(&actual, 0).apply(&mut base, &mut actual).unwrap();
    let adm = admission(2 * 65_536);
    let planner = TilePlanner::new(&adm).unwrap();
    let mut model = planner.layer(&base, &|| Ok(())).unwrap();
    let mut real = base.clone();
    real.expand_to(257, 1).unwrap();
    model.expand_to(257, 1).unwrap();
    for (op, x) in [0, 3, 256].into_iter().enumerate() {
        let p = paint(&adm, x);
        model.apply_checked(&p, op, &|| Ok(())).unwrap();
        p.apply(&mut real, &mut actual).unwrap();
    }
    assert_eq!(planner.footprint().allocated, 2);
    assert_eq!(planner.footprint().peak, 2);
    assert_eq!(decoded.usage().reserved, 3 * BYTES);
    assert_eq!(base.get(1, 0), Rgba([13, 29, 47, 0]));
}

#[test]
fn uniform_huge_noop_has_no_touched_tiles_or_simulated_allocations() {
    let adm = admission(0);
    let planner = TilePlanner::new(&adm).unwrap();
    let base = RasterLayer::new(1_000_000, 1_000_000, Rgba([13, 29, 47, 0]));
    let mut model = planner.layer(&base, &|| Ok(())).unwrap();
    let mut p = adm.paint_plan();
    p.rect(base.bounds(), base.default_pixel(), BlendMode::Replace)
        .unwrap();
    let calls = Cell::new(0);
    model
        .apply_checked(&p, 0, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    assert!(calls.get() < 10);
    assert_eq!(planner.footprint().allocated, 0);
    assert_eq!(model.resident_tiles(), 0);
}

#[test]
fn cumulative_limit_is_not_replenished_by_deleting_simulated_layers() {
    let adm = admission(2 * 65_536);
    let planner = TilePlanner::new(&adm).unwrap();
    let p = paint(&adm, 0);
    for operation in 0..3 {
        let mut model = planner
            .layer(&RasterLayer::new(1, 1, Rgba::TRANSPARENT), &|| Ok(()))
            .unwrap();
        let result = model.apply_checked(&p, operation, &|| Ok(()));
        if operation < 2 {
            result.unwrap();
        } else {
            let e = result.unwrap_err();
            assert_eq!(e.code, ErrorCode::ResourceLimit);
            assert!(e.allocation_denial().is_none());
        }
        drop(model);
        assert_eq!(planner.footprint().live, 0);
    }
    assert_eq!(
        (planner.footprint().allocated, planner.footprint().peak),
        (2, 1)
    );
    assert_eq!(adm.residency_usage().peak, 0);
}

#[test]
fn metadata_denial_and_every_observed_simulation_checkpoint_release_private_versions() {
    let adm = admission(4 * 65_536);
    let metadata = adm.paint_metadata_budget();
    let p = paint(&adm, 0);
    let run = |stop: usize| {
        let planner = TilePlanner::new(&adm).unwrap();
        let mut model = planner
            .layer(&RasterLayer::new(257, 1, Rgba::TRANSPARENT), &|| Ok(()))
            .unwrap();
        let calls = Cell::new(0);
        let result = model.apply_checked(&p, 0, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel model"))
            } else {
                Ok(())
            }
        });
        drop(model);
        assert_eq!(planner.footprint().live, 0);
        (result, calls.get())
    };
    let (success, total) = run(usize::MAX);
    success.unwrap();
    let held = metadata.usage().reserved;
    for stop in 1..=total {
        assert_eq!(run(stop).0.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(metadata.usage().reserved, held);
    }
    let planner = TilePlanner::new(&adm).unwrap();
    let mut model = planner
        .layer(&RasterLayer::new(257, 1, Rgba::TRANSPARENT), &|| Ok(()))
        .unwrap();
    let blocker = metadata
        .reserve(metadata.usage().limit - metadata.usage().reserved)
        .unwrap();
    let error = model.apply_checked(&p, 0, &|| Ok(())).unwrap_err();
    assert_eq!(
        error.allocation_denial().unwrap().class(),
        AllocationClass::PaintMetadata
    );
    assert_eq!(planner.footprint().allocated, 0);
    drop(model);
    drop(blocker);
    drop(planner);
    assert_eq!(metadata.usage().reserved, held);
}

#[test]
fn version_release_callbacks_can_observe_the_updated_footprint_without_lock_cycles() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let adm = admission(65_536);
    let metadata = adm.paint_metadata_budget();
    let planner = Arc::new(TilePlanner::new(&adm).unwrap());
    let observed = Arc::new(AtomicBool::new(false));
    let recorded = observed.clone();
    let probe = planner.clone();
    let watch = metadata
        .watch(1, move || {
            if probe.footprint().live == 0 {
                recorded.store(true, Ordering::Release);
            }
        })
        .unwrap();
    watch.wait_for_release(true);
    watch.observe();
    let mut model = planner
        .layer(&RasterLayer::new(1, 1, Rgba::TRANSPARENT), &|| Ok(()))
        .unwrap();
    model.apply_checked(&paint(&adm, 0), 0, &|| Ok(())).unwrap();
    observed.store(false, Ordering::Release);
    watch.observe();
    drop(model);
    assert!(observed.load(Ordering::Acquire));
    assert_eq!(planner.footprint().live, 0);
}
