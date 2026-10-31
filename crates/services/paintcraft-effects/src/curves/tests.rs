use super::*;
use std::cell::Cell;
fn cancelled() -> Error {
    Error::new(ErrorCode::OperationCancelled, "Curves caller cancellation")
}
#[test]
fn geometry_and_both_quotas_refuse_before_callbacks_or_publication() {
    let p = CurvesRgbV1::default();
    let q = quote(p);
    let mut scratch = Scratch::default();
    assert_eq!(q.scratch_bytes, std::mem::size_of::<Scratch>() as u64);
    assert_eq!(q.scratch_bytes, 256);
    assert_eq!(q.work, 32768);
    let visits = Cell::new(0);
    let check = || {
        visits.set(visits.get() + 1);
        Ok(())
    };
    for length in [0, 255, 257] {
        let mut output = [0xa5; 257];
        assert_eq!(
            build_lut(p, &mut scratch, &mut output[..length], q.into(), &check)
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        assert_eq!(output, [0xa5; 257]);
    }
    for limits in [
        Limits {
            scratch_bytes: 0,
            work: 0,
        },
        Limits {
            scratch_bytes: 0,
            work: q.work,
        },
        Limits {
            scratch_bytes: q.scratch_bytes - 1,
            work: q.work,
        },
        Limits {
            scratch_bytes: q.scratch_bytes,
            work: 0,
        },
        Limits {
            scratch_bytes: q.scratch_bytes,
            work: q.work - 1,
        },
    ] {
        let mut output = [0xa5; 256];
        assert_eq!(
            build_lut(p, &mut scratch, &mut output, limits, &check)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(output, [0xa5; 256]);
    }
    assert_eq!(visits.get(), 0);
}
#[test]
fn exact_quotes_charge_the_fixed_model_and_gate_all_entries_before_final_copy() {
    let p = CurvesRgbV1::default();
    let q = quote(p);
    let mut scratch = Scratch::default();
    let mut output = [0xa5; 256];
    let visits = Cell::new(0);
    let used = build_lut(p, &mut scratch, &mut output, q.into(), &|| {
        visits.set(visits.get() + 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(used.units, 32768);
    assert_eq!(visits.get(), 258);
    for (input, actual) in output.iter().enumerate() {
        assert_eq!(*actual, input as u8);
    }
}
#[test]
fn one_shot_initial_middle_last_and_final_cancellation_preserves_output_and_reuses_scratch() {
    let p = CurvesRgbV1::new([255, 0, 255, 0, 255]);
    let q = quote(p);
    let mut scratch = Scratch::default();
    for stop in [1, 2, 129, 257, 258] {
        let mut output = [0xa5; 256];
        let visits = Cell::new(0);
        let error = build_lut(p, &mut scratch, &mut output, q.into(), &|| {
            let next = visits.get() + 1;
            visits.set(next);
            if next == stop {
                Err(cancelled())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(visits.get(), stop);
        assert_eq!(output, [0xa5; 256]);
        // No stale scratch prefix can escape a later successful identity compile.
        build_lut(
            CurvesRgbV1::default(),
            &mut scratch,
            &mut output,
            q.into(),
            &|| Ok(()),
        )
        .unwrap();
        for (input, actual) in output.iter().enumerate() {
            assert_eq!(*actual, input as u8);
        }
    }
}
#[test]
fn exact_last_span_centered_tangent_and_half_rounding_literals_keep_free_endpoints() {
    // Independent Fraction-planning literals; full oracle is a separate module.
    for (points, cases) in [
        (
            [0, 0, 0, 0, 254],
            &[
                (191, 0),
                (192, 0),
                (193, 2),
                (223, 109),
                (254, 250),
                (255, 254),
            ][..],
        ),
        (
            [0, 8, 251, 9, 243],
            &[
                (127, 251),
                (128, 251),
                (129, 251),
                (159, 136),
                (191, 9),
                (192, 9),
                (193, 9),
                (223, 94),
                (254, 239),
                (255, 243),
            ][..],
        ),
        (
            [23, 9, 240, 73, 201],
            &[
                (0, 23),
                (63, 7),
                (64, 9),
                (127, 239),
                (128, 240),
                (192, 73),
                (223, 117),
                (255, 201),
            ][..],
        ),
        ([0, 0, 255, 0, 255], &[(160, 128)][..]),
    ] {
        let p = CurvesRgbV1::new(points);
        let mut output = [0xa5; 256];
        build_lut(
            p,
            &mut Scratch::default(),
            &mut output,
            quote(p).into(),
            &|| Ok(()),
        )
        .unwrap();
        for &(input, expected) in cases {
            assert_eq!(output[input], expected, "{points:?} input{input}");
        }
        for (input, expected) in [0, 64, 128, 192, 255].into_iter().zip(points) {
            assert_eq!(output[input], expected);
        }
    }
    for points in [[0; 5], [255; 5], [255, 0, 255, 0, 255], [0, 255, 0, 255, 0]] {
        let p = CurvesRgbV1::new(points);
        let mut output = [0xa5; 256];
        build_lut(
            p,
            &mut Scratch::default(),
            &mut output,
            quote(p).into(),
            &|| Ok(()),
        )
        .unwrap();
        for (input, expected) in [0, 64, 128, 192, 255].into_iter().zip(points) {
            assert_eq!(output[input], expected);
        }
    }
}
#[test]
fn lookup_changes_hidden_rgb_without_changing_alpha_or_applying_a_mask() {
    let table = std::array::from_fn(|input| 255 - input as u8);
    for alpha in [0, 128, 255] {
        assert_eq!(
            evaluate_pixel(&table, Rgba([0, 64, 255, alpha])),
            Rgba([255, 191, 0, alpha])
        );
    }
}
