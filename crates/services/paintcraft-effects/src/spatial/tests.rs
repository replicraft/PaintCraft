//! Geometry, storage and execution controls; numeric corpus has a separate oracle.
use super::*;
use std::cell::Cell;

fn rect(x: i64, y: i64, width: i64, height: i64) -> Rect {
    Rect::new(x, y, width, height).unwrap()
}
fn plan(source: Rect, output: Rect, radius: i16) -> BoxBlurPlan {
    BoxBlurPlan::new(source, output, BoxBlurV1::new(radius).unwrap()).unwrap()
}
fn field(source: Rect) -> Vec<Rgba> {
    (0..source.height())
        .flat_map(|y| {
            (0..source.width()).map(move |x| {
                Rgba([
                    ((x * 37 + y * 11) % 256) as u8,
                    ((x * 13 + y * 41) % 256) as u8,
                    ((x * 17 + y * 7) % 256) as u8,
                    ((x * 29 + y * 43) % 256) as u8,
                ])
            })
        })
        .collect()
}
fn extract(source: Rect, pixels: &[Rgba], area: Rect) -> Vec<Rgba> {
    let mut output = Vec::new();
    for y in area.y0..area.y1 {
        let start = ((y - source.y0) * source.width() + (area.x0 - source.x0)) as usize;
        output.extend_from_slice(&pixels[start..start + area.width() as usize]);
    }
    output
}
fn run(plan: BoxBlurPlan, input: &[Rgba]) -> Vec<Rgba> {
    let mut output = vec![Rgba([13, 17, 19, 23]); plan.output_pixels()];
    let mut scratch = vec![u64::MAX; plan.scratch_elements() + 7];
    box_blur(plan, input, &mut output, &mut scratch, &|| Ok(())).unwrap();
    assert!(
        scratch[plan.scratch_elements()..]
            .iter()
            .all(|&v| v == u64::MAX)
    );
    output
}

#[test]
fn zero_radius_preserves_every_hidden_byte_and_uses_no_scratch() {
    let source = rect(-17, -23, 130, 1);
    let input = field(source);
    let original = input.clone();
    let request = plan(source, source, 0);
    assert_eq!(request.required_input(), source);
    assert_eq!(request.output(), source);
    assert_eq!(request.input_pixels(), 130);
    assert_eq!(request.output_pixels(), 130);
    assert_eq!(request.scratch_elements(), 0);
    assert_eq!(request.storage_bytes(), 130 * 8);
    assert_eq!(request.sample_visits(), 130);
    assert_eq!(run(request, &input), input);
    assert_eq!(input, original);
    for hidden in [Rgba([255, 17, 91, 0]), Rgba([1, 2, 3, 1])] {
        let tiny = rect(-1, -1, 1, 1);
        assert_eq!(run(plan(tiny, tiny, 0), &[hidden]), [hidden]);
    }
}

#[test]
fn true_finite_edges_and_interior_stripes_have_distinct_input_footprints() {
    let source = rect(-10, -4, 3, 1);
    let input = [
        Rgba([0, 0, 0, 255]),
        Rgba([90, 60, 30, 255]),
        Rgba([240, 120, 0, 255]),
    ];
    let expected = [
        Rgba([30, 20, 10, 255]),
        Rgba([110, 60, 10, 255]),
        Rgba([190, 100, 10, 255]),
    ];
    assert_eq!(run(plan(source, source, 1), &input), expected);
    for (column, &pixel) in expected.iter().enumerate() {
        let output = rect(source.x0 + column as i64, source.y0, 1, 1);
        let request = plan(source, output, 1);
        let part = extract(source, &input, request.required_input());
        assert_eq!(run(request, &part), [pixel]);
        if column == 1 {
            assert_eq!(request.required_input(), source);
            assert_ne!(pixel, input[1], "An interior stripe is not a source edge");
        }
    }
    let tiny = rect(7, -8, 1, 1);
    for radius in [1, 32] {
        assert_eq!(
            run(plan(tiny, tiny, radius), &[Rgba([33, 127, 250, 255])]),
            [Rgba([33, 127, 250, 255])]
        );
        assert_eq!(
            run(plan(tiny, tiny, radius), &[Rgba([33, 127, 250, 0])]),
            [Rgba([0; 4])]
        );
    }
}

#[test]
fn arbitrary_partitions_equal_whole_source_at_negative_origins() {
    let source = rect(-133, -71, 9, 5);
    let input = field(source);
    let original = input.clone();
    for radius in [0, 1, 2, 32] {
        let expected = run(plan(source, source, radius), &input);
        for edge in [1, 2, 3, 8, 128, 256] {
            let mut assembled = vec![Rgba([0; 4]); input.len()];
            for y in (0..source.height()).step_by(edge) {
                for x in (0..source.width()).step_by(edge) {
                    let output = rect(
                        source.x0 + x,
                        source.y0 + y,
                        (edge as i64).min(source.width() - x),
                        (edge as i64).min(source.height() - y),
                    );
                    let request = plan(source, output, radius);
                    let pixels = run(request, &extract(source, &input, request.required_input()));
                    for row in 0..output.height() as usize {
                        let start = (y as usize + row) * source.width() as usize + x as usize;
                        let width = output.width() as usize;
                        assembled[start..start + width]
                            .copy_from_slice(&pixels[row * width..(row + 1) * width]);
                    }
                }
            }
            assert_eq!(assembled, expected, "radius={radius}, edge={edge}");
        }
    }
    assert_eq!(input, original);
}

#[test]
fn translated_extreme_signed_origins_preserve_the_same_finite_sampling() {
    let ordinary = rect(0, 0, 3, 3);
    let input = field(ordinary);
    for radius in [0, 1, 32] {
        let expected = run(plan(ordinary, ordinary, radius), &input);
        for (x, y) in [
            (i64::MIN, i64::MIN),
            (i64::MAX - 3, i64::MAX - 3),
            (i64::MIN, i64::MAX - 3),
        ] {
            let source = rect(x, y, 3, 3);
            assert_eq!(run(plan(source, source, radius), &input), expected);
            let middle = rect(x + 1, y + 1, 1, 1);
            let request = plan(source, middle, radius);
            assert_eq!(
                run(request, &extract(source, &input, request.required_input())),
                [expected[4]]
            );
        }
    }
}

#[test]
fn exact_storage_quote_and_invalid_public_rectangles_fail_without_allocation() {
    let source = rect(-9, -7, 20, 10);
    let output = rect(-7, -5, 5, 5);
    let request = plan(source, output, 2);
    assert_eq!(request.required_input(), rect(-9, -7, 9, 9));
    assert_eq!(request.input_pixels(), 81);
    assert_eq!(request.output_pixels(), 25);
    assert_eq!(request.scratch_elements(), 180);
    assert_eq!(request.storage_bytes(), 81 * 4 + 25 * 4 + 180 * 8);
    // Nine horizontal rows and five vertical columns each visit five initial
    // taps, then two new/old samples for each of the four subsequent positions.
    assert_eq!(request.sample_visits(), 9 * 13 + 5 * 13);
    let bad = [
        Rect {
            x0: 1,
            y0: 0,
            x1: 0,
            y1: 1,
        },
        Rect {
            x0: 0,
            y0: 0,
            x1: 0,
            y1: 1,
        },
        Rect {
            x0: 0,
            y0: 2,
            x1: 1,
            y1: 1,
        },
        Rect {
            x0: i64::MIN,
            y0: 0,
            x1: i64::MAX,
            y1: 1,
        },
    ];
    for malformed in bad {
        assert_eq!(
            BoxBlurPlan::new(malformed, output, BoxBlurV1::default())
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        assert_eq!(
            BoxBlurPlan::new(source, malformed, BoxBlurV1::default())
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    assert_eq!(
        BoxBlurPlan::new(source, rect(-10, -7, 1, 1), BoxBlurV1::default())
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    let huge = Rect {
        x0: 0,
        y0: 0,
        x1: i64::MAX,
        y1: i64::MAX,
    };
    assert_eq!(
        BoxBlurPlan::new(huge, huge, BoxBlurV1::default())
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}

#[test]
fn invalid_slice_lengths_refuse_before_callback_or_private_buffer_writes() {
    let bounds = rect(0, 0, 3, 2);
    let request = plan(bounds, bounds, 1);
    let input = field(bounds);
    for (input_len, output_len, scratch_len) in
        [(5, 6, 24), (7, 6, 24), (6, 5, 24), (6, 7, 24), (6, 6, 23)]
    {
        let mut supplied = input.clone();
        supplied.resize(input_len, Rgba([99; 4]));
        let before_source = supplied.clone();
        let mut output = vec![Rgba([17; 4]); output_len];
        let mut scratch = vec![u64::MAX; scratch_len];
        let error = box_blur(request, &supplied, &mut output, &mut scratch, &|| {
            panic!("Malformed storage must precede cancellation/work")
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::InputSchema);
        assert_eq!(supplied, before_source);
        assert!(output.iter().all(|&p| p == Rgba([17; 4])));
        assert!(scratch.iter().all(|&p| p == u64::MAX));
    }
}

#[test]
fn cancellation_at_every_observed_phase_never_changes_source_or_scratch_tail() {
    let source = rect(-20, -30, 10, 72);
    let output_rect = rect(-19, -29, 8, 70);
    let request = plan(source, output_rect, 1);
    let input = field(source);
    assert_eq!(request.required_input(), source);
    let original = input.clone();
    let expected = run(request, &input);
    let calls = Cell::new(0);
    let mut output = vec![Rgba([251, 252, 253, 254]); request.output_pixels()];
    let mut scratch = vec![u64::MAX; request.scratch_elements() + 9];
    box_blur(request, &input, &mut output, &mut scratch, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    let total = calls.get();
    assert!(total > 3);
    let mut early = false;
    let mut scratch_only = false;
    let mut partial_output = false;
    let mut completed_but_cancelled = false;
    for cancel_at in 1..=total {
        let count = Cell::new(0);
        let mut output = vec![Rgba([251, 252, 253, 254]); request.output_pixels()];
        let mut scratch = vec![u64::MAX; request.scratch_elements() + 9];
        let error = box_blur(request, &input, &mut output, &mut scratch, &|| {
            count.set(count.get() + 1);
            if count.get() == cancel_at {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "phase cancellation",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(count.get(), cancel_at);
        assert_eq!(input, original);
        assert!(
            scratch[request.scratch_elements()..]
                .iter()
                .all(|&v| v == u64::MAX)
        );
        let writes = output
            .iter()
            .filter(|&&p| p != Rgba([251, 252, 253, 254]))
            .count();
        early |= scratch.iter().all(|&v| v == u64::MAX) && writes == 0;
        scratch_only |= scratch[..request.scratch_elements()]
            .iter()
            .any(|&v| v != u64::MAX)
            && writes == 0;
        partial_output |= writes > 0 && writes < output.len();
        completed_but_cancelled |= output == expected;
    }
    assert!(early && scratch_only && partial_output && completed_but_cancelled);
}

#[test]
fn zero_radius_cancellation_is_chunked_and_spatial_mix_retains_exact_endpoints() {
    let bounds = rect(0, 0, 130, 1);
    let request = plan(bounds, bounds, 0);
    let input = field(bounds);
    let mut output = vec![Rgba([255; 4]); 130];
    let mut scratch = [91; 7];
    let calls = Cell::new(0);
    let error = box_blur(request, &input, &mut output, &mut scratch, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == 3 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "after first chunk",
            ))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(&output[..64], &input[..64]);
    assert!(output[64..].iter().all(|&p| p == Rgba([255; 4])));
    assert_eq!(scratch, [91; 7]);
    let p = Rgba([250, 7, 99, 0]);
    let q = Rgba([4, 180, 33, 211]);
    assert_eq!(mix_spatial_coverage(p, q, 0), p);
    assert_eq!(mix_spatial_coverage(p, q, 255), q);
    for coverage in 0..=255 {
        assert_eq!(mix_spatial_coverage(p, p, coverage), p);
        assert_eq!(mix_spatial_coverage(q, q, coverage), q);
    }
    assert_ne!(mix_spatial_coverage(p, q, 128).a(), p.a());
}

#[test]
fn affected_output_expands_halo_damage_and_clips_only_at_true_source_edges() {
    let source = rect(-10, -20, 100, 60);
    let changed = rect(-5, -13, 1, 1);
    assert_eq!(
        box_affected_output(source, changed, BoxBlurV1::new(0).unwrap()).unwrap(),
        changed
    );
    let affected = box_affected_output(source, changed, BoxBlurV1::new(2).unwrap()).unwrap();
    assert_eq!(affected, rect(-7, -15, 5, 5));
    assert!(!changed.contains(-7, -15));
    assert!(
        affected.contains(-7, -15),
        "A halo-only edit dependency must invalidate output"
    );
    let corner = rect(-10, -20, 1, 1);
    assert_eq!(
        box_affected_output(source, corner, BoxBlurV1::new(2).unwrap()).unwrap(),
        rect(-10, -20, 3, 3)
    );
    let tiny = rect(i64::MAX - 3, i64::MIN, 3, 1);
    assert_eq!(
        box_affected_output(
            tiny,
            rect(i64::MAX - 2, i64::MIN, 1, 1),
            BoxBlurV1::new(32).unwrap()
        )
        .unwrap(),
        tiny
    );
    for invalid in [
        Rect {
            x0: -5,
            y0: -13,
            x1: -5,
            y1: -12,
        },
        rect(-11, -20, 1, 1),
    ] {
        assert_eq!(
            box_affected_output(source, invalid, BoxBlurV1::default())
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
}
