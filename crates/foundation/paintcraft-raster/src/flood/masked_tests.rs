use super::*;
use std::{cell::Cell, collections::VecDeque};

fn oracle(pixels: &[Rgba; 6], selected: u8, seed: usize, tolerance: u8) -> [bool; 6] {
    let mut reached = [false; 6];
    if selected >> seed & 1 == 0 {
        return reached;
    }
    let mut queue = VecDeque::from([seed]);
    reached[seed] = true;
    while let Some(index) = queue.pop_front() {
        let (x, y) = (index % 3, index / 3);
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if !(0..3).contains(&nx) || !(0..2).contains(&ny) {
                continue;
            }
            let next = ny as usize * 3 + nx as usize;
            if reached[next] || selected >> next & 1 == 0 {
                continue;
            }
            // Independent explicit channel comparison, always against the seed.
            if (0..4).all(|channel| {
                let difference =
                    i16::from(pixels[next].0[channel]) - i16::from(pixels[seed].0[channel]);
                difference.abs() <= i16::from(tolerance)
            }) {
                reached[next] = true;
                queue.push_back(next);
            }
        }
    }
    reached
}

#[test]
fn every_small_binary_mask_and_two_color_image_matches_independent_bfs() {
    let colors = [Rgba([10, 30, 50, 70]), Rgba([27, 20, 60, 80])];
    for offset in [(0, 0), (-257, 255)] {
        let clip = Rect::new(offset.0, offset.1, 3, 2).unwrap();
        let index = |x, y| (y - offset.1) as usize * 3 + (x - offset.0) as usize;
        for image in 0u8..64 {
            let pixels = std::array::from_fn(|i| colors[(image >> i & 1) as usize]);
            for selected in 0u8..64 {
                for seed in 0..6 {
                    for tolerance in [0, 16, 17] {
                        let spans = region_from_source_masked_checked(
                            |x, y| {
                                assert!(clip.contains(x, y));
                                let index = index(x, y);
                                assert_ne!(selected >> index & 1, 0, "forbidden source read");
                                Ok(pixels[index])
                            },
                            |x, y| {
                                assert!(clip.contains(x, y));
                                Ok(selected >> index(x, y) & 1 != 0)
                            },
                            (offset.0 + seed as i64 % 3, offset.1 + seed as i64 / 3),
                            &clip,
                            tolerance,
                            8192,
                            &|| Ok(()),
                        )
                        .unwrap();
                        let mut actual = [false; 6];
                        for span in spans {
                            for x in span.x0..=span.x1 {
                                let index = index(x, span.y);
                                assert!(!actual[index], "duplicate emitted sample");
                                actual[index] = true;
                            }
                        }
                        assert_eq!(actual, oracle(&pixels, selected, seed, tolerance));
                    }
                }
            }
        }
    }
}

#[test]
fn forbidden_seed_needs_no_scratch_or_source_read_even_for_oversized_clip() {
    let clip = Rect::new(-5, -7, 1_048_576, 1_048_576).unwrap();
    let predicate_calls = Cell::new(0);
    let spans = region_from_source_masked_checked(
        |_, _| panic!("unselected seed must not read pixels"),
        |x, y| {
            assert_eq!((x, y), (-5, -7));
            predicate_calls.set(predicate_calls.get() + 1);
            Ok(false)
        },
        (-5, -7),
        &clip,
        255,
        0,
        &|| Ok(()),
    )
    .unwrap();
    assert!(spans.is_empty());
    assert_eq!(predicate_calls.get(), 1);
    assert!(
        region_from_source_masked_checked(
            |_, _| panic!("outside seed must not read pixels"),
            |_, _| panic!("outside seed must not inspect mask"),
            (-6, -7),
            &clip,
            255,
            0,
            &|| Ok(()),
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn selection_barriers_disconnect_identical_color_islands_and_diagonals() {
    let clip = Rect::new(0, 0, 5, 3).unwrap();
    for seed in [(0, 0), (4, 0)] {
        let spans = region_from_source_masked_checked(
            |x, _| {
                assert_ne!(x, 2, "barrier pixels must never be read");
                Ok(Rgba([20; 4]))
            },
            |x, _| Ok(x != 2),
            seed,
            &clip,
            255,
            8192,
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(spans.iter().map(|s| s.x1 - s.x0 + 1).sum::<i64>(), 6);
        assert!(
            spans
                .iter()
                .all(|s| if seed.0 == 0 { s.x1 < 2 } else { s.x0 > 2 })
        );
    }
    let spans = region_from_source_masked_checked(
        |_, _| Ok(Rgba([20; 4])),
        |x, y| Ok(x == y),
        (0, 0),
        &clip,
        255,
        8192,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(spans.len(), 1);
    assert_eq!((spans[0].y, spans[0].x0, spans[0].x1), (0, 0, 0));
}

#[test]
fn every_predicate_failure_discards_classification_and_preserves_error() {
    let clip = Rect::new(0, 0, 8, 4).unwrap();
    let calls = Cell::new(0);
    let baseline = region_from_source_masked_checked(
        |_, _| Ok(Rgba([42; 4])),
        |_, _| {
            calls.set(calls.get() + 1);
            Ok(true)
        },
        (3, 1),
        &clip,
        0,
        8192,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(baseline.iter().map(|s| s.x1 - s.x0 + 1).sum::<i64>(), 32);
    for failure in 1..=calls.get() {
        for code in [ErrorCode::OperationCancelled, ErrorCode::ResourceLimit] {
            let seen = Cell::new(0);
            let error = region_from_source_masked_checked(
                |_, _| Ok(Rgba([42; 4])),
                |_, _| {
                    seen.set(seen.get() + 1);
                    if seen.get() == failure {
                        Err(Error::new(code, "mask predicate failed"))
                    } else {
                        Ok(true)
                    }
                },
                (3, 1),
                &clip,
                0,
                8192,
                &|| Ok(()),
            )
            .unwrap_err();
            assert_eq!(error.code, code);
            assert_eq!(error.message, "mask predicate failed");
            assert_eq!(seen.get(), failure);
        }
    }
}
