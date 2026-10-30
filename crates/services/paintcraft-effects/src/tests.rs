use super::*;
use paintcraft_core::types::filters::BrightnessContrastV1;
use std::cell::Cell;
fn bc(b: i16, c: i16) -> Descriptor {
    Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(b, c).unwrap())
}
#[test]
fn independently_calculated_numeric_controls_and_rounding() {
    // Direct arithmetic controls: c=50 gives gain3, c=-50 gain1/3.
    // b=10 adds25.5; b=-10 subtracts25.5; exact half rounds upward.
    let values = [0, 64, 127, 128, 192, 255];
    for (b, c, expected) in [
        (0, 0, [0, 64, 127, 128, 192, 255]),
        (10, 0, [26, 90, 153, 154, 218, 255]),
        (-10, 0, [0, 39, 102, 103, 167, 230]),
        (0, 50, [0, 0, 126, 129, 255, 255]),
        (0, -50, [85, 106, 127, 128, 149, 170]),
        (100, 0, [255; 6]),
        (-100, 0, [0; 6]),
        (0, 99, [0, 0, 28, 227, 255, 255]),
        // Clamp once after combined brightness/contrast, not between them.
        (100, 99, [0, 0, 255, 255, 255, 255]),
        (-100, 99, [0, 0, 0, 0, 255, 255]),
    ] {
        for (input, expected) in values.into_iter().zip(expected) {
            assert_eq!(
                evaluate_pixel(&[bc(b, c)], Rgba([input; 4])).unwrap(),
                Rgba([expected, expected, expected, input]),
                "b={b},c={c},v={input}"
            );
        }
    }
}
#[test]
fn no_op_inversion_alpha_hidden_rgb_and_order_are_exact() {
    for v in 0..=255 {
        let source = Rgba([v, 255 - v, 73, 0]);
        assert_eq!(evaluate_pixel(&[], source).unwrap(), source);
        assert_eq!(evaluate_pixel(&[bc(0, 0)], source).unwrap(), source);
        assert_eq!(
            evaluate_pixel(&[Descriptor::InvertRgbV1], source).unwrap(),
            Rgba([255 - v, v, 182, 0])
        );
        assert_eq!(
            evaluate_pixel(&[Descriptor::InvertRgbV1; 2], source).unwrap(),
            source
        );
    }
    let input = Rgba([0, 100, 255, 31]);
    assert_eq!(
        evaluate_pixel(&[bc(10, 0), Descriptor::InvertRgbV1], input).unwrap(),
        Rgba([229, 129, 0, 31])
    );
    assert_eq!(
        evaluate_pixel(&[Descriptor::InvertRgbV1, bc(10, 0)], input).unwrap(),
        Rgba([255, 181, 26, 31])
    );
}
#[test]
fn all_parameter_boundaries_validate_before_evaluation() {
    for (b, c) in [
        (i16::MIN, 0),
        (i16::MAX, 0),
        (-101, 0),
        (101, 0),
        (0, -100),
        (0, 100),
        (0, i16::MAX),
    ] {
        assert_eq!(
            BrightnessContrastV1::new(b, c).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
    for b in [-100, 0, 100] {
        for c in [-99, 0, 99] {
            assert!(BrightnessContrastV1::new(b, c).is_ok());
        }
    }
    assert_eq!(
        Descriptor::checked("invert-rgb-v2", None, None)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        Descriptor::checked("invert-rgb-v1", Some(0), None)
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        Descriptor::checked("brightness-contrast-v1", Some(0), None)
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        Descriptor::checked("brightness-contrast-v1", Some(-100), Some(-99)).unwrap(),
        bc(-100, -99)
    );
    assert_eq!(Descriptor::InvertRgbV1.id(), "invert-rgb-v1");
    assert_eq!(bc(0, 0).id(), "brightness-contrast-v1");
}
#[test]
fn row_validation_and_cancellation_never_claim_atomic_output() {
    let source = vec![Rgba([1, 2, 3, 4]); 130];
    let sentinel = Rgba([9; 4]);
    let mut output = vec![sentinel; 130];
    let cancelled = || {
        Err(Error::new(
            ErrorCode::OperationCancelled,
            "test cancellation",
        ))
    };
    assert_eq!(
        evaluate_row(&[], &source, &mut output[..129], &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert!(output.iter().all(|p| *p == sentinel));
    assert_eq!(
        evaluate_row(&[], &source, &mut output, &cancelled)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert!(output.iter().all(|p| *p == sentinel));
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() == 4 {
            cancelled()
        } else {
            Ok(())
        }
    };
    assert_eq!(
        evaluate_row(&[Descriptor::InvertRgbV1], &source, &mut output, &check)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(calls.get(), 4);
    assert!(output[..64].iter().all(|p| *p == Rgba([254, 253, 252, 4])));
    assert!(output[64..].iter().all(|p| *p == sentinel));
    evaluate_row(&[bc(10, 0)], &source, &mut output, &|| Ok(())).unwrap();
    assert!(output.iter().all(|p| *p == Rgba([27, 28, 29, 4])));
    let final_calls = Cell::new(0);
    let check = || {
        final_calls.set(final_calls.get() + 1);
        if final_calls.get() == 5 {
            cancelled()
        } else {
            Ok(())
        }
    };
    assert!(evaluate_row(&[], &source, &mut output, &check).is_err());
    assert_eq!(
        output, source,
        "even fully written output must be discarded on final cancellation"
    );
}
#[test]
fn row_partitions_and_empty_rows_match_scalar_pipeline() {
    let source = (0..257)
        .map(|i| Rgba([i as u8, (i / 2) as u8, 7, (i % 256) as u8]))
        .collect::<Vec<_>>();
    let filters = [bc(-19, 23), Descriptor::InvertRgbV1, bc(2, -17)];
    let expected = source
        .iter()
        .map(|p| evaluate_pixel(&filters, *p).unwrap())
        .collect::<Vec<_>>();
    for partition in [1, 2, 3, 64, 128, 256] {
        let mut output = vec![Rgba([0; 4]); source.len()];
        for (input, output) in source.chunks(partition).zip(output.chunks_mut(partition)) {
            evaluate_row(&filters, input, output, &|| Ok(())).unwrap();
        }
        assert_eq!(output, expected);
    }
    evaluate_row(&filters, &[], &mut [], &|| Ok(())).unwrap();
}

#[test]
fn cancellation_checks_each_descriptor_even_for_one_pixel() {
    let sentinel = Rgba([9; 4]);
    let mut output = [sentinel];
    let filters = vec![Descriptor::InvertRgbV1; 4096];
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() == 5 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "stack cancellation",
            ))
        } else {
            Ok(())
        }
    };
    assert_eq!(
        evaluate_row(&filters, &[Rgba([1, 2, 3, 0])], &mut output, &check)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(calls.get(), 5);
    assert_eq!(
        output[0],
        Rgba([1, 2, 3, 0]),
        "two stages written privately; discard on Err"
    );
}
