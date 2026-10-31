use super::*;
use std::cell::Cell;
fn policy(distribution: NoiseDistribution, monochrome: bool, amount: i16) -> AddNoiseRgbV1 {
    AddNoiseRgbV1::new(amount, 1, monochrome, distribution).unwrap()
}
#[test]
fn independent_finite_literals_preserve_alpha_and_hidden_rgb() {
    // Frozen independent mathematical corpus; finite byte agreement only,
    // never authority for libm intermediate bits or a global rounding claim.
    for (d, expected) in [
        (NoiseDistribution::Uniform, Rgba([150, 133, 144, 128])),
        (NoiseDistribution::Gaussian, Rgba([134, 107, 113, 128])),
    ] {
        let p = policy(d, false, 10);
        assert_eq!(
            evaluate_pixel(p, SourcePixel::new(0, 0), Rgba([128; 4])).unwrap(),
            expected
        );
        for alpha in 0..=255 {
            let got =
                evaluate_pixel(p, SourcePixel::new(0, 0), Rgba([128, 128, 128, alpha])).unwrap();
            assert_eq!(
                got,
                Rgba([expected.0[0], expected.0[1], expected.0[2], alpha])
            );
        }
    }
}
#[test]
fn source_local_partitions_and_explicit_max_coordinate_match_scalar_candidates() {
    let source = [Rgba([64, 128, 192, 17]); 130];
    for d in [NoiseDistribution::Uniform, NoiseDistribution::Gaussian] {
        for mono in [false, true] {
            let p = policy(d, mono, 10);
            let origin = SourcePixel::new(u32::MAX - 129, u32::MAX);
            let mut whole = [Rgba([0; 4]); 130];
            let q = quote(p, source.len()).unwrap();
            assert_eq!(
                evaluate_row(p, origin, &source, &mut whole, q.into(), &|| Ok(()))
                    .unwrap()
                    .units,
                q.work
            );
            for (i, pixel) in whole.iter().enumerate() {
                assert_eq!(
                    *pixel,
                    evaluate_pixel(
                        p,
                        SourcePixel::new(origin.x() + i as u32, origin.y()),
                        source[i]
                    )
                    .unwrap()
                );
            }
            for width in [1, 31, 32, 63, 64, 127, 128] {
                let mut partitioned = [Rgba([0; 4]); 130];
                for start in (0..130).step_by(width) {
                    let end = (start + width).min(130);
                    evaluate_row(
                        p,
                        SourcePixel::new(origin.x() + start as u32, origin.y()),
                        &source[start..end],
                        &mut partitioned[start..end],
                        quote(p, end - start).unwrap().into(),
                        &|| Ok(()),
                    )
                    .unwrap();
                }
                assert_eq!(partitioned, whole);
            }
        }
    }
}
#[test]
fn geometry_coordinate_and_work_refusal_precede_callbacks_and_writes() {
    let source = [Rgba([20, 115, 200, 0]); 2];
    for d in [NoiseDistribution::Uniform, NoiseDistribution::Gaussian] {
        let p = policy(d, true, 0);
        let q = quote(p, 2).unwrap();
        assert!(q.work > 0);
        assert_eq!(q.scratch_bytes, 0);
        assert_eq!(quote(p, 2), quote(policy(d, false, 100), 2));
        let calls = Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        let mut output = [Rgba([7; 4]); 2];
        assert_eq!(
            evaluate_row(
                p,
                SourcePixel::new(0, 0),
                &source,
                &mut output[..1],
                q.into(),
                &check
            )
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
        assert_eq!(
            evaluate_row(
                p,
                SourcePixel::new(u32::MAX, 0),
                &source,
                &mut output,
                q.into(),
                &check
            )
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
        assert_eq!(
            evaluate_row(
                p,
                SourcePixel::new(0, 0),
                &source,
                &mut output,
                Limits {
                    work: q.work - 1,
                    scratch_bytes: 0
                },
                &check
            )
            .unwrap_err()
            .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(calls.get(), 0);
        assert_eq!(output, [Rgba([7; 4]); 2]);
        if usize::BITS == 64 {
            assert_eq!(
                quote(p, usize::MAX).unwrap_err().code,
                ErrorCode::ResourceLimit
            );
        }
        evaluate_row(
            p,
            SourcePixel::new(u32::MAX, u32::MAX),
            &source[..1],
            &mut output[..1],
            quote(p, 1).unwrap().into(),
            &check,
        )
        .unwrap();
        assert_eq!(output[0], source[0]);
        let calls = Cell::new(0);
        evaluate_row(
            p,
            SourcePixel::new(u32::MAX, u32::MAX),
            &[],
            &mut [],
            quote(p, 0).unwrap().into(),
            &|| {
                calls.set(calls.get() + 1);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(calls.get(), 2);
    }
}
#[test]
fn cancellation_discards_private_prefix_and_reuse_keeps_source_and_guards() {
    let source = [Rgba([64, 128, 192, 31]); 130];
    let before = source;
    for d in [NoiseDistribution::Uniform, NoiseDistribution::Gaussian] {
        for mono in [false, true] {
            let p = policy(d, mono, 10);
            let origin = SourcePixel::new(63, 127);
            let q = quote(p, 130).unwrap();
            let mut expected = [Rgba([0; 4]); 130];
            evaluate_row(p, origin, &source, &mut expected, q.into(), &|| Ok(())).unwrap();
            for (cancel, prefix) in [(1, 0), (2, 0), (3, 64), (4, 128), (5, 130)] {
                let mut guarded = [Rgba([7; 4]); 132];
                let calls = Cell::new(0);
                let err = evaluate_row(p, origin, &source, &mut guarded[1..131], q.into(), &|| {
                    calls.set(calls.get() + 1);
                    if calls.get() == cancel {
                        Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "noise test cancellation",
                        ))
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err();
                assert_eq!(err.code, ErrorCode::OperationCancelled);
                assert_eq!(calls.get(), cancel);
                assert_eq!(&guarded[1..1 + prefix], &expected[..prefix]);
                assert!(guarded[1 + prefix..131].iter().all(|p| *p == Rgba([7; 4])));
                assert_eq!(guarded[0], Rgba([7; 4]));
                assert_eq!(guarded[131], Rgba([7; 4]));
                assert_eq!(source, before);
                // The failed private prefix is discarded, then the caller can reuse storage.
                guarded[1..131].fill(Rgba([7; 4]));
                assert_eq!(
                    evaluate_row(p, origin, &source, &mut guarded[1..131], q.into(), &|| Ok(
                        ()
                    ))
                    .unwrap()
                    .units,
                    q.work
                );
                assert_eq!(&guarded[1..131], &expected);
            }
        }
    }
}
