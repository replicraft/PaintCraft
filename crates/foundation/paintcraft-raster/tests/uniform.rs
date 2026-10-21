use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba};
use paintcraft_raster::{Admission, BlendMode, RasterLayer, TileCoord};

#[test]
fn uniform_proofs_preserve_cow_initial_colors_and_skip_absent_sparse_tiles() {
    let red = Rgba([91, 37, 13, 0]);
    let mut source = RasterLayer::new(1_048_576, 1_048_576, red);
    assert_eq!(
        source.uniform_region(source.bounds(), &|| Ok(())).unwrap(),
        Some(red)
    );
    let mut admission = Admission::new(4 * 65536);
    source
        .blend_pixel(
            7,
            9,
            Rgba([1, 2, 3, 255]),
            BlendMode::Replace,
            &mut admission,
        )
        .unwrap();
    let snapshot = source.clone();
    assert_eq!(
        source.uniform_region(source.bounds(), &|| Ok(())).unwrap(),
        None
    );
    assert_eq!(
        source
            .uniform_region(Rect::new(64, 64, 900000, 900000).unwrap(), &|| Ok(()))
            .unwrap(),
        Some(red)
    );
    assert_eq!(
        source
            .uniform_region(Rect::new(63, 9, 2, 1).unwrap(), &|| Ok(()))
            .unwrap(),
        None
    );
    assert_eq!(
        source
            .uniform_region(Rect::new(64, 9, 1, 1).unwrap(), &|| Ok(()))
            .unwrap(),
        Some(red)
    );
    let lease = source.tile_leases().next().unwrap().1;
    let mut other = RasterLayer::new(512, 512, Rgba([2, 4, 6, 255]));
    other
        .install_tile(TileCoord { tx: 0, ty: 0 }, lease)
        .unwrap();
    assert_eq!(
        other
            .uniform_region(Rect::new(64, 64, 128, 128).unwrap(), &|| Ok(()))
            .unwrap(),
        Some(red)
    );
    assert_eq!(
        other
            .uniform_region(Rect::new(64, 64, 400, 400).unwrap(), &|| Ok(()))
            .unwrap(),
        None
    );
    source
        .blend_pixel(
            65,
            65,
            Rgba([4, 5, 6, 7]),
            BlendMode::Replace,
            &mut admission,
        )
        .unwrap();
    assert_eq!(
        source
            .uniform_region(Rect::new(64, 64, 1, 1).unwrap(), &|| Ok(()))
            .unwrap(),
        None
    );
    assert_eq!(
        snapshot
            .uniform_region(Rect::new(64, 64, 1, 1).unwrap(), &|| Ok(()))
            .unwrap(),
        Some(red)
    );
}

#[test]
fn uniform_queries_validate_bounds_and_propagate_cancellation() {
    let raster = RasterLayer::new(17, 19, Rgba([1, 2, 3, 4]));
    for rect in [
        Rect::new(-1, 0, 1, 1).unwrap(),
        Rect::new(17, 0, 1, 1).unwrap(),
        Rect {
            x0: 0,
            y0: 0,
            x1: 0,
            y1: 1,
        },
    ] {
        assert_eq!(
            raster.uniform_region(rect, &|| Ok(())).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
    assert_eq!(
        raster
            .uniform_region(raster.bounds(), &|| Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel proof"
            )))
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
}
