use super::*;
use std::cell::Cell;
const BYTES: u64 = 262144;
const PIXELS: u64 = 65536;
fn stop() -> Error {
    Error::new(ErrorCode::OperationCancelled, "independent stop")
}
fn factories() -> ReservedDecodeFactories {
    ReservedDecodeFactories {
        selection_budget: SharedBudget::new(128),
        coverage_budget: SharedBudget::new(128),
        paint_metadata_scope: SharedBudget::new(128).scope(AllocationClass::PaintMetadata),
    }
}
// Independent structural formula, not control_quote-derived capacity.
fn independent_arc<T>() -> u64 {
    let a = std::mem::align_of::<T>();
    let h = 2 * std::mem::size_of::<usize>();
    let offset = h.div_ceil(a) * a;
    let alignment = a.max(std::mem::align_of::<usize>());
    ((offset + std::mem::size_of::<T>()).div_ceil(alignment) * alignment) as u64
}
fn fixed() -> u64 {
    independent_arc::<Mutex<tile::Scope>>() + independent_arc::<BudgetAccount>()
}
fn owner(tiles: u64, pixels: u64, scope: &BudgetScope) -> (ReservedDecodeAdmission, SharedBudget) {
    let budget = SharedBudget::new((tiles + 2) * BYTES);
    let admission = Admission::with_reserved_decode_factories(
        pixels,
        budget.reserve_capacity(tiles * BYTES).unwrap(),
        factories(),
        scope,
        &|| Ok(()),
    )
    .unwrap();
    (admission, budget)
}
#[test]
fn exact_control_capacity_and_original_tracker_not_parent_headroom() {
    let q = ReservedDecodeAdmission::control_quote().unwrap();
    assert_eq!(q.fixed_bytes, fixed());
    assert_eq!(q.gauge_arc_bytes, independent_arc::<Mutex<tile::Scope>>());
    assert_eq!(q.control_arc_bytes, independent_arc::<BudgetAccount>());
    assert_eq!(q.tile_header_bytes, independent_arc::<Tile>());
    assert_eq!(
        q.handle_bytes,
        std::mem::size_of::<ReservedDecodeAdmission>() as u64
    );
    for short in [false, true] {
        let budget = SharedBudget::new(fixed() * 4);
        let scope = budget
            .reserve_capacity(fixed() - u64::from(short))
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let decode = SharedBudget::new(BYTES);
        let result = Admission::with_reserved_decode_factories(
            PIXELS,
            decode.reserve_capacity(BYTES).unwrap(),
            factories(),
            &scope,
            &|| Ok(()),
        );
        if short {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
            assert_eq!(scope.footprint().live, 0);
        } else {
            let admission = result.unwrap();
            assert_eq!(scope.footprint().live, fixed());
            drop(admission);
            assert_eq!(scope.footprint().live, 0);
        }
        drop(scope);
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(decode.usage().reserved, 0);
    }
}
#[test]
fn supplied_factories_are_retained_without_replacement() {
    let f = factories();
    let selected = f.selection_budget.reserve(128).unwrap();
    let covered = f.coverage_budget.reserve(128).unwrap();
    let metadata = f.paint_metadata_scope.clone();
    let scope = SharedBudget::new(fixed()).scope(AllocationClass::PaintMetadata);
    let decoded = SharedBudget::new(BYTES);
    let a = Admission::with_reserved_decode_factories(
        PIXELS,
        decoded.reserve_capacity(BYTES).unwrap(),
        f,
        &scope,
        &|| Ok(()),
    )
    .unwrap();
    assert!(a.admission.selection_budget().reserve(1).is_err());
    assert!(a.admission.coverage_budget().reserve(1).is_err());
    drop(selected);
    drop(covered);
    assert!(a.admission.selection_budget().reserve(128).is_ok());
    assert!(a.admission.coverage_budget().reserve(128).is_ok());
    let mut m = a.admission.paint_metadata_account();
    m.resize(37).unwrap();
    assert_eq!(metadata.footprint().live, 37);
    drop(m);
    assert_eq!(metadata.footprint().live, 0);
}
#[test]
fn constructor_cancellation_at_each_boundary_releases_every_owner() {
    for cut in 1..=3 {
        let control = SharedBudget::new(fixed());
        let scope = control.scope(AllocationClass::PaintMetadata);
        let decoded = SharedBudget::new(BYTES);
        let n = Cell::new(0u64);
        let result = Admission::with_reserved_decode_factories(
            PIXELS,
            decoded.reserve_capacity(BYTES).unwrap(),
            factories(),
            &scope,
            &|| {
                n.set(n.get() + 1);
                if n.get() == cut { Err(stop()) } else { Ok(()) }
            },
        );
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(n.get(), cut);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(decoded.usage().reserved, 0);
        assert_eq!(control.usage().reserved, 0);
    }
}
#[test]
fn first_write_cancel_precedes_even_index_or_pixel_mutation() {
    let scope = SharedBudget::new(fixed()).scope(AllocationClass::PaintMetadata);
    let (mut a, budget) = owner(1, PIXELS, &scope);
    let mut layer = RasterLayer::new(512, 256, Rgba::TRANSPARENT);
    let before = layer.clone();
    let capacity = layer.tiles.capacity();
    assert_eq!(
        a.write_block(&mut layer, 0, 0, &vec![7; BYTES as usize], &|| Err(stop()))
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(layer, before);
    assert_eq!(layer.tiles.capacity(), capacity);
    assert_eq!(a.remaining_pixels(), PIXELS);
    assert_eq!(a.admission.reservation.as_ref().unwrap().available(), BYTES);
    drop(a);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn capacity_error_cannot_escape_original_reservation_or_orphan_prior_tile() {
    let scope = SharedBudget::new(fixed()).scope(AllocationClass::PaintMetadata);
    let (mut a, budget) = owner(1, 3 * PIXELS, &scope);
    assert_eq!(a.reservation_capacity(), BYTES);
    assert!(a.reserve_tiles(2).is_err());
    let mut layer = RasterLayer::new(512, 256, Rgba::TRANSPARENT);
    a.write_block(&mut layer, 0, 0, &vec![7; BYTES as usize], &|| Ok(()))
        .unwrap();
    assert_eq!(
        a.write_block(&mut layer, 1, 0, &vec![9; BYTES as usize], &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(layer.resident_tiles(), 1);
    drop(a);
    assert_eq!(scope.footprint().live, fixed());
    assert_eq!(budget.usage().reserved, BYTES);
    assert_eq!(layer.get(0, 0), Rgba([7; 4]));
    assert_eq!(layer.get(256, 0), Rgba::TRANSPARENT);
    drop(layer);
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn cancellation_after_second_local_allocation_keeps_first_tile_owned() {
    let scope = SharedBudget::new(fixed()).scope(AllocationClass::PaintMetadata);
    let (mut a, budget) = owner(2, 2 * PIXELS, &scope);
    let mut layer = RasterLayer::new(512, 256, Rgba::TRANSPARENT);
    a.write_block(&mut layer, 0, 0, &vec![7; BYTES as usize], &|| Ok(()))
        .unwrap();
    let n = Cell::new(0u64);
    assert_eq!(
        a.write_block(&mut layer, 1, 0, &vec![9; BYTES as usize], &|| {
            n.set(n.get() + 1);
            if n.get() == 2 { Err(stop()) } else { Ok(()) }
        })
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(n.get(), 2);
    assert_eq!(layer.resident_tiles(), 1);
    assert_eq!(
        a.admission
            .allocation_scope
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .live,
        BYTES
    );
    let clone = layer.clone();
    let lease = layer.tile_leases().next().unwrap().1;
    drop(a);
    drop(layer);
    drop(clone);
    assert_eq!(scope.footprint().live, fixed());
    assert_eq!(budget.usage().reserved, BYTES);
    assert_eq!(&lease.bytes()[..4], &[7; 4]);
    drop(lease);
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn replacement_requires_fresh_admission_and_finite_padding_stays_default() {
    let scope = SharedBudget::new(fixed()).scope(AllocationClass::PaintMetadata);
    let (mut a, budget) = owner(2, 2 * PIXELS, &scope);
    let mut layer = RasterLayer::new(1, 1, Rgba([3, 5, 7, 0]));
    a.write_block(&mut layer, 0, 0, &[1, 2, 3, 255], &|| Ok(()))
        .unwrap();
    let first = layer.tile_leases().next().unwrap().1;
    a.write_block(&mut layer, 0, 0, &[9, 8, 7, 255], &|| Ok(()))
        .unwrap();
    assert_eq!(&first.bytes()[..4], &[1, 2, 3, 255]);
    assert_eq!(layer.get(0, 0), Rgba([9, 8, 7, 255]));
    assert_eq!(&first.bytes()[4..8], &[3, 5, 7, 0]);
    assert_eq!(a.remaining_pixels(), 0);
    drop(a);
    drop(layer);
    assert_eq!(scope.footprint().live, fixed());
    drop(first);
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn concurrent_last_distinct_tile_owners_release_shared_controls_once() {
    for _ in 0..16 {
        let scope = SharedBudget::new(fixed()).scope(AllocationClass::PaintMetadata);
        let (mut a, budget) = owner(2, 2 * PIXELS, &scope);
        let mut layer = RasterLayer::new(512, 256, Rgba::TRANSPARENT);
        for x in 0..2 {
            a.write_block(&mut layer, x, 0, &vec![7; BYTES as usize], &|| Ok(()))
                .unwrap();
        }
        let mut leases = layer.tile_leases().map(|(_, l)| l).collect::<Vec<_>>();
        let one = leases.pop().unwrap();
        let two = leases.pop().unwrap();
        drop(a);
        drop(layer);
        assert_eq!(scope.footprint().live, fixed());
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|threads| {
            let b = &barrier;
            let t = threads.spawn(move || {
                b.wait();
                drop(one);
            });
            barrier.wait();
            drop(two);
            t.join().unwrap();
        });
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(budget.usage().reserved, 0);
    }
}
