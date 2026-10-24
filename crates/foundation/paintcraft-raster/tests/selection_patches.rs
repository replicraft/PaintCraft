use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba, orthogonal::OrthogonalTransform as T, resources::SharedBudget,
};
use paintcraft_raster::{Admission, BlendMode, CropPlan, MappedPlan, PaintSnapshot, RasterLayer};
const ALL: [T; 5] = [
    T::RotateRight,
    T::RotateLeft,
    T::Rotate180,
    T::FlipHorizontal,
    T::FlipVertical,
];
fn rotate(a: Vec<Vec<Rgba>>, t: T) -> Vec<Vec<Rgba>> {
    match t {
        T::RotateRight => (0..a[0].len())
            .map(|x| a.iter().rev().map(|r| r[x]).collect())
            .collect(),
        T::RotateLeft => (0..a[0].len())
            .rev()
            .map(|x| a.iter().map(|r| r[x]).collect())
            .collect(),
        T::Rotate180 => a
            .into_iter()
            .rev()
            .map(|r| r.into_iter().rev().collect())
            .collect(),
        T::FlipHorizontal => a
            .into_iter()
            .map(|r| r.into_iter().rev().collect())
            .collect(),
        T::FlipVertical => a.into_iter().rev().collect(),
    }
}
fn source() -> RasterLayer {
    let mut r = RasterLayer::new(8, 6, Rgba([11, 22, 33, 0]));
    let mut a = Admission::new(65536);
    for y in 0..6 {
        for x in 0..8 {
            r.blend_pixel(
                x,
                y,
                Rgba([
                    x as u8 * 20,
                    y as u8 * 30,
                    90,
                    if (x + y) % 2 == 0 { 0 } else { 128 },
                ]),
                BlendMode::Replace,
                &mut a,
            )
            .unwrap();
        }
    }
    r
}
fn dense(r: &RasterLayer) -> Vec<Vec<Rgba>> {
    (0..r.height())
        .map(|y| (0..r.width()).map(|x| r.get(x.into(), y.into())).collect())
        .collect()
}
fn patch(input: Vec<Vec<Rgba>>, area: Rect, t: T, clear: Rgba) -> (Vec<Vec<Rgba>>, (i64, i64)) {
    let old = Rect::new(0, 0, input[0].len() as i64, input.len() as i64).unwrap();
    let selected = (area.y0..area.y1)
        .map(|y| {
            (area.x0..area.x1)
                .map(|x| {
                    if old.contains(x, y) {
                        input[y as usize][x as usize]
                    } else {
                        Rgba::TRANSPARENT
                    }
                })
                .collect()
        })
        .collect();
    let selected = rotate(selected, t);
    let dx = area.x0;
    let dy = area.y0;
    let (ox, oy) = (0.min(dx), 0.min(dy));
    let w = old.x1.max(dx + selected[0].len() as i64) - ox;
    let h = old.y1.max(dy + selected.len() as i64) - oy;
    let mut output = vec![vec![Rgba::TRANSPARENT; w as usize]; h as usize];
    for (y, row) in input.iter().enumerate() {
        for (x, p) in row.iter().enumerate() {
            output[(y as i64 - oy) as usize][(x as i64 - ox) as usize] =
                if area.contains(x as i64, y as i64) {
                    clear
                } else {
                    *p
                };
        }
    }
    for (y, row) in selected.iter().enumerate() {
        for (x, p) in row.iter().enumerate() {
            output[(dy + y as i64 - oy) as usize][(dx + x as i64 - ox) as usize] = *p;
        }
    }
    (output, (ox, oy))
}
#[test]
fn all_orientations_overlap_clear_and_expand_against_independent_dense_oracle() {
    let r = source();
    let a = Admission::with_residency(20 * 65536, SharedBudget::new(0));
    for t in ALL {
        for area in [
            Rect::new(2, 1, 3, 2).unwrap(),
            Rect::new(-2, 3, 5, 3).unwrap(),
            Rect::new(7, -1, 2, 4).unwrap(),
            Rect::new(20, 20, 2, 3).unwrap(),
        ] {
            for clear in [Rgba([255; 4]), Rgba::TRANSPARENT] {
                let (expected, origin) = patch(dense(&r), area, t, clear);
                let (plan, offset) = MappedPlan::prepare_patch(
                    PaintSnapshot::capture(&r, &a, &|| Ok(())).unwrap(),
                    area,
                    t,
                    clear,
                    &a,
                    u64::MAX,
                    &|| Ok(()),
                )
                .unwrap();
                assert_eq!(offset, origin);
                let s = plan.snapshot(&a, &|| Ok(())).unwrap();
                let reader = s.reader(&a, &|| Ok(())).unwrap();
                let out = plan
                    .replay(&mut Admission::new(30 * 65536), &|| Ok(()))
                    .unwrap();
                assert_eq!(dense(&out), expected, "{t:?} {area:?}");
                for (y, row) in expected.iter().enumerate() {
                    for (x, p) in row.iter().enumerate() {
                        assert_eq!(
                            reader.read_checked(x as i64, y as i64, &|| Ok(())).unwrap(),
                            *p
                        );
                    }
                }
            }
        }
    }
    assert_eq!(a.residency_usage().peak, 0);
}
#[test]
fn conditional_lineages_compose_with_paint_rotations_crop_and_expansion_without_decoding() {
    let r = source();
    let mut expected = dense(&r);
    let a = Admission::with_residency(40 * 65536, SharedBudget::new(0));
    let mut snapshot = PaintSnapshot::capture(&r, &a, &|| Ok(())).unwrap();
    for (i, t) in ALL.into_iter().cycle().take(12).enumerate() {
        let area = Rect::new(2, 1, 3, 2).unwrap();
        let (p, offset) =
            MappedPlan::prepare_patch(snapshot, area, t, Rgba::TRANSPARENT, &a, u64::MAX, &|| {
                Ok(())
            })
            .unwrap();
        assert_eq!(offset, (0, 0));
        expected = patch(expected, area, t, Rgba::TRANSPARENT).0;
        snapshot = p.snapshot(&a, &|| Ok(())).unwrap();
        let mut paint = a.paint_plan();
        let color = Rgba([i as u8, 30, 60, 128]);
        paint
            .rect(Rect::new(0, 0, 1, 1).unwrap(), color, BlendMode::Replace)
            .unwrap();
        expected[0][0] = color;
        snapshot = snapshot
            .fork(&a, &|| Ok(()))
            .unwrap()
            .append_checked(paint, &|| Ok(()))
            .unwrap()
            .freeze_checked(&a, &|| Ok(()))
            .unwrap();
        let p = MappedPlan::prepare_transform(snapshot, T::FlipVertical, &a, u64::MAX, &|| Ok(()))
            .unwrap();
        expected = rotate(expected, T::FlipVertical);
        snapshot = p.snapshot(&a, &|| Ok(())).unwrap();
    }
    let p = CropPlan::prepare(snapshot, Rect::new(1, 1, 5, 4).unwrap(), &a, &|| Ok(())).unwrap();
    let out = p
        .replay(&mut Admission::new(40 * 65536), &|| Ok(()))
        .unwrap();
    assert_eq!(
        dense(&out),
        expected
            .into_iter()
            .skip(1)
            .take(4)
            .map(|r| r.into_iter().skip(1).take(5).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
    let snapshot = p
        .snapshot(&a, &|| Ok(()))
        .unwrap()
        .fork_expanded(10, 10, &a, &|| Ok(()))
        .unwrap()
        .freeze_checked(&a, &|| Ok(()))
        .unwrap();
    let reader = snapshot.reader(&a, &|| Ok(())).unwrap();
    assert_eq!(
        reader.read_checked(8, 8, &|| Ok(())).unwrap(),
        r.default_pixel()
    );
    assert_eq!(a.residency_usage().peak, 0);
}
#[test]
fn sparse_uniform_patch_limits_cancellation_and_extreme_coordinates_are_safe() {
    let a = Admission::with_residency(0, SharedBudget::new(0));
    let r = RasterLayer::new(900_000, 800_000, Rgba([255; 4]));
    let s = PaintSnapshot::capture(&r, &a, &|| Ok(())).unwrap();
    let (p, _) = MappedPlan::prepare_patch(
        s.clone(),
        Rect::new(10, 20, 3, 2).unwrap(),
        T::RotateRight,
        Rgba([255; 4]),
        &a,
        0,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(p.source_reads(), 0);
    assert_eq!(
        p.replay(&mut a.clone(), &|| Ok(()))
            .unwrap()
            .resident_tiles(),
        0
    );
    assert_eq!(
        MappedPlan::prepare_patch(
            s,
            Rect::new(i64::MIN, 0, 3, 2).unwrap(),
            T::RotateRight,
            Rgba::TRANSPARENT,
            &a,
            0,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
    let r = source();
    let s = PaintSnapshot::capture(&r, &a, &|| Ok(())).unwrap();
    assert_eq!(
        MappedPlan::prepare_patch(
            s.clone(),
            Rect::new(1, 1, 3, 2).unwrap(),
            T::RotateRight,
            Rgba::TRANSPARENT,
            &a,
            0,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    let count = std::cell::Cell::new(0);
    let check = || {
        count.set(count.get() + 1);
        if count.get() > 40 {
            Err(Error::new(ErrorCode::OperationCancelled, "test"))
        } else {
            Ok(())
        }
    };
    assert_eq!(
        MappedPlan::prepare_patch(
            s,
            Rect::new(1, 1, 3, 2).unwrap(),
            T::RotateRight,
            Rgba::TRANSPARENT,
            &a,
            10000,
            &check
        )
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(a.residency_usage().peak, 0);
}
