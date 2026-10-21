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
