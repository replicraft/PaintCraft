use super::*;
fn versions(raster: &RasterLayer) -> Vec<(RegionCoord, RegionVersion)> {
    raster
        .region_versions(raster.bounds())
        .unwrap()
        .unwrap()
        .collect()
}
#[test]
fn region_versions_preserve_untouched_defaults_and_cow_cells_without_pinning_pixels() {
    let mut raster = RasterLayer::new(256, 256, Rgba([31, 63, 127, 0]));
    let empty = versions(&raster);
    raster
        .blend_pixel(
            3,
            4,
            Rgba([1; 4]),
            BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    let first = versions(&raster);
    assert_ne!(first[0], empty[0]);
    assert_eq!(first[1..], empty[1..]);
    let old = raster.clone();
    raster
        .fill_rect(
            &Rect::new(64, 64, 2, 2).unwrap(),
            Rgba([2; 4]),
            BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    let second = versions(&raster);
    assert_ne!(first[5], second[5]);
    for i in 0..16 {
        if i != 5 {
            assert_eq!(first[i], second[i]);
        }
    }
    assert_eq!(versions(&old), first);
    assert_eq!(old.get(64, 64), Rgba([31, 63, 127, 0]));
    assert_eq!(second.len(), 16);
}
#[test]
fn restored_imported_and_installed_leases_never_confuse_unwritten_colors() {
    let mut source = RasterLayer::new(256, 256, Rgba([1, 2, 3, 0]));
    source
        .blend_pixel(
            0,
            0,
            Rgba([4; 4]),
            BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    let mut different_default = RasterLayer::new(256, 256, Rgba([9, 8, 7, 0]));
    let missing = versions(&different_default);
    different_default
        .install_tile(
            TileCoord { tx: 0, ty: 0 },
            source.tile_leases().next().unwrap().1,
        )
        .unwrap();
    assert_ne!(versions(&different_default)[1], missing[1]);
    assert_eq!(versions(&different_default), versions(&source));
    let before = versions(&source);
    source
        .set_block(
            0,
            0,
            &vec![17; RasterLayer::TILE_BYTES],
            &mut Admission::new(65536),
        )
        .unwrap();
    assert!(versions(&source).iter().zip(before).all(|(a, b)| *a != b));
    let lease = TileLease::materialize(Rgba([1; 4]), &mut Admission::new(65536), |b| {
        b[0] = 42;
        Ok(())
    })
    .unwrap();
    source
        .install_tile(TileCoord { tx: 0, ty: 0 }, lease)
        .unwrap();
    assert_eq!(source.get(0, 0), Rgba([42, 1, 1, 1]));
}
#[test]
fn revision_exhaustion_never_wraps_and_disables_derived_cache_keys() {
    let counter = AtomicU64::new(u64::MAX - 1);
    assert_eq!(revision_from(&counter), u64::MAX - 1);
    assert_eq!(revision_from(&counter), u64::MAX);
    assert_eq!(revision_from(&counter), u64::MAX);
    let mut raster = RasterLayer::new(256, 256, Rgba::TRANSPARENT);
    raster
        .with_tile_mut(
            TileCoord { tx: 0, ty: 0 },
            &mut Admission::new(65536),
            |t| t.mark_region(Rect::new(0, 0, 1, 1).unwrap(), u64::MAX),
        )
        .unwrap();
    assert!(raster.region_versions(raster.bounds()).unwrap().is_none());
    assert!(
        raster
            .region_versions(Rect::new(-1, 0, 1, 1).unwrap())
            .is_err()
    );
}

#[test]
fn dependency_tokens_outlive_the_pixels_without_holding_decoded_capacity() {
    let budget =
        paintcraft_core::types::resources::SharedBudget::new(RasterLayer::TILE_BYTES as u64);
    let mut raster = RasterLayer::new(256, 256, Rgba([31, 63, 127, 0]));
    raster
        .blend_pixel(
            0,
            0,
            Rgba([1; 4]),
            BlendMode::Replace,
            &mut Admission::with_residency(65536, budget.clone()),
        )
        .unwrap();
    let retained = versions(&raster);
    assert_eq!(budget.usage().reserved, RasterLayer::TILE_BYTES as u64);
    drop(raster);
    assert_eq!(budget.usage().reserved, 0);
    let mut replacement = RasterLayer::new(256, 256, Rgba([31, 63, 127, 0]));
    replacement
        .blend_pixel(
            0,
            0,
            Rgba([2; 4]),
            BlendMode::Replace,
            &mut Admission::with_residency(65536, budget.clone()),
        )
        .unwrap();
    assert_ne!(retained[0], versions(&replacement)[0]);
}

#[test]
fn concurrent_revision_allocation_never_reuses_tokens() {
    let counter = AtomicU64::new(1);
    let mut revisions = std::thread::scope(|scope| {
        let tasks: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    (0..1024)
                        .map(|_| revision_from(&counter))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        tasks
            .into_iter()
            .flat_map(|t| t.join().unwrap())
            .collect::<Vec<_>>()
    });
    revisions.sort_unstable();
    assert_eq!(revisions, (1..=4096).collect::<Vec<_>>());
}
