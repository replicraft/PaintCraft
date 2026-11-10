//! Public admitted mapping against independent pre-source center-sampling RGBA literals.
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba, policy::BlendMode, stretch_skew::StretchSkew,
};
use paintcraft_raster::{Admission, MappedPlan, PaintSnapshot, RasterLayer, StretchSkewMapping};
use std::cell::Cell;
fn pixel(x: i64, y: i64) -> Rgba {
    Rgba([
        (10 + 73 * x) as u8,
        (20 + 79 * y) as u8,
        (33 + x + y) as u8,
        if x == y { 0 } else { 128 },
    ])
}
fn check(source: Rect, extent: Rect, size: (u32, u32), shear: (i32, i32), expected: &[[u8; 4]]) {
    let g = StretchSkew::new(source, size, shear).unwrap();
    let destination = g.bounds(extent).unwrap();
    let mut raster = RasterLayer::new(
        extent.width() as u32,
        extent.height() as u32,
        Rgba([7, 8, 9, 0]),
    );
    let mut source_admission = Admission::new(65536);
    for y in 0..extent.height() {
        for x in 0..extent.width() {
            raster
                .blend_pixel(x, y, pixel(x, y), BlendMode::Replace, &mut source_admission)
                .unwrap();
        }
    }
    let admission = Admission::new(65536);
    let snapshot = PaintSnapshot::capture(&raster, &admission, &|| Ok(())).unwrap();
    let plan = MappedPlan::prepare_stretch_skew(
        snapshot,
        StretchSkewMapping {
            transform: g,
            source_origin: (extent.x0, extent.y0),
            destination,
            selection: None,
            clear: Rgba::TRANSPARENT,
        },
        &admission,
        16777216,
        &|| Ok(()),
    )
    .unwrap();
    let result = plan.replay(&mut Admission::new(65536), &|| Ok(())).unwrap();
    assert_eq!(expected.len(), (result.width() * result.height()) as usize);
    for y in 0..result.height() {
        for x in 0..result.width() {
            assert_eq!(
                result.get(i64::from(x), i64::from(y)).0,
                expected[(y * result.width() + x) as usize]
            );
        }
    }
    for y in 0..extent.height() {
        for x in 0..extent.width() {
            assert_eq!(raster.get(x, y), pixel(x, y));
        }
    }
}
#[test]
fn all_fraction_oracles_preserve_hidden_rgba_and_offcanvas_samples() {
    // identity
    check(
        Rect::new(0, 0, 2, 2).unwrap(),
        Rect::new(0, 0, 2, 2).unwrap(),
        (2, 2),
        (0, 0),
        &[
            [10, 20, 33, 0],
            [83, 20, 34, 128],
            [10, 99, 34, 128],
            [83, 99, 35, 0],
        ],
    );
    // double
    check(
        Rect::new(0, 0, 2, 2).unwrap(),
        Rect::new(0, 0, 2, 2).unwrap(),
        (4, 4),
        (0, 0),
        &[
            [10, 20, 33, 0],
            [10, 20, 33, 0],
            [83, 20, 34, 128],
            [83, 20, 34, 128],
            [10, 20, 33, 0],
            [10, 20, 33, 0],
            [83, 20, 34, 128],
            [83, 20, 34, 128],
            [10, 99, 34, 128],
            [10, 99, 34, 128],
            [83, 99, 35, 0],
            [83, 99, 35, 0],
            [10, 99, 34, 128],
            [10, 99, 34, 128],
            [83, 99, 35, 0],
            [83, 99, 35, 0],
        ],
    );
    // halve
    check(
        Rect::new(0, 0, 4, 4).unwrap(),
        Rect::new(0, 0, 4, 4).unwrap(),
        (2, 2),
        (0, 0),
        &[
            [83, 99, 35, 0],
            [229, 99, 37, 128],
            [83, 1, 37, 128],
            [229, 1, 39, 0],
        ],
    );
    // horizontal45
    check(
        Rect::new(0, 0, 2, 2).unwrap(),
        Rect::new(0, 0, 2, 2).unwrap(),
        (2, 2),
        (65536, 0),
        &[
            [10, 20, 33, 0],
            [83, 20, 34, 128],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [10, 99, 34, 128],
            [83, 99, 35, 0],
            [0, 0, 0, 0],
        ],
    );
    // negative45
    check(
        Rect::new(0, 0, 2, 2).unwrap(),
        Rect::new(0, 0, 2, 2).unwrap(),
        (2, 2),
        (-65536, 0),
        &[
            [0, 0, 0, 0],
            [10, 20, 33, 0],
            [83, 20, 34, 128],
            [0, 0, 0, 0],
            [10, 99, 34, 128],
            [83, 99, 35, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        ],
    );
    // both45
    check(
        Rect::new(0, 0, 2, 2).unwrap(),
        Rect::new(0, 0, 2, 2).unwrap(),
        (2, 2),
        (65536, 65536),
        &[
            [10, 20, 33, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [83, 20, 34, 128],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [10, 99, 34, 128],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [83, 99, 35, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        ],
    );
    // offcanvas
    check(
        Rect::new(0, 0, 2, 2).unwrap(),
        Rect::new(-1, -1, 4, 4).unwrap(),
        (4, 4),
        (0, 0),
        &[
            [10, 20, 33, 0],
            [10, 20, 33, 0],
            [83, 20, 34, 128],
            [83, 20, 34, 128],
            [156, 20, 35, 128],
            [156, 20, 35, 128],
            [229, 20, 36, 128],
            [229, 20, 36, 128],
            [10, 20, 33, 0],
            [10, 20, 33, 0],
            [83, 20, 34, 128],
            [83, 20, 34, 128],
            [156, 20, 35, 128],
            [156, 20, 35, 128],
            [229, 20, 36, 128],
            [229, 20, 36, 128],
            [10, 99, 34, 128],
            [10, 99, 34, 128],
            [83, 99, 35, 0],
            [83, 99, 35, 0],
            [156, 99, 36, 128],
            [156, 99, 36, 128],
            [229, 99, 37, 128],
            [229, 99, 37, 128],
            [10, 99, 34, 128],
            [10, 99, 34, 128],
            [83, 99, 35, 0],
            [83, 99, 35, 0],
            [156, 99, 36, 128],
            [156, 99, 36, 128],
            [229, 99, 37, 128],
            [229, 99, 37, 128],
            [10, 178, 35, 128],
            [10, 178, 35, 128],
            [83, 178, 36, 128],
            [83, 178, 36, 128],
            [156, 178, 37, 0],
            [156, 178, 37, 0],
            [229, 178, 38, 128],
            [229, 178, 38, 128],
            [10, 178, 35, 128],
            [10, 178, 35, 128],
            [83, 178, 36, 128],
            [83, 178, 36, 128],
            [156, 178, 37, 0],
            [156, 178, 37, 0],
            [229, 178, 38, 128],
            [229, 178, 38, 128],
            [10, 1, 36, 128],
            [10, 1, 36, 128],
            [83, 1, 37, 128],
            [83, 1, 37, 128],
            [156, 1, 38, 128],
            [156, 1, 38, 128],
            [229, 1, 39, 0],
            [229, 1, 39, 0],
            [10, 1, 36, 128],
            [10, 1, 36, 128],
            [83, 1, 37, 128],
            [83, 1, 37, 128],
            [156, 1, 38, 128],
            [156, 1, 38, 128],
            [229, 1, 39, 0],
            [229, 1, 39, 0],
        ],
    );
    // selection-anchor
    check(
        Rect::new(3, 4, 2, 2).unwrap(),
        Rect::new(3, 4, 2, 2).unwrap(),
        (4, 2),
        (0, 0),
        &[
            [10, 20, 33, 0],
            [10, 20, 33, 0],
            [83, 20, 34, 128],
            [83, 20, 34, 128],
            [10, 99, 34, 128],
            [10, 99, 34, 128],
            [83, 99, 35, 0],
            [83, 99, 35, 0],
        ],
    );
}
#[test]
fn destination_limit_and_cancellation_leave_source_unchanged() {
    let raster = RasterLayer::new(2, 2, Rgba([9, 8, 7, 0]));
    let admission = Admission::new(65536);
    let g = StretchSkew::new(Rect::new(0, 0, 2, 2).unwrap(), (4, 4), (0, 0)).unwrap();
    let request = StretchSkewMapping {
        transform: g,
        source_origin: (0, 0),
        destination: g.bounds(g.source()).unwrap(),
        selection: None,
        clear: Rgba::TRANSPARENT,
    };
    let snapshot = PaintSnapshot::capture(&raster, &admission, &|| Ok(())).unwrap();
    assert_eq!(
        MappedPlan::prepare_stretch_skew(snapshot, request.clone(), &admission, 15, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    for cut in 1..=10 {
        let count = Cell::new(0);
        let check = || {
            count.set(count.get() + 1);
            if count.get() == cut {
                Err(Error::new(ErrorCode::OperationCancelled, "cut"))
            } else {
                Ok(())
            }
        };
        let snapshot = PaintSnapshot::capture(&raster, &admission, &|| Ok(())).unwrap();
        assert_eq!(
            MappedPlan::prepare_stretch_skew(snapshot, request.clone(), &admission, 16, &check)
                .unwrap_err()
                .code,
            ErrorCode::OperationCancelled
        );
        assert_eq!(raster.get(0, 0), Rgba([9, 8, 7, 0]));
    }
}
