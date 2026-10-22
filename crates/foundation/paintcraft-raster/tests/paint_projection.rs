use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba, resources::SharedBudget};
use paintcraft_raster::{Admission, BlendMode, PaintPlan, RasterLayer, flood};
use std::{
    cell::Cell,
    collections::{BTreeSet, VecDeque},
};

fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn cancel() -> Error {
    Error::new(
        ErrorCode::OperationCancelled,
        "cancel projected preparation",
    )
}
fn coverage(src: Rgba, dst: Rgba, samples: u32) -> Rgba {
    Rgba(std::array::from_fn(|i| {
        ((u32::from(src.0[i]) * samples + u32::from(dst.0[i]) * (16 - samples) + 8) / 16) as u8
    }))
}

#[test]
fn logical_reads_keep_order_coverage_hidden_rgb_and_recover_admitted_replay() {
    let budget = SharedBudget::new(4 * RasterLayer::TILE_BYTES as u64);
    let mut base = RasterLayer::new(520, 4, Rgba([37, 19, 83, 0]));
    base.blend_pixel(
        255,
        1,
        Rgba([1, 255, 7, 255]),
        BlendMode::Replace,
        &mut Admission::with_residency(65_536, budget.clone()),
    )
    .unwrap();
    let versions: Vec<_> = base.tile_versions(base.bounds()).unwrap().collect();
    let before = budget.usage();
    let mut lower = PaintPlan::default();
    lower
        .rect(
            rect(-2, -1, 272, 4),
            Rgba([220, 40, 90, 128]),
            BlendMode::SourceOver,
        )
        .unwrap();
    let lower = lower
        .into_projection_checked(base.bounds(), &|| Ok(()))
        .unwrap();
    let mut upper = PaintPlan::default();
    upper
        .coverage_rect(rect(254, 0, 6, 4), Rgba([251, 139, 37, 0]), 7)
        .unwrap();
    upper
        .rect(rect(258, 2, 5, 1), Rgba([1, 2, 3, 0]), BlendMode::Replace)
        .unwrap();
    let upper = upper
        .into_projection_checked(base.bounds(), &|| Ok(()))
        .unwrap();
    assert_eq!(lower.tile_coords().count(), 2);
    assert!(upper.metadata_bytes() > 0);
    let mut expected = Vec::new();
    for y in 0..4 {
        for x in 0..520 {
            let mut pixel = base.get(x, y);
            if x < 270 && y < 3 {
                pixel = Rgba([220, 40, 90, 128]).source_over(pixel);
            }
            if (254..260).contains(&x) {
                pixel = coverage(Rgba([251, 139, 37, 0]), pixel, 7);
            }
            if (258..263).contains(&x) && y == 2 {
                pixel = Rgba([1, 2, 3, 0]);
            }
            let actual = upper
                .read_checked(
                    x,
                    y,
                    |x, y| lower.read_checked(x, y, |x, y| Ok(base.get(x, y)), &|| Ok(())),
                    &|| Ok(()),
                )
                .unwrap();
            assert_eq!(actual, pixel, "{x},{y}");
            expected.push(pixel);
        }
    }
    assert_eq!(
        budget.usage(),
        before,
        "projection must not acquire decoded leases"
    );
    assert_eq!(
        versions,
        base.tile_versions(base.bounds())
            .unwrap()
            .collect::<Vec<_>>()
    );
    let mut replay = base.clone();
    let mut admission = Admission::with_residency(4 * 65_536, budget.clone());
    lower
        .into_plan()
        .apply(&mut replay, &mut admission)
        .unwrap();
    upper
        .into_plan()
        .apply(&mut replay, &mut admission)
        .unwrap();
    for y in 0..4 {
        for x in 0..520 {
            assert_eq!(replay.get(x, y), expected[(y * 520 + x) as usize]);
        }
    }
    assert_eq!(base.get(255, 1), Rgba([1, 255, 7, 255]));
}

#[test]
fn projection_index_is_bounded_and_source_independent() {
    let mut plan = PaintPlan::with_metadata_limit(512);
    plan.rect(
        rect(0, 0, 1_048_576, 1),
        Rgba::TRANSPARENT,
        BlendMode::Replace,
    )
    .unwrap();
    let calls = Cell::new(0);
    let error = plan
        .into_projection_checked(rect(0, 0, 1_048_576, 1), &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(error.allocation_denial().is_none());
    assert!(
        calls.get() < 32,
        "must stop index growth before the whole sparse row"
    );
    // Transparent replacement is retained: it can clear a preceding logical
    // source even when no decoded raster exists at projection construction.
    let mut plan = PaintPlan::default();
    plan.rect(rect(0, 0, 1, 1), Rgba([9, 8, 7, 0]), BlendMode::Replace)
        .unwrap();
    let projection = plan
        .into_projection_checked(rect(0, 0, 2, 1), &|| Ok(()))
        .unwrap();
    assert_eq!(
        projection
            .read_checked(0, 0, |_, _| Ok(Rgba([255; 4])), &|| Ok(()))
            .unwrap(),
        Rgba([9, 8, 7, 0])
    );
    assert_eq!(
        projection
            .read_checked(1, 0, |_, _| Ok(Rgba([255; 4])), &|| Ok(()))
            .unwrap(),
        Rgba([255; 4])
    );
    assert_eq!(
        projection
            .read_checked(
                -1,
                0,
                |_, _| panic!("outside must not read source"),
                &|| Ok(())
            )
            .unwrap(),
        Rgba::TRANSPARENT
    );
    for bounds in [
        rect(-1, 0, 2, 1),
        rect(1, 0, 2, 1),
        rect(0, 0, i64::from(u32::MAX) + 1, 1),
    ] {
        assert_eq!(
            PaintPlan::default()
                .into_projection_checked(bounds, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
}

#[test]
fn construction_cancels_at_every_observed_index_checkpoint() {
    let build = || {
        let mut plan = PaintPlan::default();
        for y in 0..4 {
            plan.rect(
                rect(0, y, 520, 1),
                Rgba([17, 88, 156, 128]),
                BlendMode::SourceOver,
            )
            .unwrap();
        }
        plan
    };
    let total = Cell::new(0);
    build()
        .into_projection_checked(rect(0, 0, 520, 4), &|| {
            total.set(total.get() + 1);
            Ok(())
        })
        .unwrap();
    for at in 1..=total.get() {
        let calls = Cell::new(0);
        let error = build()
            .into_projection_checked(rect(0, 0, 520, 4), &|| {
                calls.set(calls.get() + 1);
                if calls.get() == at {
                    Err(cancel())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(calls.get(), at);
    }
}

#[test]
fn long_noncovering_descriptor_lists_cancel_and_source_errors_propagate() {
    let mut plan = PaintPlan::default();
    for _ in 0..4096 {
        plan.rect(rect(1, 1, 1, 1), Rgba([1; 4]), BlendMode::SourceOver)
            .unwrap();
    }
    // Keep the read inside the conservative envelope but outside every actual
    // write, so the long-list cancellation contract still exercises the scan.
    plan.rect(rect(3, 3, 1, 1), Rgba([2; 4]), BlendMode::Replace)
        .unwrap();
    let projection = plan
        .into_projection_checked(rect(0, 0, 8, 8), &|| Ok(()))
        .unwrap();
    let calls = Cell::new(0);
    projection
        .read_checked(2, 2, |_, _| Ok(Rgba([7, 8, 9, 0])), &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    assert!(calls.get() >= 6);
    for at in 1..=calls.get() {
        let seen = Cell::new(0);
        let error = projection
            .read_checked(2, 2, |_, _| Ok(Rgba([7, 8, 9, 0])), &|| {
                seen.set(seen.get() + 1);
                if seen.get() == at {
                    Err(cancel())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(seen.get(), at);
    }
    let error = projection
        .read_checked(
            2,
            2,
            |_, _| Err(Error::new(ErrorCode::ConflictStaleBase, "source retired")),
            &|| Ok(()),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ConflictStaleBase);
}

fn bfs(pixels: &[Rgba; 9], seed: usize, tolerance: u8) -> BTreeSet<(i64, i64)> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([seed]);
    while let Some(i) = queue.pop_front() {
        let (x, y) = ((i % 3) as i64, (i / 3) as i64);
        if seen.contains(&(x, y))
            || pixels[i]
                .0
                .iter()
                .zip(pixels[seed].0)
                .any(|(&a, b)| a.abs_diff(b) > tolerance)
        {
            continue;
        }
        seen.insert((x, y));
        for (x, y) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if (0..3).contains(&x) && (0..3).contains(&y) {
                queue.push_back((y * 3 + x) as usize);
            }
        }
    }
    seen
}

#[test]
fn projected_flood_observes_prior_writes_and_matches_independent_bfs() {
    let bounds = rect(0, 0, 3, 3);
    let wall = Rgba([1, 2, 3, 255]);
    let ground = Rgba([20, 40, 80, 0]);
    let mut lower = PaintPlan::default();
    lower
        .rect(rect(1, 0, 1, 2), wall, BlendMode::Replace)
        .unwrap();
    let lower = lower.into_projection_checked(bounds, &|| Ok(())).unwrap();
    let mut upper = PaintPlan::default();
    upper.coverage_rect(rect(1, 1, 2, 1), ground, 7).unwrap();
    let upper = upper.into_projection_checked(bounds, &|| Ok(())).unwrap();
    for bits in 0..512 {
        let base: [Rgba; 9] = std::array::from_fn(|i| {
            if bits >> i & 1 == 0 {
                Rgba([5, 10, 15, 0])
            } else {
                Rgba([200, 80, 20, 255])
            }
        });
        let mut expected = base;
        expected[1] = wall;
        expected[4] = wall;
        for i in [4, 5] {
            expected[i] = coverage(ground, expected[i], 7);
        }
        for seed in 0..9 {
            for tolerance in [0, 16, 128, 255] {
                let spans = flood::region_from_source_checked(
                    |x, y| {
                        upper.read_checked(
                            x,
                            y,
                            |x, y| {
                                lower.read_checked(
                                    x,
                                    y,
                                    |x, y| Ok(base[(y * 3 + x) as usize]),
                                    &|| Ok(()),
                                )
                            },
                            &|| Ok(()),
                        )
                    },
                    ((seed % 3) as i64, (seed / 3) as i64),
                    &bounds,
                    tolerance,
                    64 << 10,
                    &|| Ok(()),
                )
                .unwrap();
                let actual: BTreeSet<_> = spans
                    .iter()
                    .flat_map(|s| (s.x0..=s.x1).map(move |x| (x, s.y)))
                    .collect();
                assert_eq!(
                    actual,
                    bfs(&expected, seed, tolerance),
                    "bits={bits} seed={seed} tolerance={tolerance}"
                );
            }
        }
    }
}

#[test]
fn fallible_flood_reads_discard_regions_and_preserve_scratch_checks() {
    let clip = rect(0, 0, 256, 1);
    let reads = Cell::new(0);
    let error = flood::region_from_source_checked(
        |_, _| {
            reads.set(reads.get() + 1);
            if reads.get() == 100 {
                Err(cancel())
            } else {
                Ok(Rgba([1; 4]))
            }
        },
        (0, 0),
        &clip,
        0,
        64 << 10,
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(reads.get(), 100);
    assert!(
        flood::region_from_source_checked(
            |_, _| panic!("outside seed"),
            (-1, 0),
            &clip,
            0,
            0,
            &|| Ok(())
        )
        .unwrap()
        .is_empty()
    );
    let error = flood::region_from_source_checked(
        |_, _| panic!("scratch admission first"),
        (0, 0),
        &clip,
        0,
        0,
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(error.allocation_denial().is_none());
}
