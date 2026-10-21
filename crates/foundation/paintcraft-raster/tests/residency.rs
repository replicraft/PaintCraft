use paintcraft_core::types::{Error, ErrorCode, Rgba, resources::SharedBudget};
use paintcraft_raster::{Admission, BlendMode, RasterLayer, TileLease};
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
fn admission(budget: &SharedBudget, tiles: u64) -> Admission {
    Admission::with_residency(tiles * 65_536, budget.clone())
}
fn ink() -> Rgba {
    Rgba([13, 29, 41, 0])
}

#[test]
fn readers_keep_decoded_bytes_charged_and_weak_keys_do_not() {
    let budget = SharedBudget::new(BYTES);
    let mut raster = RasterLayer::new(256, 256, Rgba::TRANSPARENT);
    raster
        .blend_pixel(0, 0, ink(), BlendMode::Replace, &mut admission(&budget, 1))
        .unwrap();
    let snapshot = raster.clone();
    let reader = raster.tile_leases().next().unwrap().1;
    let version = reader.version();
    assert_eq!(budget.usage().reserved, BYTES);
    let error = raster
        .blend_pixel(
            0,
            0,
            Rgba([255; 4]),
            BlendMode::Replace,
            &mut admission(&budget, 1),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(raster.get(0, 0), ink());
    assert_eq!(budget.usage().reserved, BYTES);
    drop(snapshot);
    assert!(
        raster
            .blend_pixel(
                0,
                0,
                Rgba([255; 4]),
                BlendMode::Replace,
                &mut admission(&budget, 1)
            )
            .is_err()
    );
    drop(reader);
    // Only a weak cache key remains: move ownership, don't allocate/copy pixels.
    raster
        .blend_pixel(
            0,
            0,
            Rgba([255; 4]),
            BlendMode::Replace,
            &mut admission(&budget, 0),
        )
        .unwrap();
    assert_eq!(budget.usage().reserved, BYTES);
    assert_eq!(version.resident_bytes(), 0);
    drop(raster);
    assert_eq!(budget.usage().reserved, 0);
    assert!(version.try_lease().is_none());
}
#[test]
fn each_cow_allocation_is_charged_once_until_its_final_owner_drops() {
    let budget = SharedBudget::new(2 * BYTES);
    let mut old = RasterLayer::new(256, 256, Rgba::TRANSPARENT);
    old.blend_pixel(0, 0, ink(), BlendMode::Replace, &mut admission(&budget, 1))
        .unwrap();
    let mut edited = old.clone();
    let reader = old.tile_leases().next().unwrap().1;
    edited
        .blend_pixel(
            0,
            0,
            Rgba([255; 4]),
            BlendMode::Replace,
            &mut admission(&budget, 1),
        )
        .unwrap();
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    assert_eq!(old.get(0, 0), ink());
    assert_eq!(edited.get(0, 0), Rgba([255; 4]));
    drop(old);
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    drop(edited);
    assert_eq!(budget.usage().reserved, BYTES);
    drop(reader);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn failed_materialization_returns_all_owned_bytes_and_never_exposes_partial_pixels() {
    let budget = SharedBudget::new(BYTES);
    let error = TileLease::materialize(Rgba::TRANSPARENT, &mut admission(&budget, 1), |data| {
        data[..4].copy_from_slice(&ink().0);
        Err(Error::new(
            ErrorCode::CodecFailed,
            "injected decode failure",
        ))
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::CodecFailed);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(budget.usage().peak, BYTES);
    let denied = SharedBudget::new(0);
    assert!(
        TileLease::materialize(Rgba::TRANSPARENT, &mut admission(&denied, 1), |_| panic!(
            "denied allocation must not invoke writer"
        ))
        .is_err()
    );
    assert_eq!(denied.usage().reserved, 0);
    assert!(
        TileLease::materialize(Rgba::TRANSPARENT, &mut admission(&budget, 0), |_| panic!(
            "staged denial must not invoke writer"
        ))
        .is_err()
    );
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn partial_import_failure_releases_previously_materialized_edge_tiles() {
    let mut bytes = vec![0; 257 * 4];
    bytes[..4].copy_from_slice(&ink().0);
    bytes[256 * 4..].copy_from_slice(&ink().0);
    let budget = SharedBudget::new(BYTES);
    assert_eq!(
        RasterLayer::from_rgba(
            257,
            1,
            Rgba::TRANSPARENT,
            &bytes,
            &mut admission(&budget, 2)
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, 0);
    let enough = SharedBudget::new(2 * BYTES);
    let layer = RasterLayer::from_rgba(
        257,
        1,
        Rgba::TRANSPARENT,
        &bytes,
        &mut admission(&enough, 2),
    )
    .unwrap();
    assert_eq!(enough.usage().reserved, 2 * BYTES);
    assert_eq!(layer.get(0, 0), ink());
    assert_eq!(layer.get(256, 0), ink());
    drop(layer);
    assert_eq!(enough.usage().reserved, 0);
}
#[test]
fn racing_allocators_cannot_claim_the_same_resident_capacity() {
    use std::sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering},
    };
    let budget = SharedBudget::new(2 * BYTES);
    let start = Barrier::new(9);
    let allocated = Barrier::new(9);
    let release = Barrier::new(9);
    let successes = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let budget = budget.clone();
            let (start, allocated, release, successes) = (&start, &allocated, &release, &successes);
            scope.spawn(move || {
                start.wait();
                let tile =
                    TileLease::materialize(ink(), &mut admission(&budget, 1), |_| Ok(())).ok();
                if tile.is_some() {
                    successes.fetch_add(1, Ordering::AcqRel);
                }
                allocated.wait();
                release.wait();
                drop(tile);
            });
        }
        start.wait();
        allocated.wait();
        let count = successes.load(Ordering::Acquire);
        let used = budget.usage().reserved;
        release.wait();
        assert_eq!(count, 2);
        assert_eq!(used, 2 * BYTES);
    });
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(budget.usage().peak, 2 * BYTES);
}

#[test]
fn independent_default_admissions_observe_the_same_process_tile_realm() {
    let mut first = Admission::new(65_536);
    let second = Admission::new(65_536);
    assert_eq!(
        first.residency_usage().limit,
        paintcraft_raster::DEFAULT_RESIDENT_TILE_BYTES
    );
    // Other tests in this integration binary use explicit, isolated realms.
    assert_eq!(second.residency_usage().reserved, 0);
    let tile = TileLease::materialize(ink(), &mut first, |_| Ok(())).unwrap();
    assert_eq!(second.residency_usage().reserved, BYTES);
    assert_eq!(first.residency_usage(), second.residency_usage());
    drop(first);
    assert_eq!(second.residency_usage().reserved, BYTES);
    drop(tile);
    assert_eq!(second.residency_usage().reserved, 0);
}

#[test]
fn denied_partial_attempt_requires_its_whole_known_footprint_not_just_one_more_tile() {
    use paintcraft_core::types::resources::AllocationClass;
    let budget = SharedBudget::new(2 * BYTES);
    let blocker = budget.reserve(BYTES).unwrap();
    let mut scope = admission(&budget, 2);
    let mut layer = RasterLayer::new(512, 1, Rgba::TRANSPARENT);
    layer
        .blend_pixel(0, 0, ink(), BlendMode::Replace, &mut scope)
        .unwrap();
    let error = layer
        .blend_pixel(256, 0, ink(), BlendMode::Replace, &mut scope)
        .unwrap_err();
    let denial = error.allocation_denial().unwrap();
    assert_eq!(denial.class(), AllocationClass::DecodedTiles);
    assert_eq!(denial.required_bytes(), 2 * BYTES);
    drop(layer);
    assert_eq!(budget.usage().reserved, BYTES);
    assert!(
        !denial.can_retry(),
        "rolling back one's own partial work must not trigger the same failing attempt"
    );
    drop(blocker);
    assert!(denial.can_retry());
}
#[test]
fn released_temporaries_do_not_inflate_the_attempt_requirement_and_earlier_peak_is_retained() {
    let budget = SharedBudget::new(3 * BYTES);
    let mut scope = admission(&budget, 8);
    let first = TileLease::materialize(ink(), &mut scope, |_| Ok(())).unwrap();
    let second = TileLease::materialize(ink(), &mut scope, |_| Ok(())).unwrap();
    drop(first);
    drop(second);
    let a = budget.reserve(2 * BYTES).unwrap();
    let b = budget.reserve(BYTES).unwrap();
    let error = TileLease::materialize(ink(), &mut scope, |_| Ok(())).unwrap_err();
    let denial = error.allocation_denial().unwrap();
    assert_eq!(denial.required_bytes(), 2 * BYTES);
    assert!(denial.is_possible());
    drop(b);
    assert!(
        !denial.can_retry(),
        "retry must fit the earlier two-tile peak"
    );
    drop(a);
    assert!(denial.can_retry());
    // A fresh attempt can actually repeat the known peak under the same realm.
    let mut retry = admission(&budget, 2);
    let first = TileLease::materialize(ink(), &mut retry, |_| Ok(())).unwrap();
    let second = TileLease::materialize(ink(), &mut retry, |_| Ok(())).unwrap();
    drop(first);
    drop(second);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn fixed_staging_limits_are_not_misclassified_as_reclaimable_tile_pressure() {
    let budget = SharedBudget::new(BYTES);
    let mut layer = RasterLayer::new(256, 256, Rgba::TRANSPARENT);
    let error = layer
        .blend_pixel(0, 0, ink(), BlendMode::Replace, &mut admission(&budget, 0))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(error.allocation_denial().is_none());
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn reserved_tiles_recycle_scratch_without_reentering_shared_admission_and_release_unused_bytes() {
    let budget = SharedBudget::new(3 * BYTES);
    let reservation = budget.reserve_capacity(2 * BYTES).unwrap();
    let competing = budget.reserve(BYTES).unwrap();
    let mut admission = Admission::with_reserved_residency(4 * 65_536, reservation);
    let first = TileLease::materialize(Rgba([1, 2, 3, 4]), &mut admission, |_| Ok(())).unwrap();
    let scratch = TileLease::materialize(Rgba::TRANSPARENT, &mut admission, |_| Ok(())).unwrap();
    assert!(budget.reserve(1).is_err());
    drop(scratch);
    let scratch = TileLease::materialize(Rgba::TRANSPARENT, &mut admission, |_| Ok(())).unwrap();
    drop(scratch);
    assert_eq!(budget.usage().reserved, 3 * BYTES);
    drop(admission);
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    let restored = first.clone();
    drop(first);
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    drop(restored);
    assert_eq!(budget.usage().reserved, BYTES);
    drop(competing);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn reservation_exhaustion_is_atomic_and_failed_materialization_returns_capacity() {
    let budget = SharedBudget::new(4 * BYTES);
    let reservation = budget.reserve_capacity(BYTES).unwrap();
    let mut admission = Admission::with_reserved_residency(4 * 65_536, reservation.clone());
    let first = TileLease::materialize(Rgba::TRANSPARENT, &mut admission, |_| Ok(())).unwrap();
    let error = TileLease::materialize(Rgba::TRANSPARENT, &mut admission, |_| Ok(())).unwrap_err();
    assert_eq!(
        error.allocation_denial().unwrap().required_bytes(),
        2 * BYTES
    );
    assert_eq!(budget.usage().reserved, BYTES); // Free global bytes cannot bypass the pool.
    drop(first);
    let error = TileLease::materialize(Rgba::TRANSPARENT, &mut admission, |_| {
        Err(paintcraft_core::types::Error::new(
            ErrorCode::OperationCancelled,
            "injected",
        ))
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(reservation.available(), BYTES);
    let tile = TileLease::materialize(Rgba::TRANSPARENT, &mut admission, |_| Ok(())).unwrap();
    drop((reservation, admission));
    assert_eq!(budget.usage().reserved, BYTES);
    drop(tile);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn full_tile_plan_admits_once_without_double_charging_staged_pixels_or_expanding_explicit_pools() {
    let budget = SharedBudget::new(3 * BYTES);
    let blocker = budget.reserve(BYTES).unwrap();
    let mut admission = Admission::with_residency(2 * 65_536, budget.clone());
    admission.reserve_tiles(2).unwrap();
    assert_eq!(budget.usage().reserved, 3 * BYTES);
    assert!(budget.reserve(1).is_err());
    let a = TileLease::materialize(Rgba::TRANSPARENT, &mut admission, |_| Ok(())).unwrap();
    let b = TileLease::materialize(Rgba::TRANSPARENT, &mut admission, |_| Ok(())).unwrap();
    let error = admission.reserve_tiles(1).unwrap_err();
    assert!(error.allocation_denial().is_none()); // Cumulative staged cap still applies.
    drop((a, b, admission, blocker));
    assert_eq!(budget.usage().reserved, 0);
    let mut admission =
        Admission::with_reserved_residency(2 * 65_536, budget.reserve_capacity(BYTES).unwrap());
    let error = admission.reserve_tiles(2).unwrap_err();
    assert_eq!(
        error.allocation_denial().unwrap().required_bytes(),
        2 * BYTES
    );
    assert_eq!(budget.usage().reserved, BYTES);
}
