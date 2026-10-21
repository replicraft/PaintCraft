use super::*;

fn samples(points: &[(i64, i64)], size: u32, clip: &Rect) -> BTreeMap<(i64, i64), u8> {
    let mut pixels = BTreeMap::new();
    for run in round_path(points, size, clip).unwrap() {
        for x in run.x0..=run.x1 {
            assert!(pixels.insert((x, run.y), run.samples).is_none());
        }
    }
    pixels
}

// Independent slow point-to-segment distance oracle, rather than scanline math.
#[test]
fn coverage_matches_sample_distance_in_every_direction() {
    let clip = Rect::new(-7, -7, 15, 15).unwrap();
    for size in 1..=7 {
        for b in [(-4, -3), (-4, 3), (0, 0), (0, 4), (4, 0), (4, 3), (3, 4)] {
            let path = [(0, 0), b];
            let got = samples(&path, size, &clip);
            for y in clip.y0..clip.y1 {
                for x in clip.x0..clip.x1 {
                    let mut expected = 0;
                    for sy in 0..4 {
                        for sx in 0..4 {
                            let p = (
                                x as f64 + (f64::from(sx) + 0.5) / 4.0 - 0.5,
                                y as f64 + (f64::from(sy) + 0.5) / 4.0 - 0.5,
                            );
                            let d = (b.0 as f64, b.1 as f64);
                            let len = d.0 * d.0 + d.1 * d.1;
                            let t = if len == 0.0 {
                                0.0
                            } else {
                                ((p.0 * d.0 + p.1 * d.1) / len).clamp(0.0, 1.0)
                            };
                            if (p.0 - t * d.0).powi(2) + (p.1 - t * d.1).powi(2)
                                <= (f64::from(size) / 2.0).powi(2)
                            {
                                expected += 1;
                            }
                        }
                    }
                    assert_eq!(
                        got.get(&(x, y)).copied().unwrap_or(0),
                        expected,
                        "size {size}, end {b:?}, pixel {x},{y}"
                    );
                }
            }
        }
    }
}

#[test]
fn retracing_and_segmentation_do_not_accumulate_opacity() {
    let clip = Rect::new(-10, -10, 30, 30).unwrap();
    let expected = samples(&[(0, 0), (8, 4)], 5, &clip);
    assert_eq!(expected, samples(&[(0, 0), (4, 2), (8, 4)], 5, &clip));
    assert_eq!(
        expected,
        samples(&[(8, 4), (0, 0), (8, 4), (8, 4)], 5, &clip)
    );
    assert!(expected.values().any(|&n| n > 0 && n < 16));
}

#[test]
fn clipping_and_large_translation_preserve_coverage() {
    let clip = Rect::new(-2, -2, 5, 5).unwrap();
    let expected = samples(&[(-3, -4), (7, 8)], 4, &clip);
    let d = 100_000_000_000_000;
    let translated = samples(
        &[(d - 3, d - 4), (d + 7, d + 8)],
        4,
        &clip.translated(d, d).unwrap(),
    );
    assert_eq!(
        expected,
        translated
            .into_iter()
            .map(|((x, y), n)| ((x - d, y - d), n))
            .collect()
    );
    assert!(
        expected
            .keys()
            .all(|&(x, y)| x >= clip.x0 && x < clip.x1 && y >= clip.y0 && y < clip.y1)
    );
}

#[test]
fn excessive_geometry_is_rejected() {
    let clip = Rect::new(0, 0, 1_000_000, 1_000_000).unwrap();
    assert_eq!(
        round_path(&[(0, 0), (0, 999_999)], 1, &clip)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert!(round_path(&[(i64::MIN, 0)], 4, &clip).is_err());
}

#[test]
fn subpixel_capsules_match_an_independent_distance_oracle() {
    let clip = Rect::new(-3, -3, 15, 15).unwrap();
    let points = [(320, 192), (1680, 1488), (320, 192)];
    let mut actual = BTreeMap::new();
    for run in round_path_scaled(&points, 3, &clip, 256).unwrap() {
        for x in run.x0..=run.x1 {
            assert!(actual.insert((x, run.y), run.samples).is_none());
        }
    }
    for y in clip.y0..clip.y1 {
        for x in clip.x0..clip.x1 {
            let mut expected = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let p = (
                        x as f64 + (sx as f64 + 0.5) / 4.0,
                        y as f64 + (sy as f64 + 0.5) / 4.0,
                    );
                    let a = (1.25, 0.75);
                    let b = (6.5625, 5.8125);
                    let d = (b.0 - a.0, b.1 - a.1);
                    let t = (((p.0 - a.0) * d.0 + (p.1 - a.1) * d.1) / (d.0 * d.0 + d.1 * d.1))
                        .clamp(0.0, 1.0);
                    if (p.0 - a.0 - t * d.0).powi(2) + (p.1 - a.1 - t * d.1).powi(2) <= 2.25 {
                        expected += 1;
                    }
                }
            }
            assert_eq!(
                actual.get(&(x, y)).copied().unwrap_or(0),
                expected,
                "at {x},{y}"
            );
        }
    }
    // Integer centers still produce exactly the historical smooth-round pixels.
    let integer = [(1, 2), (6, 7)];
    let fixed: Vec<_> = integer
        .iter()
        .map(|&(x, y)| (x * 256 + 128, y * 256 + 128))
        .collect();
    assert_eq!(
        round_path(&integer, 4, &clip).unwrap(),
        round_path_scaled(&fixed, 4, &clip, 256).unwrap()
    );
    let delta = 100_000_000_000_000;
    let translated: Vec<_> = fixed
        .iter()
        .map(|&(x, y)| (x + delta * 256, y + delta * 256))
        .collect();
    let mut runs =
        round_path_scaled(&translated, 4, &clip.translated(delta, delta).unwrap(), 256).unwrap();
    for run in &mut runs {
        run.x0 -= delta;
        run.x1 -= delta;
        run.y -= delta;
    }
    assert_eq!(runs, round_path(&integer, 4, &clip).unwrap());
}

#[test]
fn cancellation_interrupts_scan_rows_and_union_without_changing_successful_coverage() {
    use std::cell::Cell;
    let clip = Rect::new(0, 0, 512, 512).unwrap();
    let points = [(8, 8), (500, 500), (8, 500)];
    let expected = round_path_scaled(&points, 32, &clip, 1).unwrap();
    let calls = Cell::new(0);
    let actual = round_path_scaled_checked(&points, 32, &clip, 1, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(actual, expected);
    assert!(calls.get() > 40);
    // Exercise every check boundary, including segment scans and union resolution.
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let error = round_path_scaled_checked(&points, 32, &clip, 1, &|| {
            seen.set(seen.get() + 1);
            if seen.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel geometry"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(seen.get(), stop);
    }
}

#[test]
fn cached_extensions_match_full_sample_unions_for_retraces_clipping_and_policy_changes() {
    let mut cache = PathCache::default();
    let path = [(0, 0), (4, 3), (-4, 3), (4, -3), (4, -3), (0, 0), (2, 1)];
    for scale in [1, 256] {
        for size in 1..=7 {
            for origin in [0, 100_000_000_000_000] {
                let clip = Rect::new(origin - 3, origin - 3, 7, 7).unwrap();
                let fixed: Vec<_> = path
                    .iter()
                    .map(|&(x, y)| {
                        (
                            (x + origin) * i64::from(scale),
                            (y + origin) * i64::from(scale),
                        )
                    })
                    .collect();
                for n in 1..=fixed.len() {
                    let expected = round_path_scaled(&fixed[..n], size, &clip, scale).unwrap();
                    let got = cache
                        .round_path_scaled(&fixed[..n], size, &clip, scale, &|| Ok(()))
                        .unwrap();
                    assert_eq!(got, expected, "size {size}, scale {scale}, prefix {n}");
                    assert_eq!(
                        cache
                            .round_path_scaled(&fixed[..n], size, &clip, scale, &|| Ok(()))
                            .unwrap(),
                        expected
                    );
                }
                // Truncation and a changed interior point must rebuild even with the same tail.
                for changed in [
                    vec![fixed[0]],
                    vec![fixed[0], fixed[3], *fixed.last().unwrap()],
                ] {
                    assert_eq!(
                        cache
                            .round_path_scaled(&changed, size, &clip, scale, &|| Ok(()))
                            .unwrap(),
                        round_path_scaled(&changed, size, &clip, scale).unwrap()
                    );
                }
            }
        }
    }
}

#[test]
fn cancelled_cache_extensions_retire_partial_unions_and_rebuild_exactly() {
    use std::cell::Cell;
    let clip = Rect::new(-8, -8, 32, 32).unwrap();
    let points = [(0, 0), (12, 12), (0, 12)];
    let mut cache = PathCache::default();
    let calls = Cell::new(0);
    cache
        .round_path_scaled(&points[..1], 7, &clip, 1, &|| Ok(()))
        .unwrap();
    let expected = cache
        .round_path_scaled(&points, 7, &clip, 1, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    for stop in 1..=calls.get() {
        cache.clear();
        cache
            .round_path_scaled(&points[..1], 7, &clip, 1, &|| Ok(()))
            .unwrap();
        let seen = Cell::new(0);
        let error = cache
            .round_path_scaled(&points, 7, &clip, 1, &|| {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel extension",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(cache.metadata_charge(), 0);
        assert_eq!(
            cache
                .round_path_scaled(&points, 7, &clip, 1, &|| Ok(()))
                .unwrap(),
            expected
        );
    }
    let mut disabled = PathCache::new(0);
    assert_eq!(
        disabled
            .round_path_scaled(&points, 7, &clip, 1, &|| Ok(()))
            .unwrap(),
        expected
    );
    assert_eq!(disabled.metadata_charge(), 0);
    assert!(
        cache
            .round_path_scaled(&[(i64::MIN, 0)], 7, &clip, 1, &|| Ok(()))
            .is_err()
    );
    assert_eq!(cache.metadata_charge(), 0);
}

#[test]
fn incremental_and_full_paths_enforce_the_same_scan_budget_after_a_single_point() {
    let clip = Rect::new(0, 0, 1, 1024).unwrap();
    let points = vec![(0, 512); 258];
    let mut cache = PathCache::default();
    cache
        .round_path_scaled(&points[..1], 1024, &clip, 1, &|| Ok(()))
        .unwrap();
    for n in [2, 256, 257, 258] {
        let expected = round_path_scaled(&points[..n], 1024, &clip, 1);
        let actual = cache.round_path_scaled(&points[..n], 1024, &clip, 1, &|| Ok(()));
        match (actual, expected) {
            (Ok(a), Ok(b)) => assert_eq!(a, b),
            (Err(a), Err(b)) => assert_eq!(a.code, b.code),
            _ => panic!("cache changed work-budget result for {n} points"),
        }
    }
}

#[test]
fn extending_a_cached_path_does_not_rescan_the_old_segments() {
    use std::cell::Cell;
    let clip = Rect::new(0, 0, 512, 128).unwrap();
    let points: Vec<_> = (0..256).map(|x| (x, 32 + x % 16)).collect();
    let mut cache = PathCache::default();
    cache
        .round_path_scaled(&points[..255], 16, &clip, 1, &|| Ok(()))
        .unwrap();
    let incremental_checks = Cell::new(0);
    let full_checks = Cell::new(0);
    let actual = cache
        .round_path_scaled(&points, 16, &clip, 1, &|| {
            incremental_checks.set(incremental_checks.get() + 1);
            Ok(())
        })
        .unwrap();
    let expected = round_path_scaled_checked(&points, 16, &clip, 1, &|| {
        full_checks.set(full_checks.get() + 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(actual, expected);
    assert!(full_checks.get() > incremental_checks.get() + 250);
}
