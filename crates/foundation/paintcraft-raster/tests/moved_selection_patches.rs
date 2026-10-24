use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba, resources::SharedBudget, text::MaskSpan,
};
use paintcraft_raster::{
    Admission, BlendMode, MappedPlan, PaintSnapshot, RasterLayer,
    selection::{BinaryMask, MaskLimits},
};
use std::cell::Cell;
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn source() -> RasterLayer {
    let mut r = RasterLayer::new(8, 6, Rgba([11, 22, 33, 0]));
    for y in 0..6 {
        for x in 0..8 {
            if (x + y) % 3 != 0 {
                r.blend_pixel(
                    x,
                    y,
                    Rgba([
                        x as u8 * 23,
                        y as u8 * 31,
                        90,
                        if (x + y) % 2 == 0 { 0 } else { 128 },
                    ]),
                    BlendMode::Replace,
                    &mut Admission::new(65_536),
                )
                .unwrap();
            }
        }
    }
    for (x, y, c) in [
        (1, 1, Rgba([17, 33, 55, 0])),
        (2, 1, Rgba([17, 33, 55, 255])),
        (3, 2, Rgba([90, 80, 70, 0])),
    ] {
        r.blend_pixel(x, y, c, BlendMode::Replace, &mut Admission::new(65_536))
            .unwrap();
    }
    r
}
fn pixels(r: &RasterLayer) -> Vec<Vec<Rgba>> {
    (0..r.height())
        .map(|y| (0..r.width()).map(|x| r.get(x.into(), y.into())).collect())
        .collect()
}
fn field(area: Rect, budget: &SharedBudget) -> BinaryMask {
    let spans: Vec<_> = (area.y0..area.y1)
        .flat_map(|y| {
            (area.x0..area.x1)
                .filter(move |&x| (x + 2 * y).rem_euclid(3) != 1)
                .map(move |x| MaskSpan {
                    y,
                    x0: x,
                    x1: x,
                    coverage: 255,
                })
        })
        .collect();
    BinaryMask::from_spans(area, false, &spans, budget, MaskLimits::default(), &|| {
        Ok(())
    })
    .unwrap()
}
fn oracle(
    input: &RasterLayer,
    area: Rect,
    masked: bool,
    offset: (i64, i64),
    clear: Rgba,
    key: Option<Rgba>,
) -> (Vec<Vec<Rgba>>, (i64, i64)) {
    if offset == (0, 0) {
        return (pixels(input), (0, 0));
    }
    let selected =
        |x: i64, y: i64| area.contains(x, y) && (!masked || (x + 2 * y).rem_euclid(3) != 1);
    let target = area.translated(offset.0, offset.1).unwrap();
    let (ox, oy) = (0.min(target.x0), 0.min(target.y0));
    let w = i64::from(input.width()).max(target.x1) - ox;
    let h = i64::from(input.height()).max(target.y1) - oy;
    let mut result = vec![vec![Rgba::TRANSPARENT; w as usize]; h as usize];
    for y in 0..i64::from(input.height()) {
        for x in 0..i64::from(input.width()) {
            result[(y - oy) as usize][(x - ox) as usize] = if selected(x, y) {
                clear
            } else {
                input.get(x, y)
            };
        }
    }
    for y in area.y0..area.y1 {
        for x in area.x0..area.x1 {
            if !selected(x, y) {
                continue;
            }
            let color = if input.bounds().contains(x, y) {
                input.get(x, y)
            } else {
                Rgba::TRANSPARENT
            };
            if key.is_some_and(|key| key.0[..3] == color.0[..3]) {
                continue;
            }
            result[(y + offset.1 - oy) as usize][(x + offset.0 - ox) as usize] = color;
        }
    }
    (result, (ox, oy))
}
#[test]
fn frozen_moves_match_independent_overlap_holes_finite_and_rgb_key_oracles() {
    let input = source();
    let before = pixels(&input);
    let zero = SharedBudget::new(0);
    let admission = Admission::with_residency(0, zero.clone());
    for area in [rect(1, 1, 4, 3), rect(-2, 4, 5, 3), rect(11, -3, 3, 2)] {
        for offset in [(0, 0), (-3, -2), (-1, 0), (1, 1), (9, -1), (-7, 3)] {
            for masked in [false, true] {
                for key in [
                    None,
                    Some(Rgba([11, 22, 33, 255])),
                    Some(Rgba([17, 33, 55, 128])),
                    Some(Rgba([0, 0, 0, 200])),
                ] {
                    for clear in [Rgba::TRANSPARENT, Rgba([255; 4])] {
                        let mask = masked.then(|| field(area, &SharedBudget::new(1 << 20)));
                        let expected = oracle(&input, area, masked, offset, clear, key);
                        let (plan, origin) = MappedPlan::prepare_moved_patch(
                            PaintSnapshot::capture(&input, &admission, &|| Ok(())).unwrap(),
                            area,
                            mask,
                            offset,
                            clear,
                            key,
                            &admission,
                            100_000,
                            &|| Ok(()),
                        )
                        .unwrap();
                        assert_eq!(origin, expected.1);
                        let snapshot = plan.snapshot(&admission, &|| Ok(())).unwrap();
                        let reader = snapshot.reader(&admission, &|| Ok(())).unwrap();
                        for (y, row) in expected.0.iter().enumerate() {
                            for (x, color) in row.iter().enumerate() {
                                assert_eq!(
                                    reader.read_checked(x as i64, y as i64, &|| Ok(())).unwrap(),
                                    *color,
                                    "logical {area:?} {offset:?} masked={masked} key={key:?} {x},{y}"
                                );
                            }
                        }
                        assert_eq!(
                            pixels(
                                &plan
                                    .replay(&mut Admission::new(1_000_000), &|| Ok(()))
                                    .unwrap()
                            ),
                            expected.0,
                            "replay {area:?} {offset:?} {key:?}"
                        );
                    }
                }
            }
        }
    }
    assert_eq!(pixels(&input), before);
    assert_eq!(zero.usage().peak, 0);
}
#[test]
fn zero_displacement_is_exact_identity_even_for_keys_off_layer_areas_and_zero_read_quota() {
    let input = source();
    let admission = Admission::new(0);
    for area in [rect(-5, -4, 20, 16), rect(100, 200, 4, 3)] {
        for key in [None, Some(Rgba([17, 33, 55, 0]))] {
            let (plan, origin) = MappedPlan::prepare_moved_patch(
                PaintSnapshot::capture(&input, &admission, &|| Ok(())).unwrap(),
                area,
                Some(field(area, &SharedBudget::new(1 << 20))),
                (0, 0),
                Rgba([255; 4]),
                key,
                &admission,
                0,
                &|| Ok(()),
            )
            .unwrap();
            assert_eq!(origin, (0, 0));
            assert_eq!(plan.source_reads(), 0);
            assert_eq!(plan.dimensions(), (8, 6));
            assert_eq!(
                pixels(
                    &plan
                        .replay(&mut Admission::new(65_536), &|| Ok(()))
                        .unwrap()
                ),
                pixels(&input)
            );
        }
    }
}
#[test]
fn key_rgb_ignores_alpha_and_only_filters_paste_while_cut_always_clears() {
    let input = source();
    let admission = Admission::new(0);
    let area = rect(1, 1, 3, 2);
    let mut outputs = Vec::new();
    for alpha in [0, 1, 128, 255] {
        let (plan, _) = MappedPlan::prepare_moved_patch(
            PaintSnapshot::capture(&input, &admission, &|| Ok(())).unwrap(),
            area,
            None,
            (2, 1),
            Rgba([255; 4]),
            Some(Rgba([17, 33, 55, alpha])),
            &admission,
            100_000,
            &|| Ok(()),
        )
        .unwrap();
        let output = plan
            .replay(&mut Admission::new(65_536), &|| Ok(()))
            .unwrap();
        assert_eq!(output.get(1, 1), Rgba([255; 4]));
        assert_eq!(output.get(2, 1), Rgba([255; 4]));
        assert_eq!(
            output.get(3, 2),
            Rgba([255; 4]),
            "keyed paste leaves the original cut at the overlap"
        );
        assert_eq!(
            output.get(5, 3),
            Rgba([90, 80, 70, 0]),
            "non-key hidden RGB remains exact"
        );
        outputs.push(pixels(&output));
    }
    assert!(outputs.windows(2).all(|pair| pair[0] == pair[1]));
}
#[test]
fn sparse_tile_seams_and_logical_moves_read_prior_unmaterialized_writes() {
    let mut input = RasterLayer::new(530, 265, Rgba([11, 22, 33, 0]));
    for (x, y, c) in [
        (254, 254, Rgba([17, 33, 55, 0])),
        (257, 255, Rgba([1, 2, 3, 128])),
        (520, 260, Rgba([90, 80, 70, 0])),
    ] {
        input
            .blend_pixel(x, y, c, BlendMode::Replace, &mut Admission::new(1_000_000))
            .unwrap();
    }
    let admission = Admission::with_residency(0, SharedBudget::new(0));
    let area = rect(254, 254, 7, 5);
    let key = Some(Rgba([17, 33, 55, 255]));
    let (plan, origin) = MappedPlan::prepare_moved_patch(
        PaintSnapshot::capture(&input, &admission, &|| Ok(())).unwrap(),
        area,
        Some(field(area, &SharedBudget::new(1 << 20))),
        (-2, 3),
        Rgba([255; 4]),
        key,
        &admission,
        1_000_000,
        &|| Ok(()),
    )
    .unwrap();
    let expected = oracle(&input, area, true, (-2, 3), Rgba([255; 4]), key);
    assert_eq!(origin, expected.1);
    assert_eq!(
        pixels(
            &plan
                .replay(&mut Admission::new(1_000_000), &|| Ok(()))
                .unwrap()
        ),
        expected.0
    );

    let input = source();
    let mut first = admission.paint_plan();
    first
        .rect(rect(1, 1, 1, 1), Rgba([20, 30, 40, 0]), BlendMode::Replace)
        .unwrap();
    let logical = PaintSnapshot::capture(&input, &admission, &|| Ok(()))
        .unwrap()
        .fork(&admission, &|| Ok(()))
        .unwrap()
        .append_checked(first, &|| Ok(()))
        .unwrap()
        .freeze_checked(&admission, &|| Ok(()))
        .unwrap();
    let (plan, _) = MappedPlan::prepare_moved_patch(
        logical,
        rect(1, 1, 2, 1),
        None,
        (3, 2),
        Rgba::TRANSPARENT,
        Some(Rgba([17, 33, 55, 0])),
        &admission,
        100_000,
        &|| Ok(()),
    )
    .unwrap();
    let view = plan.snapshot(&admission, &|| Ok(())).unwrap();
    let reader = view.reader(&admission, &|| Ok(())).unwrap();
    assert_eq!(
        reader.read_checked(4, 3, &|| Ok(())).unwrap(),
        Rgba([20, 30, 40, 0]),
        "frozen key gate uses earlier logical paint, not original raster"
    );
    assert_eq!(
        reader.read_checked(5, 3, &|| Ok(())).unwrap(),
        input.get(5, 3),
        "original keyed neighbor is not pasted"
    );
}
#[test]
fn classification_and_replay_cancellation_quotas_and_retained_paste_fields_are_bounded() {
    let input = source();
    let area = rect(1, 1, 4, 3);
    let selection = SharedBudget::new(1 << 20);
    let metadata = SharedBudget::new(8 << 20);
    let admission = Admission::with_residency(0, SharedBudget::new(0))
        .with_selection_budget(selection.clone())
        .with_paint_metadata_budget(metadata.clone());
    let snapshot = PaintSnapshot::capture(&input, &admission, &|| Ok(())).unwrap();
    let key = Some(Rgba([17, 33, 55, 128]));
    let calls = Cell::new(0);
    let prepare = |check: &dyn Fn() -> paintcraft_core::types::Result<()>| {
        MappedPlan::prepare_moved_patch(
            snapshot.clone(),
            area,
            None,
            (1, 1),
            Rgba([255; 4]),
            key,
            &admission,
            100_000,
            check,
        )
    };
    let before = metadata.usage().reserved;
    let (plan, _) = prepare(&|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert!(plan.source_reads() >= area.area() as u64);
    assert!(selection.usage().reserved > 0);
    let retained = plan.snapshot(&admission, &|| Ok(())).unwrap();
    drop(plan);
    assert!(selection.usage().reserved > 0);
    drop(retained);
    assert_eq!(selection.usage().reserved, 0);
    assert_eq!(metadata.usage().reserved, before);
    for at in 1..=calls.get() {
        let observed = Cell::new(0);
        let err = prepare(&|| {
            observed.set(observed.get() + 1);
            if observed.get() == at {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel moved patch",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::OperationCancelled);
        assert_eq!(selection.usage().reserved, 0);
        assert_eq!(metadata.usage().reserved, before);
    }
    for limit in [0, area.area() as u64 - 1, area.area() as u64] {
        assert_eq!(
            MappedPlan::prepare_moved_patch(
                snapshot.clone(),
                area,
                None,
                (1, 1),
                Rgba([255; 4]),
                key,
                &admission,
                limit,
                &|| Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(selection.usage().reserved, 0);
    }
    let denied = Admission::new(0).with_selection_budget(SharedBudget::new(0));
    assert_eq!(
        MappedPlan::prepare_moved_patch(
            snapshot.clone(),
            area,
            None,
            (1, 1),
            Rgba([255; 4]),
            key,
            &denied,
            100_000,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    let denied = Admission::new(0).with_paint_metadata_budget(SharedBudget::new(0));
    assert_eq!(
        MappedPlan::prepare_moved_patch(
            snapshot.clone(),
            area,
            None,
            (1, 1),
            Rgba([255; 4]),
            key,
            &denied,
            100_000,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    for offset in [(i64::MAX, 0), (0, i64::MIN)] {
        assert!(
            MappedPlan::prepare_moved_patch(
                snapshot.clone(),
                area,
                None,
                offset,
                Rgba::TRANSPARENT,
                None,
                &admission,
                100_000,
                &|| Ok(())
            )
            .is_err()
        );
    }
    let (plan, _) = prepare(&|| Ok(())).unwrap();
    assert_eq!(
        plan.replay(&mut Admission::new(0), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let resident = SharedBudget::new(RasterLayer::TILE_BYTES as u64);
    let count = Cell::new(0);
    drop(
        plan.replay(
            &mut Admission::with_residency(65_536, resident.clone()),
            &|| {
                count.set(count.get() + 1);
                Ok(())
            },
        )
        .unwrap(),
    );
    for at in 1..=count.get() {
        let n = Cell::new(0);
        assert_eq!(
            plan.replay(
                &mut Admission::with_residency(65_536, resident.clone()),
                &|| {
                    n.set(n.get() + 1);
                    if n.get() == at {
                        Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "cancel moved replay",
                        ))
                    } else {
                        Ok(())
                    }
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
        assert_eq!(resident.usage().reserved, 0);
    }
    drop(plan);
    drop(snapshot);
    assert_eq!(selection.usage().reserved, 0);
    assert_eq!(metadata.usage().reserved, 0);
}
