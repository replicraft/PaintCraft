use super::*;
use std::{hash::BuildHasher, mem::size_of};
fn coord(tx: i64) -> TileCoord {
    TileCoord { tx, ty: 0 }
}
fn tile() -> Arc<Tile> {
    Arc::new(Tile::empty())
}
fn fill(table: &mut TileTable, keys: &[TileCoord], value: &Arc<Tile>) {
    table.try_reserve(keys.len()).unwrap();
    for key in keys {
        table.insert_reserved(*key, value.clone()).unwrap();
    }
}
fn collision_keys(table: &TileTable, count: usize) -> Vec<TileCoord> {
    // Safe public hasher access only inside this private test module. All map
    // growth/removal still uses the production tracking boundary.
    let mut keys = Vec::new();
    for n in 0..1_000_000i64 {
        let key = coord(n);
        if table.values.hasher().hash_one(key) & 63 == 0 {
            keys.push(key);
        }
        if keys.len() == count {
            break;
        }
    }
    assert_eq!(keys.len(), count);
    keys
}
#[test]
fn frozen_target_layouts_and_fresh_thresholds_are_exact() {
    assert_eq!(size_of::<(TileCoord, Arc<Tile>)>(), 24);
    #[cfg(target_pointer_width = "64")]
    assert_eq!(
        (
            size_of::<HashMap<TileCoord, Arc<Tile>>>(),
            size_of::<TileTable>(),
            size_of::<RasterLayer>()
        ),
        (48, 64, 80)
    );
    #[cfg(target_arch = "wasm32")]
    assert_eq!(
        (
            size_of::<HashMap<TileCoord, Arc<Tile>>>(),
            size_of::<TileTable>(),
            size_of::<RasterLayer>()
        ),
        (32, 48, 64)
    );
    for (entries, buckets, capacity, bytes) in [
        (0, 0, 0, 0),
        (1, 4, 3, 116),
        (3, 4, 3, 116),
        (4, 8, 7, 216),
        (7, 8, 7, 216),
        (8, 16, 14, 416),
        (14, 16, 14, 416),
        (15, 32, 28, 816),
        (28, 32, 28, 816),
        (29, 64, 56, 1616),
    ] {
        let mut table = TileTable::new();
        table.try_reserve(entries).unwrap();
        let q = table.retained_quote().unwrap();
        assert_eq!(
            (q.bucket_upper, table.capacity(), q.allocation_upper),
            (buckets, capacity, bytes)
        );
        assert_eq!(q.scan_work_upper, bytes + buckets as u64);
        assert_eq!(q.live_entries, 0);
    }
}
#[test]
fn occupied_full_replacement_and_value_factory_cannot_grow_or_recreate() {
    let mut table = TileTable::new();
    let value = tile();
    fill(&mut table, &[coord(0), coord(1), coord(2)], &value);
    assert_eq!(table.len(), table.capacity());
    let before = table.retained_quote().unwrap();
    let replacement = tile();
    let old = table
        .insert_reserved(coord(1), replacement.clone())
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&old, &value));
    let occupied = table
        .get_or_try_insert_with(coord(1), || panic!("occupied factory executed"))
        .unwrap();
    assert!(Arc::ptr_eq(occupied, &replacement));
    assert_eq!(table.retained_quote().unwrap(), before);
    assert!(table.insert_reserved(coord(3), tile()).is_err());
    assert_eq!(table.retained_quote().unwrap(), before);
    table
        .get_or_try_insert_with(coord(3), || Ok(tile()))
        .unwrap();
    assert_eq!((table.len(), table.bucket_upper), (4, 8));
}
#[test]
fn failed_reserve_and_spare_reserve_preserve_actual_retained_extent() {
    let mut table = TileTable::new();
    let value = tile();
    fill(&mut table, &[coord(0)], &value);
    let before = table.retained_quote().unwrap();
    let capacity = table.capacity();
    table.try_reserve(0).unwrap();
    table.try_reserve(1).unwrap();
    assert_eq!(table.retained_quote().unwrap(), before);
    assert!(table.try_reserve(usize::MAX).is_err());
    assert_eq!(table.retained_quote().unwrap(), before);
    assert_eq!(table.capacity(), capacity);
    assert!(Arc::ptr_eq(table.get(&coord(0)).unwrap(), &value));
    let mut empty = TileTable::new();
    assert!(empty.try_reserve(usize::MAX).is_err());
    assert_eq!(empty.retained_quote().unwrap().allocation_upper, 0);
}
#[test]
fn tombstone_clone_and_repeated_inplace_reserve_do_not_inflate_buckets() {
    let mut table = TileTable::new();
    table.try_reserve(56).unwrap();
    let keys = collision_keys(&table, 56);
    let value = tile();
    fill(&mut table, &keys, &value);
    assert_eq!((table.bucket_upper, table.capacity()), (64, 56));
    for _ in 0..4 {
        for key in &keys[..40] {
            assert!(table.remove(key).is_some());
        }
        assert!(
            table.capacity() < 28,
            "pinned collision fixture must retain DELETED groups"
        );
        let before = table.retained_quote().unwrap();
        assert_eq!(
            (
                before.bucket_upper,
                before.allocation_upper,
                before.live_entries
            ),
            (64, 1616, 16)
        );
        let cloned = table.clone();
        assert_eq!(cloned.retained_quote().unwrap(), before);
        assert_eq!(cloned.capacity(), table.capacity());
        let additional = table.capacity() - table.len() + 1;
        table.try_reserve(additional).unwrap();
        assert_eq!(
            (table.bucket_upper, table.allocation_upper, table.capacity()),
            (64, 1616, 56)
        );
        assert_eq!(table.len(), 16);
        for key in &keys[..40] {
            table.insert_reserved(*key, value.clone()).unwrap();
        }
    }
}
#[test]
fn allocated_empty_clone_keeps_bound_without_len_or_capacity_inference() {
    let mut table = TileTable::new();
    table.try_reserve(56).unwrap();
    let keys = collision_keys(&table, 56);
    let value = tile();
    fill(&mut table, &keys, &value);
    for key in keys {
        table.remove(&key);
    }
    assert!(table.is_empty());
    assert_eq!(table.bucket_upper, 64);
    let before = table.retained_quote().unwrap();
    let cloned = table.clone();
    assert_eq!(cloned.retained_quote().unwrap(), before);
    assert_eq!(cloned.capacity(), table.capacity());
    assert_eq!(before.allocation_upper, 1616);
}
#[test]
fn sparse_geometry_and_fresh_compact_clone_are_distinct_source_facts() {
    let mut small = RasterLayer::new(1, 1, Rgba::TRANSPARENT);
    let mut large = RasterLayer::new(1_048_576, 1_048_576, Rgba::TRANSPARENT);
    let mut adm = Admission::new(2 * 65536);
    for raster in [&mut small, &mut large] {
        raster
            .blend_pixel(0, 0, Rgba([1, 2, 3, 255]), BlendMode::Replace, &mut adm)
            .unwrap();
    }
    assert_eq!(
        small.retained_tile_table_quote().unwrap(),
        large.retained_tile_table_quote().unwrap()
    );
    let original = large.tiles.values().next().unwrap().clone();
    large.tiles.try_reserve(56).unwrap();
    for n in 1..=56 {
        large
            .tiles
            .insert_reserved(coord(n), original.clone())
            .unwrap();
    }
    for n in 1..=56 {
        large.tiles.remove(&coord(n));
    }
    let retained = large.retained_tile_table_quote().unwrap();
    assert!(retained.bucket_upper > 4);
    let layout = large.compact_metadata_clone_quote().unwrap();
    let scope = SharedBudget::new(1 << 20).scope(AllocationClass::PaintMetadata);
    let mut charge = scope.account();
    charge.resize(layout.bytes).unwrap();
    let compact = large
        .compact_metadata_clone_precharged_checked(&charge, &|| Ok(()), &|_| Ok(()))
        .unwrap();
    assert_eq!(compact.retained_tile_table_quote().unwrap().bucket_upper, 4);
    assert_eq!(large.retained_tile_table_quote().unwrap(), retained);
    assert!(Arc::ptr_eq(
        compact.tiles.values().next().unwrap(),
        &original
    ));
    drop(compact);
    drop(charge);
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn fresh_factory_failure_has_no_inserted_key_or_untracked_growth() {
    let mut table = TileTable::new();
    let error = table
        .get_or_try_insert_with(coord(0), || {
            Err(Error::new(ErrorCode::OperationCancelled, "factory stop"))
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(table.len(), 0);
    // The successful index reservation remains real even though value creation refused.
    assert_eq!((table.bucket_upper, table.allocation_upper), (4, 116));
    let q = table.retained_quote().unwrap();
    assert_eq!(q.scan_work_upper, 120);
    assert_eq!(table.clone().retained_quote().unwrap(), q);
}
#[test]
fn every_ingest_replay_snapshot_paste_and_reserved_decode_path_keeps_provenance() {
    use paintcraft_core::types::clipboard::Image;
    let pixels = [1, 2, 3, 255, 4, 5, 6, 255];
    let mut adm = Admission::new(1 << 20);
    // Both imports, the installed tile's COW result, and shifted paste coexist.
    // Reserve their union before the first import fixes this admission's realm.
    adm.reserve_tiles(4).unwrap();
    let source =
        RasterLayer::from_rgba_checked(2, 1, Rgba::TRANSPARENT, &pixels, &mut adm, &|| Ok(()))
            .unwrap();
    let old = RasterLayer::from_rgba(2, 1, Rgba::TRANSPARENT, &pixels, &mut adm).unwrap();
    assert_eq!(
        source.retained_tile_table_quote().unwrap(),
        old.retained_tile_table_quote().unwrap()
    );
    assert_eq!(
        (source.tiles.bucket_upper, source.tiles.allocation_upper),
        (4, 116)
    );
    let mut installed = RasterLayer::new(2, 1, Rgba::TRANSPARENT);
    installed
        .install_tile(coord(0), source.tile_leases().next().unwrap().1)
        .unwrap();
    assert_eq!(
        installed.retained_tile_table_quote().unwrap(),
        source.retained_tile_table_quote().unwrap()
    );
    let mut plan = PaintPlan::default();
    plan.rect(
        Rect::new(0, 0, 2, 1).unwrap(),
        Rgba([9, 8, 7, 255]),
        BlendMode::Replace,
    )
    .unwrap();
    plan.apply_checked(&mut installed, &mut adm, &|| Ok(()))
        .unwrap();
    assert_eq!(installed.get(0, 0), Rgba([9, 8, 7, 255]));
    assert_eq!(installed.tiles.bucket_upper, 4);
    let metadata = SharedBudget::new(4 << 20);
    let scope = metadata.scope(AllocationClass::PaintMetadata);
    let snapshot_adm = Admission::new(1 << 20).with_paint_metadata_scope(scope.clone());
    let snapshot = PaintSnapshot::capture(&source, &snapshot_adm, &|| Ok(())).unwrap();
    let owned = PaintSnapshot::new(source.clone(), &snapshot_adm, &|| Ok(())).unwrap();
    for snapshot in [&snapshot, &owned] {
        let reader = snapshot.reader(&snapshot_adm, &|| Ok(())).unwrap();
        assert_eq!(
            reader.read_checked(0, 0, &|| Ok(())).unwrap(),
            Rgba([1, 2, 3, 255])
        );
    }
    let image = || {
        Image::from_fn(2, 1, &SharedBudget::new(4 << 20), &|| Ok(()), |_, _| {
            Ok(Rgba::TRANSPARENT)
        })
        .unwrap()
    };
    let paste = RgbaPastePlan::prepare(
        &source,
        image(),
        (0, 0),
        BlendMode::Replace,
        None,
        &snapshot_adm,
        &|| Ok(()),
    )
    .unwrap();
    let empty = paste.replay(&mut adm, &|| Ok(())).unwrap();
    assert_eq!(empty.resident_tiles(), 0);
    assert_eq!(
        (empty.tiles.bucket_upper, empty.tiles.allocation_upper),
        (4, 116)
    );
    let shifted_plan = RgbaPastePlan::prepare(
        &source,
        image(),
        (-1, 0),
        BlendMode::Replace,
        None,
        &snapshot_adm,
        &|| Ok(()),
    )
    .unwrap();
    let shifted = shifted_plan.replay(&mut adm, &|| Ok(())).unwrap();
    assert_eq!(shifted.get(2, 0), Rgba([4, 5, 6, 255]));
    assert_eq!(shifted.tiles.bucket_upper, 4);
    let decoded = SharedBudget::new(1 << 20);
    let mut admission = Admission::with_reserved_decode_factories(
        65536,
        decoded.reserve_capacity(262144).unwrap(),
        ReservedDecodeFactories {
            selection_budget: SharedBudget::new(128),
            coverage_budget: SharedBudget::new(128),
            paint_metadata_scope: scope.clone(),
        },
        &scope,
        &|| Ok(()),
    )
    .unwrap();
    let mut raster = RasterLayer::new(2, 1, Rgba::TRANSPARENT);
    admission
        .write_block(&mut raster, 0, 0, &pixels, &|| Ok(()))
        .unwrap();
    assert_eq!(
        raster.retained_tile_table_quote().unwrap(),
        source.retained_tile_table_quote().unwrap()
    );
    drop(admission);
    drop(raster);
    assert_eq!(decoded.usage().reserved, 0);
    drop(shifted_plan);
    drop(snapshot);
    drop(owned);
    drop(paste);
    drop(snapshot_adm);
    assert_eq!(scope.footprint().live, 0);
}
