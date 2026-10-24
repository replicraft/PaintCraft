use paintcraft_core::types::{
    Error, ErrorCode, Rect, Result, Rgba, orthogonal::OrthogonalTransform as T,
    resources::SharedBudget, text::MaskSpan,
};
use paintcraft_raster::{
    Admission, BlendMode, MappedPlan, PaintSnapshot, RasterLayer,
    selection::{BinaryMask, MaskLimits},
};
use std::cell::Cell;
const ALL: [T; 5] = [
    T::RotateRight,
    T::RotateLeft,
    T::Rotate180,
    T::FlipHorizontal,
    T::FlipVertical,
];
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn orient<V: Copy>(input: &[Vec<V>], t: T) -> Vec<Vec<V>> {
    match t {
        T::RotateRight => (0..input[0].len())
            .map(|x| input.iter().rev().map(|row| row[x]).collect())
            .collect(),
        T::RotateLeft => (0..input[0].len())
            .rev()
            .map(|x| input.iter().map(|row| row[x]).collect())
            .collect(),
        T::Rotate180 => input
            .iter()
            .rev()
            .map(|row| row.iter().rev().copied().collect())
            .collect(),
        T::FlipHorizontal => input
            .iter()
            .map(|row| row.iter().rev().copied().collect())
            .collect(),
        T::FlipVertical => input.iter().rev().cloned().collect(),
    }
}
fn source() -> RasterLayer {
    let mut raster = RasterLayer::new(8, 6, Rgba([11, 22, 33, 0]));
    let mut admission = Admission::new(65_536);
    for y in 0..6 {
        for x in 0..8 {
            if (x + 2 * y) % 3 != 0 {
                raster
                    .blend_pixel(
                        x,
                        y,
                        Rgba([
                            x as u8 * 20,
                            y as u8 * 30,
                            90,
                            if (x + y) % 2 == 0 { 0 } else { 128 },
                        ]),
                        BlendMode::Replace,
                        &mut admission,
                    )
                    .unwrap();
            }
        }
    }
    raster
}
fn pixels(source: &RasterLayer) -> Vec<Vec<Rgba>> {
    (0..source.height())
        .map(|y| {
            (0..source.width())
                .map(|x| source.get(x.into(), y.into()))
                .collect()
        })
        .collect()
}
fn selected(area: Rect, domain: Rect, inverted: bool, x: i64, y: i64) -> bool {
    domain.contains(x, y) && (((x - area.x0 + 2 * (y - area.y0)) % 3 != 1) ^ inverted)
}
fn field(area: Rect, clipped: bool, inverted: bool, budget: &SharedBudget) -> (BinaryMask, Rect) {
    // Exercise the caller contract: a document field translated to layer-local.
    let origin = (-19, 23);
    let document = area.translated(origin.0, origin.1).unwrap();
    let spans: Vec<_> = (area.y0..area.y1)
        .flat_map(|y| {
            (area.x0..area.x1)
                .filter(move |&x| selected(area, area, false, x, y))
                .map(move |x| MaskSpan {
                    y: y + origin.1,
                    x0: x + origin.0,
                    x1: x + origin.0,
                    coverage: 255,
                })
        })
        .collect();
    let mask = BinaryMask::from_spans(
        document,
        false,
        &spans,
        budget,
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap()
    .translated(-origin.0, -origin.1)
    .unwrap();
    let domain = if clipped {
        rect(area.x0, area.y0 + 1, area.width() - 1, area.height() - 1)
    } else {
        area
    };
    let mask = mask.clipped(domain).unwrap().unwrap();
    (if inverted { mask.inverted() } else { mask }, domain)
}
fn oracle(
    input: &RasterLayer,
    area: Rect,
    domain: Rect,
    inverted: bool,
    t: T,
    clear: Rgba,
) -> (Vec<Vec<Rgba>>, (i64, i64)) {
    let extract: Vec<Vec<Option<Rgba>>> = (area.y0..area.y1)
        .map(|y| {
            (area.x0..area.x1)
                .map(|x| {
                    selected(area, domain, inverted, x, y).then(|| {
                        if input.bounds().contains(x, y) {
                            input.get(x, y)
                        } else {
                            Rgba::TRANSPARENT
                        }
                    })
                })
                .collect()
        })
        .collect();
    let extract = orient(&extract, t);
    let (ox, oy) = (0.min(area.x0), 0.min(area.y0));
    let width = i64::from(input.width()).max(area.x0 + extract[0].len() as i64) - ox;
    let height = i64::from(input.height()).max(area.y0 + extract.len() as i64) - oy;
    let mut result = vec![vec![Rgba::TRANSPARENT; width as usize]; height as usize];
    for y in 0..i64::from(input.height()) {
        for x in 0..i64::from(input.width()) {
            result[(y - oy) as usize][(x - ox) as usize] = if selected(area, domain, inverted, x, y)
            {
                clear
            } else {
                input.get(x, y)
            };
        }
    }
    for (y, row) in extract.iter().enumerate() {
        for (x, pixel) in row.iter().enumerate() {
            if let Some(pixel) = pixel {
                result[(area.y0 + y as i64 - oy) as usize][(area.x0 + x as i64 - ox) as usize] =
                    *pixel;
            }
        }
    }
    (result, (ox, oy))
}
#[test]
fn every_orientation_preserves_holes_overlap_hidden_rgb_and_finite_default_samples() {
    let input = source();
    let initial = pixels(&input);
    let zero = SharedBudget::new(0);
    let admission = Admission::with_residency(0, zero.clone());
    for transform in ALL {
        for area in [
            rect(2, 1, 4, 3),
            rect(-2, -1, 5, 4),
            rect(6, 4, 5, 3),
            rect(11, -3, 4, 2),
        ] {
            for clipped in [false, true] {
                for inverted in [false, true] {
                    for clear in [Rgba::TRANSPARENT, Rgba([255; 4])] {
                        let (mask, domain) =
                            field(area, clipped, inverted, &SharedBudget::new(1 << 20));
                        let expected = oracle(&input, area, domain, inverted, transform, clear);
                        let (plan, origin) = MappedPlan::prepare_masked_patch(
                            PaintSnapshot::capture(&input, &admission, &|| Ok(())).unwrap(),
                            area,
                            mask,
                            transform,
                            clear,
                            &admission,
                            100_000,
                            &|| Ok(()),
                        )
                        .unwrap();
                        assert_eq!(origin, expected.1);
                        let logical = plan.snapshot(&admission, &|| Ok(())).unwrap();
                        let reader = logical.reader(&admission, &|| Ok(())).unwrap();
                        for (y, row) in expected.0.iter().enumerate() {
                            for (x, pixel) in row.iter().enumerate() {
                                assert_eq!(
                                    reader.read_checked(x as i64, y as i64, &|| Ok(())).unwrap(),
                                    *pixel,
                                    "logical {transform:?} {area:?} {clipped} {inverted} {x},{y}"
                                );
                            }
                        }
                        let output = plan
                            .replay(&mut Admission::new(1_000_000), &|| Ok(()))
                            .unwrap();
                        assert_eq!(
                            pixels(&output),
                            expected.0,
                            "{transform:?} {area:?} {clipped} {inverted}"
                        );
                    }
                }
            }
        }
    }
    assert_eq!(pixels(&input), initial);
    assert_eq!(zero.usage().peak, 0);
}
#[test]
fn full_masks_are_bit_exact_rectangular_patches_and_singletons_are_identity_controls() {
    let input = source();
    let admission = Admission::new(0);
    for area in [rect(2, 1, 4, 3), rect(-2, 4, 5, 3)] {
        for transform in ALL {
            let snapshot = PaintSnapshot::capture(&input, &admission, &|| Ok(())).unwrap();
            let (plain, plain_origin) = MappedPlan::prepare_patch(
                snapshot.clone(),
                area,
                transform,
                Rgba([255; 4]),
                &admission,
                100_000,
                &|| Ok(()),
            )
            .unwrap();
            let (masked, origin) = MappedPlan::prepare_masked_patch(
                snapshot,
                area,
                BinaryMask::uniform(area, true).unwrap(),
                transform,
                Rgba([255; 4]),
                &admission,
                100_000,
                &|| Ok(()),
            )
            .unwrap();
            assert_eq!(origin, plain_origin);
            assert_eq!(
                pixels(
                    &plain
                        .replay(&mut Admission::new(1_000_000), &|| Ok(()))
                        .unwrap()
                ),
                pixels(
                    &masked
                        .replay(&mut Admission::new(1_000_000), &|| Ok(()))
                        .unwrap()
                )
            );
        }
    }
    for transform in ALL {
        let area = rect(3, 2, 1, 1);
        let (plan, _) = MappedPlan::prepare_masked_patch(
            PaintSnapshot::capture(&input, &admission, &|| Ok(())).unwrap(),
            area,
            BinaryMask::uniform(area, true).unwrap(),
            transform,
            Rgba::TRANSPARENT,
            &admission,
            100_000,
            &|| Ok(()),
        )
        .unwrap();
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
#[test]
fn metadata_read_and_decoded_denials_and_every_prepare_checkpoint_release_private_owners() {
    let input = source();
    let area = rect(2, 1, 4, 3);
    let selection = SharedBudget::new(1 << 20);
    let metadata = SharedBudget::new(1 << 20);
    let admission = Admission::with_residency(0, SharedBudget::new(0))
        .with_paint_metadata_budget(metadata.clone());
    let snapshot = PaintSnapshot::capture(&input, &admission, &|| Ok(())).unwrap();
    let (mask, _) = field(area, false, false, &selection);
    let retained = selection.usage().reserved;
    let before = metadata.usage().reserved;
    let calls = Cell::new(0);
    drop(
        MappedPlan::prepare_masked_patch(
            snapshot.clone(),
            area,
            mask.clone(),
            T::RotateRight,
            Rgba::TRANSPARENT,
            &admission,
            100_000,
            &|| {
                calls.set(calls.get() + 1);
                Ok(())
            },
        )
        .unwrap(),
    );
    for at in 1..=calls.get() {
        let count = Cell::new(0);
        let result = MappedPlan::prepare_masked_patch(
            snapshot.clone(),
            area,
            mask.clone(),
            T::RotateRight,
            Rgba::TRANSPARENT,
            &admission,
            100_000,
            &|| {
                count.set(count.get() + 1);
                if count.get() == at {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel masked patch",
                    ))
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(metadata.usage().reserved, before);
        assert_eq!(selection.usage().reserved, retained);
    }
    assert_eq!(
        MappedPlan::prepare_masked_patch(
            snapshot.clone(),
            area,
            mask.clone(),
            T::RotateRight,
            Rgba::TRANSPARENT,
            &admission,
            0,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    let deny = Admission::new(0).with_paint_metadata_budget(SharedBudget::new(0));
    assert_eq!(
        MappedPlan::prepare_masked_patch(
            snapshot.clone(),
            area,
            mask.clone(),
            T::RotateRight,
            Rgba::TRANSPARENT,
            &deny,
            100_000,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    let (plan, _) = MappedPlan::prepare_masked_patch(
        snapshot.clone(),
        area,
        mask.clone(),
        T::RotateRight,
        Rgba::TRANSPARENT,
        &admission,
        100_000,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        plan.replay(&mut Admission::new(0), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let weak = plan.snapshot(&admission, &|| Ok(())).unwrap();
    drop(plan);
    drop(mask);
    assert_eq!(
        selection.usage().reserved,
        retained,
        "logical mapped snapshot owns the original selection storage"
    );
    drop(weak);
    drop(snapshot);
    assert_eq!(selection.usage().reserved, 0);
    assert_eq!(metadata.usage().reserved, 0);
}
#[test]
fn uniform_huge_layers_keep_zero_reads_when_selection_only_moves_default_pixels() -> Result<()> {
    let input = RasterLayer::new(900_000, 800_000, Rgba([255; 4]));
    let admission = Admission::with_residency(0, SharedBudget::new(0));
    let area = rect(10, 20, 4, 3);
    let (mask, _) = field(area, false, false, &SharedBudget::new(1 << 20));
    let (plan, offset) = MappedPlan::prepare_masked_patch(
        PaintSnapshot::capture(&input, &admission, &|| Ok(()))?,
        area,
        mask,
        T::RotateRight,
        Rgba([255; 4]),
        &admission,
        0,
        &|| Ok(()),
    )?;
    assert_eq!(offset, (0, 0));
    assert_eq!(plan.source_reads(), 0);
    assert_eq!(
        plan.replay(&mut admission.clone(), &|| Ok(()))?
            .resident_tiles(),
        0
    );
    Ok(())
}

#[test]
fn sparse_tile_seams_keep_default_pixels_and_cancelled_replays_release_decoded_output() {
    let mut input = RasterLayer::new(530, 265, Rgba([21, 43, 87, 0]));
    for (x, y, pixel) in [
        (254, 254, Rgba([255, 1, 3, 128])),
        (257, 255, Rgba([10, 20, 30, 0])),
        (259, 257, Rgba([4, 5, 6, 255])),
        (520, 260, Rgba([200, 100, 50, 0])),
    ] {
        input
            .blend_pixel(
                x,
                y,
                pixel,
                BlendMode::Replace,
                &mut Admission::new(1_000_000),
            )
            .unwrap();
    }
    let before = pixels(&input);
    let area = rect(254, 254, 7, 5);
    let admission = Admission::with_residency(0, SharedBudget::new(0));
    for transform in [T::RotateRight, T::FlipHorizontal] {
        let (mask, domain) = field(area, false, true, &SharedBudget::new(1 << 20));
        let expected = oracle(&input, area, domain, true, transform, Rgba::TRANSPARENT);
        let (plan, origin) = MappedPlan::prepare_masked_patch(
            PaintSnapshot::capture(&input, &admission, &|| Ok(())).unwrap(),
            area,
            mask,
            transform,
            Rgba::TRANSPARENT,
            &admission,
            1_000_000,
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(origin, expected.1);
        assert_eq!(
            pixels(
                &plan
                    .replay(&mut Admission::new(1_000_000), &|| Ok(()))
                    .unwrap()
            ),
            expected.0
        );
    }
    assert_eq!(pixels(&input), before);

    // A small irregular patch exercises every callback during actual decoded replay.
    let input = source();
    let area = rect(2, 1, 4, 3);
    let (mask, _) = field(area, false, false, &SharedBudget::new(1 << 20));
    let (plan, _) = MappedPlan::prepare_masked_patch(
        PaintSnapshot::capture(&input, &admission, &|| Ok(())).unwrap(),
        area,
        mask,
        T::RotateRight,
        Rgba::TRANSPARENT,
        &admission,
        100_000,
        &|| Ok(()),
    )
    .unwrap();
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
        let observed = Cell::new(0);
        let err = plan
            .replay(
                &mut Admission::with_residency(65_536, resident.clone()),
                &|| {
                    observed.set(observed.get() + 1);
                    if observed.get() == at {
                        Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "cancel patch replay",
                        ))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::OperationCancelled);
        assert_eq!(resident.usage().reserved, 0);
    }
}
