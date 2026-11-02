use super::*;
use std::cell::Cell;
fn policy() -> ColorOverlayRgbV1 {
    ColorOverlayRgbV1::new([220, 40, 0], 128, EffectBlendMode::Normal)
}
fn cancelled() -> Error {
    Error::new(ErrorCode::OperationCancelled, "test")
}
#[test]
fn literals_hidden_alpha_identity_and_complete_candidate() {
    for a in [0, 1, 127, 128, 254, 255] {
        assert_eq!(
            evaluate_pixel(policy(), Rgba([20, 80, 200, a])).unwrap(),
            Rgba([120, 60, 100, a])
        );
        for mode in [
            EffectBlendMode::Normal,
            EffectBlendMode::Multiply,
            EffectBlendMode::Screen,
        ] {
            let p = ColorOverlayRgbV1::new([1, 2, 3], 0, mode);
            let input = Rgba([17, 93, 201, a]);
            assert_eq!(evaluate_pixel(p, input).unwrap(), input);
        }
    }
}
#[test]
fn geometry_refusal_precedes_callback_and_writes() {
    let source = [Rgba([1, 2, 3, 4]); 2];
    let mut output = [Rgba([199; 4]); 1];
    let error = evaluate_row(policy(), &source, &mut output, &|| {
        panic!("geometry callback")
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InputSchema);
    assert_eq!(output, [Rgba([199; 4]); 1]);
}
#[test]
fn chunk_cancellation_reuse_source_and_output_guards() {
    let source = vec![Rgba([20, 80, 200, 127]); 130];
    let frozen = source.clone();
    // initial, chunks0/1/2, final; all five causal cancellation boundaries.
    for fail in 1..=5 {
        let calls = Cell::new(0);
        let mut guarded = vec![Rgba([199; 4]); 134];
        let error = evaluate_row(policy(), &source, &mut guarded[2..132], &|| {
            calls.set(calls.get() + 1);
            if calls.get() == fail {
                Err(cancelled())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(calls.get(), fail);
        let written = match fail {
            1 | 2 => 0,
            3 => 64,
            4 => 128,
            5 => 130,
            _ => unreachable!(),
        };
        assert!(
            guarded[2..2 + written]
                .iter()
                .all(|p| *p == Rgba([120, 60, 100, 127]))
        );
        assert!(
            guarded[2 + written..132]
                .iter()
                .all(|p| *p == Rgba([199; 4]))
        );
        assert_eq!(&guarded[..2], &[Rgba([199; 4]); 2]);
        assert_eq!(&guarded[132..], &[Rgba([199; 4]); 2]);
        assert_eq!(source, frozen);
        evaluate_row(policy(), &source, &mut guarded[2..132], &|| Ok(())).unwrap();
        assert!(
            guarded[2..132]
                .iter()
                .all(|p| *p == Rgba([120, 60, 100, 127]))
        );
    }
    let calls = Cell::new(0);
    evaluate_row(policy(), &[], &mut [], &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(calls.get(), 2);
}
