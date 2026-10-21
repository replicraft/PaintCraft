use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba};
use paintcraft_raster::{Admission, RasterLayer, flood, shape::Span};
use std::{
    cell::Cell,
    collections::{BTreeSet, VecDeque},
};
fn cells(spans: &[Span]) -> BTreeSet<(i64, i64)> {
    let mut cells = BTreeSet::new();
    for span in spans {
        for x in span.x0..=span.x1 {
            assert!(cells.insert((x, span.y)), "overlapping spans");
        }
    }
    cells
}
fn bfs(
    pixels: &[Rgba],
    width: usize,
    seed: (i64, i64),
    clip: Rect,
    tolerance: u8,
) -> BTreeSet<(i64, i64)> {
    let mut out = BTreeSet::new();
    if !clip.contains(seed.0, seed.1) {
        return out;
    }
    let target = pixels[seed.1 as usize * width + seed.0 as usize];
    let mut pending = VecDeque::from([seed]);
    while let Some((x, y)) = pending.pop_front() {
        if !clip.contains(x, y) || out.contains(&(x, y)) {
            continue;
        }
        let pixel = pixels[y as usize * width + x as usize];
        if !(0..4).all(|c| pixel.0[c].abs_diff(target.0[c]) <= tolerance) {
            continue;
        }
        out.insert((x, y));
        pending.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
    }
    out
}

#[test]
fn scanline_matches_independent_bfs_for_all_small_binary_images_seeds_and_clips() {
    for pattern in 0..512u32 {
        let pixels: Vec<_> = (0..9)
            .map(|n| {
                if pattern & (1 << n) == 0 {
                    Rgba([17, 31, 63, 0])
                } else {
                    Rgba([24, 38, 70, 7])
                }
            })
            .collect();
        let bytes: Vec<_> = pixels.iter().flat_map(|p| p.0).collect();
        let layer =
            RasterLayer::from_rgba(3, 3, Rgba::TRANSPARENT, &bytes, &mut Admission::new(65_536))
                .unwrap();
        for clip in [layer.bounds(), Rect::new(1, 1, 2, 2).unwrap()] {
            for y in 0..3 {
                for x in 0..3 {
                    for tol in [0, 6, 7, 255] {
                        let got =
                            flood::region_checked(&layer, (x, y), &clip, tol, &|| Ok(())).unwrap();
                        assert_eq!(
                            cells(&got),
                            bfs(&pixels, 3, (x, y), clip, tol),
                            "pattern={pattern}, seed={x},{y}, tolerance={tol}"
                        );
                    }
                }
            }
        }
        for y in 0..3 {
            for x in 0..3 {
                assert_eq!(layer.get(x, y), pixels[(y * 3 + x) as usize]);
            }
        }
    }
}

#[test]
fn cancellation_at_every_observed_checkpoint_never_returns_a_partial_wide_row() {
    let layer = RasterLayer::new(8192, 1, Rgba([17, 31, 63, 0]));
    let calls = Cell::new(0);
    let result = flood::region_checked(&layer, (0, 0), &layer.bounds(), 0, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(cells(&result).len(), 8192);
    assert!(
        calls.get() > 20,
        "one row must yield during scanning/marking, not only between spans"
    );
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let error = flood::region_checked(&layer, (0, 0), &layer.bounds(), 0, &|| {
            seen.set(seen.get() + 1);
            if seen.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel flood"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(seen.get(), stop);
        assert_eq!(layer.resident_tiles(), 0);
        assert_eq!(layer.get(0, 0), Rgba([17, 31, 63, 0]));
    }
}

#[test]
fn fixed_scratch_limit_bounds_bitmap_and_fragmented_frontier_output_without_mutation() {
    let colors: Vec<_> = (0..64 * 64)
        .map(|i| {
            if (i / 64) % 2 == 0 || i % 2 == 0 {
                Rgba([17, 31, 63, 0])
            } else {
                Rgba([24, 38, 70, 7])
            }
        })
        .collect();
    let bytes: Vec<_> = colors.iter().flat_map(|p| p.0).collect();
    let layer = RasterLayer::from_rgba(
        64,
        64,
        Rgba::TRANSPARENT,
        &bytes,
        &mut Admission::new(65_536),
    )
    .unwrap();
    let original = layer.clone();
    for limit in [0, 511, 2048] {
        let error = flood::region_with_scratch_limit_checked(
            &layer,
            (0, 0),
            &layer.bounds(),
            0,
            limit,
            &|| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(error.message, "flood-fill scratch budget exceeded");
        assert!(error.allocation_denial().is_none());
        assert_eq!(layer, original);
    }
    let result = flood::region_with_scratch_limit_checked(
        &layer,
        (0, 0),
        &layer.bounds(),
        0,
        128 << 10,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(cells(&result), bfs(&colors, 64, (0, 0), layer.bounds(), 0));
    assert_eq!(layer, original);
}

#[test]
fn bitmap_initialization_is_cancellable_and_signed_extremes_do_not_overflow() {
    let layer = RasterLayer::new(8192, 8192, Rgba::TRANSPARENT);
    let calls = Cell::new(0);
    let error = flood::region_checked(&layer, (0, 0), &layer.bounds(), 0, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == 4 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel bitmap initialization",
            ))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(calls.get(), 4);
    assert_eq!(layer.resident_tiles(), 0);
    let wide = Rect {
        x0: i64::MIN,
        y0: i64::MIN,
        x1: i64::MAX,
        y1: i64::MAX,
    };
    assert_eq!(
        flood::region(&layer, (0, 0), &wide, 0).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let corner = Rect::new(i64::MIN, i64::MIN, 2, 2).unwrap();
    let result = flood::region(&layer, (i64::MIN, i64::MIN), &corner, 0).unwrap();
    assert_eq!(cells(&result).len(), 4);
    let outside = flood::region_with_scratch_limit_checked(
        &layer,
        (8, 8),
        &Rect::new(0, 0, 1, 1).unwrap(),
        0,
        0,
        &|| Ok(()),
    )
    .unwrap();
    assert!(outside.is_empty());
}
