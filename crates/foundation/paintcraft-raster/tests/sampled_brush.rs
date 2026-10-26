use paintcraft_core::types::{
    Error, ErrorCode, Rect, policy::COORD_ABS_MAX, resources::SharedBudget,
};
use paintcraft_raster::sampled_brush::{self, SampledTip};
use std::cell::Cell;

fn tip(alpha: &[u8], width: u32, height: u32, anchor: (u32, u32)) -> SampledTip {
    SampledTip::from_alpha(
        width,
        height,
        anchor,
        alpha,
        &SharedBudget::new(1 << 24),
        &|| Ok(()),
    )
    .unwrap()
}
fn dense(result: &sampled_brush::Coverage, area: Rect) -> Vec<u8> {
    let mut output = vec![0; (area.width() * area.height()) as usize];
    for span in result.spans() {
        for x in span.x0..=span.x1 {
            assert!(area.contains(x, span.y));
            let dest = ((span.y - area.y0) * area.width() + x - area.x0) as usize;
            assert_eq!(output[dest], 0, "coverage runs must be disjoint");
            output[dest] = span.coverage;
        }
    }
    output
}

#[test]
fn asymmetric_native_masks_match_an_independent_dense_stamp_oracle() {
    let alpha = [0, 64, 255, 128, 192, 1];
    let brush = tip(&alpha, 3, 2, (1, 1));
    let area = Rect::new(0, 0, 10, 10).unwrap();
    let result = sampled_brush::coverage(
        &brush,
        &[(3, 3), (6, 3), (6, 6)],
        1,
        256,
        area,
        &SharedBudget::new(1 << 20),
        &|| Ok(()),
    )
    .unwrap();
    // Deliberately independent: known integer centers, direct source lookups,
    // and dense per-pixel max; no production path sampling or span helper.
    let centers = [(3, 3), (4, 3), (5, 3), (6, 3), (6, 4), (6, 5), (6, 6)];
    let mut expected = vec![0; 100];
    for y in 0..10i64 {
        for x in 0..10i64 {
            for &(cx, cy) in &centers {
                let (u, v) = (x - cx + 1, y - cy + 1);
                if (0..3).contains(&u) && (0..2).contains(&v) {
                    let value = alpha[(v * 3 + u) as usize];
                    let dest = &mut expected[(y * 10 + x) as usize];
                    *dest = (*dest).max(value);
                }
            }
        }
    }
    assert_eq!(dense(&result, area), expected);
    assert_eq!(brush.alpha(), alpha);
}

#[test]
fn q8_translations_have_exact_bilinear_coverage_and_no_forced_endpoint_dab() {
    let brush = tip(&[255], 1, 1, (0, 0));
    let area = Rect::new(0, 0, 4, 4).unwrap();
    let budget = SharedBudget::new(1 << 20);
    let half = sampled_brush::coverage(&brush, &[(384, 384)], 256, 256, area, &budget, &|| Ok(()))
        .unwrap();
    assert_eq!(
        dense(&half, area),
        [0, 0, 0, 0, 0, 64, 64, 0, 0, 64, 64, 0, 0, 0, 0, 0]
    );
    let spaced =
        sampled_brush::coverage(&brush, &[(0, 0), (3, 0)], 1, 512, area, &budget, &|| Ok(()))
            .unwrap();
    assert_eq!(&dense(&spaced, area)[..4], [255, 0, 255, 0]);
    let segmented = sampled_brush::coverage(
        &brush,
        &[(0, 0), (1, 0), (3, 0)],
        1,
        512,
        area,
        &budget,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(spaced.spans(), segmented.spans());
}

#[test]
fn clipping_and_large_translation_preserve_exact_integer_and_subpixel_values() {
    let brush = tip(&[255, 64, 128, 0], 2, 2, (1, 1));
    let small = Rect::new(-2, -2, 6, 6).unwrap();
    let shift = COORD_ABS_MAX - 16;
    let big = Rect::new(shift - 2, shift - 2, 6, 6).unwrap();
    let budget = SharedBudget::new(1 << 20);
    let a = sampled_brush::coverage(
        &brush,
        &[(128, 128), (384, 384)],
        256,
        256,
        small,
        &budget,
        &|| Ok(()),
    )
    .unwrap();
    let b = sampled_brush::coverage(
        &brush,
        &[
            (shift * 256 + 128, shift * 256 + 128),
            (shift * 256 + 384, shift * 256 + 384),
        ],
        256,
        256,
        big,
        &budget,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(dense(&a, small), dense(&b, big));
    let clipped = Rect::new(0, 0, 1, 1).unwrap();
    let result =
        sampled_brush::coverage(&brush, &[(0, 0)], 1, 256, clipped, &budget, &|| Ok(())).unwrap();
    assert!(result.is_empty()); // this sample's anchored pixel is zero
}

#[test]
fn immutable_clones_hold_one_resource_declaration_until_the_last_owner_drops() {
    let budget = SharedBudget::new(1 << 20);
    let brush = SampledTip::from_alpha(1, 1, (0, 0), &[255], &budget, &|| Ok(())).unwrap();
    let held = brush.declared_bytes();
    let clone = brush.clone();
    assert!(brush.shares_storage(&clone));
    assert_eq!(budget.usage().reserved, held);
    drop(brush);
    assert_eq!(budget.usage().reserved, held);
    let output = sampled_brush::coverage(
        &clone,
        &[(0, 0)],
        1,
        256,
        Rect::new(0, 0, 4, 4).unwrap(),
        &budget,
        &|| Ok(()),
    )
    .unwrap();
    let retained = output.declared_bytes();
    assert_eq!(budget.usage().reserved, held + retained);
    let other = output.clone();
    drop(output);
    assert_eq!(budget.usage().reserved, held + retained);
    drop(other);
    assert_eq!(budget.usage().reserved, held);
    drop(clone);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn resource_geometry_path_and_span_limits_do_not_return_partial_coverage() {
    let budget = SharedBudget::new(0);
    assert_eq!(
        SampledTip::from_alpha(1, 1, (0, 0), &[255], &budget, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, 0);
    for (w, h, anchor, alpha) in [
        (0, 1, (0, 0), &[0][..]),
        (2, 1, (0, 0), &[0][..]),
        (1, 1, (1, 0), &[0][..]),
    ] {
        assert!(
            SampledTip::from_alpha(w, h, anchor, alpha, &SharedBudget::new(1 << 20), &|| Ok(()))
                .is_err()
        );
    }
    let brush = tip(&[255], 1, 1, (0, 0));
    let area = Rect::new(0, 0, 8192, 8192).unwrap();
    assert_eq!(
        sampled_brush::coverage(&brush, &[(0, 0)], 1, 256, area, &budget, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let ample = SharedBudget::new(1 << 25);
    for (points, scale, spacing) in [
        (&[][..], 1, 256),
        (&[(0, 0)][..], 3, 256),
        (&[(0, 0)][..], 1, 0),
        (&[(0, 0), (65536, 0)][..], 1, 256),
        (&[(0, 0), (4096, 4096)][..], 1, 256),
    ] {
        assert!(
            sampled_brush::coverage(&brush, points, scale, spacing, area, &ample, &|| Ok(()))
                .is_err()
        );
        assert_eq!(ample.usage().reserved, 0);
    }
    let alpha: Vec<_> = (0..1024 * 1024)
        .map(|i| if i % 2 == 0 { 1 } else { 2 })
        .collect();
    let noisy = tip(&alpha, 1024, 1024, (0, 0));
    assert_eq!(
        sampled_brush::coverage(&noisy, &[(0, 0)], 1, 256, area, &ample, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(ample.usage().reserved, 0);
}

#[test]
fn every_coverage_checkpoint_cancels_without_retaining_scratch_or_changing_the_tip() {
    let brush = tip(&[255, 64, 128, 1], 2, 2, (0, 0));
    let budget = SharedBudget::new(1 << 20);
    let area = Rect::new(0, 0, 16, 16).unwrap();
    let total = Cell::new(0);
    drop(
        sampled_brush::coverage(&brush, &[(1, 1), (5, 5)], 1, 256, area, &budget, &|| {
            total.set(total.get() + 1);
            Ok(())
        })
        .unwrap(),
    );
    for stop in 0..total.get() {
        let calls = Cell::new(0);
        let result =
            sampled_brush::coverage(&brush, &[(1, 1), (5, 5)], 1, 256, area, &budget, &|| {
                let n = calls.get();
                calls.set(n + 1);
                if n == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "test"))
                } else {
                    Ok(())
                }
            });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(brush.alpha(), [255, 64, 128, 1]);
    }
    let zero = tip(&[0; 4], 2, 2, (0, 0));
    assert!(
        sampled_brush::coverage(
            &zero,
            &[(0, 0), (5, 5)],
            1,
            256,
            area,
            &SharedBudget::new(0),
            &|| Ok(())
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn diagonal_spacing_has_independent_q8_centers_at_large_positive_and_negative_origins() {
    let brush = tip(&[255], 1, 1, (0, 0));
    let budget = SharedBudget::new(1 << 20);
    // Four equally spaced one-pixel advances on a (3,3) diagonal. These
    // independently specified rounded Q8 centers are not generated using any
    // production arc-length or interpolation helper. No forced endpoint dab.
    let centers = [(256, 256), (437, 437), (618, 618), (799, 799), (980, 980)];
    let mut expected = vec![0; 49];
    for (x, y) in centers {
        let (left, top) = (x / 256, y / 256);
        let (fx, fy) = (x % 256, y % 256);
        // Independent forward splat of a single pixel to four destinations.
        for (dx, wx) in [(0, 256 - fx), (1, fx)] {
            for (dy, wy) in [(0, 256 - fy), (1, fy)] {
                let alpha = ((255 * wx * wy + 32768) / 65536) as u8;
                let dest = &mut expected[((top + dy) * 7 + left + dx) as usize];
                *dest = (*dest).max(alpha);
            }
        }
    }
    for shift in [0, COORD_ABS_MAX - 8, -COORD_ABS_MAX + 8] {
        let clip = Rect::new(shift, shift, 7, 7).unwrap();
        let result = sampled_brush::coverage(
            &brush,
            &[
                (shift * 256 + 256, shift * 256 + 256),
                (shift * 256 + 1024, shift * 256 + 1024),
            ],
            256,
            256,
            clip,
            &budget,
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(dense(&result, clip), expected);
    }
    assert_eq!(budget.usage().reserved, 0);
}
