use super::*;
use std::cell::Cell;
fn p(h: i16, s: i16, l: i16) -> HueSaturationRgbV1 {
    HueSaturationRgbV1::new(h, s, l).unwrap()
}
fn cancelled() -> Error {
    Error::new(ErrorCode::OperationCancelled, "HSL test cancellation")
}
#[test]
fn geometry_and_work_admission_refuse_before_callbacks_or_output_changes() {
    let source = [Rgba([255, 0, 0, 0]); 2];
    let sentinel = Rgba([201, 202, 203, 204]);
    let mut output = [sentinel; 2];
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let q = quote(2).unwrap();
    assert_eq!((q.scratch_bytes, q.work), (0, 256));
    assert_eq!(
        evaluate_row(p(2, 0, 0), &source[..1], &mut output, q.into(), &check)
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        evaluate_row(
            p(2, 0, 0),
            &source,
            &mut output,
            Limits {
                scratch_bytes: 0,
                work: q.work - 1
            },
            &check
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(output, [sentinel; 2]);
    assert_eq!(calls.get(), 0);
    assert_eq!(source, [Rgba([255, 0, 0, 0]); 2]);
    if usize::BITS == 64 {
        assert_eq!(
            quote(usize::MAX).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
    assert_eq!(quote(0).unwrap().work, 0);
}
#[test]
fn every_initial_chunk_final_cancel_discards_private_output_and_reuses_guarded_storage() {
    let source = [Rgba([255, 0, 0, 0]); 130];
    let candidate = Rgba([255, 9, 0, 0]);
    let sentinel = Rgba([201, 202, 203, 204]);
    let q = quote(130).unwrap();
    for (stop, written) in [(1, 0), (2, 0), (3, 64), (4, 128), (5, 130)] {
        let mut guarded = [sentinel; 134];
        let calls = Cell::new(0);
        let result = evaluate_row(p(2, 0, 0), &source, &mut guarded[2..132], q.into(), &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(cancelled())
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(calls.get(), stop);
        assert!(
            guarded[..2]
                .iter()
                .chain(&guarded[132..])
                .all(|v| *v == sentinel)
        );
        assert!(guarded[2..2 + written].iter().all(|v| *v == candidate));
        assert!(guarded[2 + written..132].iter().all(|v| *v == sentinel));
        let used = evaluate_row(p(2, 0, 0), &source, &mut guarded[2..132], q.into(), &|| {
            Ok(())
        })
        .unwrap();
        assert_eq!(used.units, 130 * 128);
        assert!(guarded[2..132].iter().all(|v| *v == candidate));
        assert!(
            guarded[..2]
                .iter()
                .chain(&guarded[132..])
                .all(|v| *v == sentinel)
        );
        assert_eq!(source, [Rgba([255, 0, 0, 0]); 130]);
    }
}
#[test]
fn all_grays_identity_additive_lightness_extremes_wrap_intent_and_fixed_work_are_exact() {
    for gray in 0..=255 {
        let input = Rgba([gray, gray, gray, 0]);
        for h in [-180, 0, 180] {
            for s in [-100, 0, 100] {
                assert_eq!(evaluate_pixel(p(h, s, 0), input).unwrap(), input);
                assert_eq!(
                    evaluate_pixel(p(h, s, -100), input).unwrap(),
                    Rgba([0, 0, 0, 0])
                );
                assert_eq!(
                    evaluate_pixel(p(h, s, 100), input).unwrap(),
                    Rgba([255, 255, 255, 0])
                );
            }
        }
    }
    for (input, policy, expected) in [
        (Rgba([0, 0, 0, 0]), p(0, 0, 10), Rgba([26, 26, 26, 0])),
        (
            Rgba([255, 255, 255, 128]),
            p(0, 0, -10),
            Rgba([230, 230, 230, 128]),
        ),
        (Rgba([0, 1, 0, 255]), p(0, -100, 0), Rgba([1, 1, 1, 255])),
        (Rgba([255, 0, 0, 0]), p(2, 0, 0), Rgba([255, 9, 0, 0])),
        (Rgba([255, 0, 0, 255]), p(-2, 0, 0), Rgba([255, 0, 9, 255])),
    ] {
        assert_eq!(evaluate_pixel(policy, input).unwrap(), expected);
    }
    let input = Rgba([20, 115, 200, 0]);
    assert_ne!(p(-180, 0, 0), p(180, 0, 0));
    assert_eq!(
        evaluate_pixel(p(-180, 0, 0), input).unwrap(),
        evaluate_pixel(p(180, 0, 0), input).unwrap()
    );
    for policy in [p(0, 0, 0), p(2, 0, 0), p(0, 0, 100), p(0, -100, 0)] {
        let mut output = [Rgba([0; 4])];
        assert_eq!(
            evaluate_row(
                policy,
                &[input],
                &mut output,
                quote(1).unwrap().into(),
                &|| Ok(())
            )
            .unwrap()
            .units,
            128
        );
        assert_eq!(output[0].0[3], 0);
    }
    let calls = Cell::new(0);
    assert_eq!(
        evaluate_row(p(0, 0, 0), &[], &mut [], quote(0).unwrap().into(), &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap()
        .units,
        0
    );
    assert_eq!(calls.get(), 2);
}
