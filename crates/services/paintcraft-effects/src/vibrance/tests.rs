use super::*;
use std::cell::Cell;
fn policy(a: i16) -> VibranceRgbV1 {
    VibranceRgbV1::new(a).unwrap()
}
fn cancelled() -> Error {
    Error::new(ErrorCode::OperationCancelled, "test cancellation")
}

#[test]
fn geometry_and_work_refuse_before_callbacks_and_output_changes() {
    let source = [Rgba([20, 115, 200, 0]); 2];
    let sentinel = Rgba([201, 202, 203, 204]);
    let mut output = [sentinel; 2];
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let q = quote(2).unwrap();
    assert_eq!(q.scratch_bytes, 0);
    assert_eq!(q.work, 128);
    assert_eq!(
        evaluate_row(policy(100), &source[..1], &mut output, q.into(), &check)
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        evaluate_row(
            policy(100),
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
    assert_eq!(calls.get(), 0);
    assert_eq!(output, [sentinel; 2]);
    assert_eq!(source, [Rgba([20, 115, 200, 0]); 2]);
    if usize::BITS == 64 {
        assert_eq!(
            quote(usize::MAX).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
}

#[test]
fn initial_chunk_and_final_cancellation_have_explicit_private_output_boundary() {
    let source = [Rgba([20, 115, 200, 0]); 130];
    let sentinel = Rgba([201, 202, 203, 204]);
    let candidate = Rgba([4, 116, 216, 0]);
    let q = quote(source.len()).unwrap();
    for (stop, written) in [(1, 0), (2, 0), (3, 64), (4, 128), (5, 130)] {
        let mut output = [sentinel; 130];
        let calls = Cell::new(0);
        let result = evaluate_row(policy(100), &source, &mut output, q.into(), &|| {
            calls.set(calls.get() + 1);
            if calls.get() == stop {
                Err(cancelled())
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(calls.get(), stop);
        assert!(output[..written].iter().all(|p| *p == candidate));
        assert!(output[written..].iter().all(|p| *p == sentinel));
        // Reuse the same caller output after discarding the canceled result.
        let used = evaluate_row(policy(100), &source, &mut output, q.into(), &|| Ok(())).unwrap();
        assert_eq!(used.units, 130 * 64);
        assert_eq!(output, [candidate; 130]);
        assert_eq!(source, [Rgba([20, 115, 200, 0]); 130]);
    }
}

#[test]
fn empty_identity_grayscale_saturation_and_alpha_follow_declared_work_and_bytes() {
    let calls = Cell::new(0);
    let q = quote(0).unwrap();
    assert_eq!(
        evaluate_row(policy(100), &[], &mut [], q.into(), &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap()
        .units,
        0
    );
    assert_eq!(calls.get(), 2);
    for a in [-100, -1, 0, 1, 100] {
        for v in 0..=255 {
            let incoming = Rgba([v, v, v, 0]);
            assert_eq!(evaluate_pixel(policy(a), incoming).unwrap(), incoming);
        }
        for incoming in [
            Rgba([255, 0, 0, 0]),
            Rgba([0, 255, 0, 128]),
            Rgba([0, 0, 255, 255]),
            Rgba([255, 255, 0, 1]),
        ] {
            assert_eq!(evaluate_pixel(policy(a), incoming).unwrap(), incoming);
        }
    }
    let source = [Rgba([20, 115, 200, 0])];
    for a in [0, 100] {
        let mut output = [Rgba([0; 4])];
        assert_eq!(
            evaluate_row(
                policy(a),
                &source,
                &mut output,
                quote(1).unwrap().into(),
                &|| Ok(())
            )
            .unwrap()
            .units,
            64
        );
        assert_eq!(output[0].0[3], 0);
        if a == 0 {
            assert_eq!(output, source);
        } else {
            assert_ne!(output, source);
        }
    }
}
