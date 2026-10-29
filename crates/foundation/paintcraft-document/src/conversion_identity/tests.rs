use super::*;
fn context() -> RasterConversionContext<'static> {
    RasterConversionContext {
        source_sha256: [7; 32],
        profile: "dcx-rgba8/v1",
        selection: RasterSelection::Page(0),
        assessment_sha256: None,
    }
}
fn key(context: RasterConversionContext<'_>, rgba: &[u8]) -> RasterConversionKey {
    RasterConversionKey::checked(context, 1, 1, rgba, 1, &|| Ok(())).unwrap()
}
#[test]
fn semantic_choices_samples_and_geometry_are_distinct_while_repeats_and_adequate_caps_are_stable() {
    let rgba = [13, 71, 201, 0];
    let baseline = key(context(), &rgba);
    // Independently serialized with Python hashlib/struct, little-endian framing.
    assert_eq!(
        baseline.hex(),
        "d2354566ec68983caa6f0c3beb30fbbc88fd0fb144c1e833943a491ef51de2c0"
    );
    assert_eq!(baseline, key(context(), &rgba));
    assert_eq!(
        baseline,
        RasterConversionKey::checked(context(), 1, 1, &rgba, 4096, &|| Ok(())).unwrap()
    );
    for changed in [
        RasterConversionContext {
            selection: RasterSelection::Page(1),
            ..context()
        },
        RasterConversionContext {
            selection: RasterSelection::Image,
            ..context()
        },
        RasterConversionContext {
            profile: "dcx-rgba8/v2",
            ..context()
        },
        RasterConversionContext {
            source_sha256: [8; 32],
            ..context()
        },
        RasterConversionContext {
            assessment_sha256: Some([0; 32]),
            ..context()
        },
        RasterConversionContext {
            assessment_sha256: Some([9; 32]),
            ..context()
        },
    ] {
        assert_ne!(baseline, key(changed, &rgba));
    }
    // Hidden RGB and straight alpha independently contribute to immutable state.
    assert_ne!(baseline, key(context(), &[14, 71, 201, 0]));
    assert_ne!(baseline, key(context(), &[13, 71, 201, 1]));
    let two = [rgba, rgba].concat();
    assert_ne!(
        RasterConversionKey::checked(context(), 2, 1, &two, 2, &|| Ok(())).unwrap(),
        RasterConversionKey::checked(context(), 1, 2, &two, 2, &|| Ok(())).unwrap()
    );
}
#[test]
fn malformed_bounds_and_cancellation_never_return_a_key() {
    for profile in ["", "with space", "nonascii-日", "nul\0profile"] {
        assert!(
            RasterConversionKey::checked(
                RasterConversionContext {
                    profile,
                    ..context()
                },
                1,
                1,
                &[0; 4],
                1,
                &|| Ok(())
            )
            .is_err()
        );
    }
    let long = "x".repeat(CONVERSION_PROFILE_BYTES_MAX + 1);
    assert!(
        RasterConversionKey::checked(
            RasterConversionContext {
                profile: &long,
                ..context()
            },
            1,
            1,
            &[0; 4],
            1,
            &|| Ok(())
        )
        .is_err()
    );
    for (width, height, bytes, limit) in [
        (0, 1, 4, 1),
        (CANVAS_SIDE_MAX + 1, 1, 4, 1),
        (1, 1, 3, 1),
        (1, 1, 5, 1),
        (1, 1, 4, 0),
    ] {
        assert!(
            RasterConversionKey::checked(context(), width, height, &vec![0; bytes], limit, &|| Ok(
                ()
            ))
            .is_err()
        );
    }
    use std::cell::Cell;
    let calls = Cell::new(0);
    let value =
        RasterConversionKey::checked(context(), 32768, 1, &vec![0; 32768 * 4], 32768, &|| {
            let count = calls.get();
            calls.set(count + 1);
            if count >= 2 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel identity hash",
                ))
            } else {
                Ok(())
            }
        });
    assert!(matches!(value,Err(ref error) if error.code==ErrorCode::OperationCancelled));
    assert!(calls.get() >= 3);
}
