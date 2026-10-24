use paintcraft_core::types::{
    ErrorCode, Rect, Rgba, orthogonal::OrthogonalTransform as T, resources::SharedBudget,
};
use paintcraft_raster::{Admission, BlendMode, CropPlan, MappedPlan, PaintSnapshot, RasterLayer};
const ALL: [T; 5] = [
    T::RotateRight,
    T::RotateLeft,
    T::Rotate180,
    T::FlipHorizontal,
    T::FlipVertical,
];
// Independent dense row/column operations; does not call the production mapping.
fn orient(input: Vec<Vec<Rgba>>, t: T) -> Vec<Vec<Rgba>> {
    match t {
        T::FlipHorizontal => input
            .into_iter()
            .map(|r| r.into_iter().rev().collect())
            .collect(),
        T::FlipVertical => input.into_iter().rev().collect(),
        T::Rotate180 => input
            .into_iter()
            .rev()
            .map(|r| r.into_iter().rev().collect())
            .collect(),
        T::RotateRight => (0..input[0].len())
            .map(|x| input.iter().rev().map(|r| r[x]).collect())
            .collect(),
        T::RotateLeft => (0..input[0].len())
            .rev()
            .map(|x| input.iter().map(|r| r[x]).collect())
            .collect(),
    }
}
fn rows(r: &RasterLayer) -> Vec<Vec<Rgba>> {
    (0..r.height())
        .map(|y| (0..r.width()).map(|x| r.get(x.into(), y.into())).collect())
        .collect()
}
fn source(w: u32, h: u32) -> RasterLayer {
    let mut r = RasterLayer::new(w, h, Rgba([1, 2, 3, 0]));
    let mut a = Admission::new(20 * 65536);
    for (x, y, c) in [
        (0, 0, Rgba([200, 100, 50, 0])),
        (1, 1, Rgba([0, 0, 0, 255])),
        (w - 1, h - 1, Rgba([20, 40, 80, 128])),
        (w / 2, h / 2, Rgba([255; 4])),
    ] {
        r.blend_pixel(x.into(), y.into(), c, BlendMode::Replace, &mut a)
            .unwrap();
    }
    r
}
#[test]
fn every_orientation_matches_dense_oracle_across_unaligned_tiles_and_one_pixel_axes() {
    for (w, h) in [(3, 2), (259, 257), (1, 5), (5, 1), (1, 1)] {
        let r = source(w, h);
        for t in ALL {
            let a = Admission::with_residency(20 * 65536, SharedBudget::new(0));
            let plan = MappedPlan::prepare_transform(
                PaintSnapshot::capture(&r, &a, &|| Ok(())).unwrap(),
                t,
                &a,
                u64::MAX,
                &|| Ok(()),
            )
            .unwrap();
            assert_eq!(a.residency_usage().peak, 0);
            let snapshot = plan.snapshot(&a, &|| Ok(())).unwrap();
            let reader = snapshot.reader(&a, &|| Ok(())).unwrap();
            let result = plan
                .replay(&mut Admission::new(20 * 65536), &|| Ok(()))
                .unwrap();
            let expected = orient(rows(&r), t);
            assert_eq!(rows(&result), expected, "{w}x{h} {t:?}");
            for (y, row) in expected.iter().enumerate() {
                for (x, c) in row.iter().enumerate() {
                    assert_eq!(
                        reader.read_checked(x as i64, y as i64, &|| Ok(())).unwrap(),
                        *c
                    );
                }
            }
            assert_eq!(result.default_pixel(), r.default_pixel());
            assert_eq!(
                plan.replay(&mut a.clone(), &|| Ok(())).is_err(),
                result.resident_tiles() > 0
            );
        }
    }
}
#[test]
fn composed_mappings_paint_crop_and_expansion_preserve_order_and_padding() {
    let r = source(9, 7);
    let mut expected = rows(&r);
    let a = Admission::with_residency(30 * 65536, SharedBudget::new(0));
    let mut snapshot = PaintSnapshot::capture(&r, &a, &|| Ok(())).unwrap();
    for t in ALL.into_iter().cycle().take(15) {
        let p = MappedPlan::prepare_transform(snapshot, t, &a, u64::MAX, &|| Ok(())).unwrap();
        snapshot = p.snapshot(&a, &|| Ok(())).unwrap();
        expected = orient(expected, t);
    }
    let mut paint = a.paint_plan();
    let c = Rgba([70, 80, 90, 255]);
    paint
        .rect(Rect::new(1, 1, 2, 2).unwrap(), c, BlendMode::Replace)
        .unwrap();
    for row in expected.iter_mut().skip(1).take(2) {
        for pixel in row.iter_mut().skip(1).take(2) {
            *pixel = c;
        }
    }
    snapshot = snapshot
        .fork(&a, &|| Ok(()))
        .unwrap()
        .append_checked(paint, &|| Ok(()))
        .unwrap()
        .freeze_checked(&a, &|| Ok(()))
        .unwrap();
    let crop = CropPlan::prepare(snapshot, Rect::new(1, 1, 4, 3).unwrap(), &a, &|| Ok(())).unwrap();
    let cropped = crop.snapshot(&a, &|| Ok(())).unwrap();
    let expanded = cropped
        .fork_expanded(8, 8, &a, &|| Ok(()))
        .unwrap()
        .freeze_checked(&a, &|| Ok(()))
        .unwrap();
    let t =
        MappedPlan::prepare_transform(expanded, T::RotateLeft, &a, u64::MAX, &|| Ok(())).unwrap();
    let result = t
        .replay(&mut Admission::new(30 * 65536), &|| Ok(()))
        .unwrap();
    let mut padded = vec![vec![r.default_pixel(); 8]; 8];
    for y in 0..3 {
        for x in 0..4 {
            padded[y][x] = expected[y + 1][x + 1];
        }
    }
    assert_eq!(rows(&result), orient(padded, T::RotateLeft));
    assert_eq!(a.residency_usage().peak, 0);
}
#[test]
fn uniform_huge_sources_require_no_samples_and_sample_limits_fail_before_decode() {
    let a = Admission::with_residency(0, SharedBudget::new(0));
    let r = RasterLayer::new(900_000, 800_000, Rgba([4, 5, 6, 0]));
    let p = MappedPlan::prepare_transform(
        PaintSnapshot::capture(&r, &a, &|| Ok(())).unwrap(),
        T::RotateRight,
        &a,
        0,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(p.source_reads(), 0);
    let out = p.replay(&mut a.clone(), &|| Ok(())).unwrap();
    assert_eq!((out.width(), out.height()), (800_000, 900_000));
    assert_eq!(out.resident_tiles(), 0);
    let r = source(3, 2);
    let err = MappedPlan::prepare_transform(
        PaintSnapshot::capture(&r, &a, &|| Ok(())).unwrap(),
        T::FlipVertical,
        &a,
        0,
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourceLimit);
    assert_eq!(a.residency_usage().peak, 0);
}

#[test]
fn cancellation_during_mapping_and_replay_releases_private_storage() {
    use paintcraft_core::types::Error;
    use std::cell::Cell;
    let r = source(259, 257);
    let a = Admission::with_residency(20 * 65536, SharedBudget::new(0));
    let snapshot = PaintSnapshot::capture(&r, &a, &|| Ok(())).unwrap();
    let count = Cell::new(0);
    let check = || {
        count.set(count.get() + 1);
        if count.get() > 1000 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "test cancellation",
            ))
        } else {
            Ok(())
        }
    };
    let err = MappedPlan::prepare_transform(snapshot.clone(), T::RotateRight, &a, u64::MAX, &check)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::OperationCancelled);
    assert_eq!(a.residency_usage().peak, 0);
    let plan =
        MappedPlan::prepare_transform(snapshot, T::RotateRight, &a, u64::MAX, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(20 * 65536 * 4);
    let mut replay = Admission::with_residency(20 * 65536, budget.clone());
    count.set(0);
    assert_eq!(
        plan.replay(&mut replay, &check).unwrap_err().code,
        ErrorCode::OperationCancelled
    );
    assert!(budget.usage().peak > 0);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(r.get(0, 0), Rgba([200, 100, 50, 0]));
}
