//! Independent libjxl fixture exercises the isolated profile and conversion.
//! Tests use trusted owned bytes; production acquisition belongs in the helper.
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::jxl::{self, Samples};
use sha2::{Digest, Sha256};
use std::cell::Cell;

const SOURCE: &[u8] = include_bytes!("jxl-fixtures/rgba16.jxl");
const RAW: [u16; 16] = [
    0, 1, 65535, 256, 257, 32767, 32768, 0, 65535, 1024, 2048, 32768, 4096, 8192, 16384, 65535,
];

#[test]
fn independent_exact16_samples_convert_with_bound_digests_and_mandatory_loss() {
    assert_eq!(
        paintcraft_document::hex(&Sha256::digest(SOURCE)),
        "5d9b5850856d421d1ebfe1a69f7f6a5005e798acc191bd71b095a5e2406dc57d"
    );
    let decoded = jxl::decode_worker_rgba16_checked(SOURCE, 4, &|| Ok(())).unwrap();
    assert_eq!((decoded.width, decoded.height), (2, 2));
    assert!(decoded.losses.is_empty());
    assert_eq!(
        jxl::decode_worker_integer_checked(SOURCE, 4, &|| Ok(())).unwrap(),
        decoded
    );
    assert_eq!(decoded.samples, Samples::Rgba16(RAW.to_vec()));
    let Samples::Rgba16(samples) = decoded.samples else {
        panic!()
    };
    let sha = Sha256::digest(SOURCE).into();
    let converted = jxl::convert_rgba16_checked(2, 2, &samples, sha, 16, &|| Ok(())).unwrap();
    // Independently evaluated nearest /257, including nonzero RGB at alpha0.
    let expected = [
        0, 0, 255, 1, 1, 127, 128, 0, 255, 4, 8, 128, 16, 32, 64, 255,
    ];
    assert_eq!(converted.rgba, expected);
    assert_eq!(converted.assessment.source_sha256, sha);
    assert_eq!(
        converted.assessment.converted_sha256,
        <[u8; 32]>::from(Sha256::digest(expected))
    );
    assert_eq!(converted.assessment.changed_samples(), 10);
    assert_eq!(converted.assessment.changed_rgb_components, 8);
    assert_eq!(converted.assessment.changed_alpha, 2);
    assert_eq!(converted.assessment.policy(), jxl::DEPTH16_POLICY);
    assert_eq!(converted.assessment.loss(), "depth-16-reduced-to-8");
}

#[test]
fn every_integer_uses_nearest_representable_value_including_rounding_boundaries() {
    let samples: Vec<u16> = (0..=u16::MAX).collect();
    let result =
        jxl::convert_rgba16_checked(16384, 1, &samples, [7; 32], 65536, &|| Ok(())).unwrap();
    for (&raw, &actual) in samples.iter().zip(&result.rgba) {
        let expected = (f64::from(raw) / 257.0).round() as u8;
        assert_eq!(actual, expected, "sample {raw}");
    }
    assert_eq!(result.assessment.changed_samples(), 65536 - 256);
    let exact =
        jxl::convert_rgba16_checked(1, 1, &[0, 257, 32896, 65535], [0; 32], 4, &|| Ok(())).unwrap();
    assert_eq!(exact.assessment.changed_samples(), 0);
    assert_eq!(exact.assessment.loss(), jxl::DEPTH16_LOSS);
}

#[test]
fn wrong_worker_precision_and_unsupported_profiles_stay_refused() {
    assert_eq!(
        jxl::decode_worker_rgba8_checked(SOURCE, 4, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    for source in [
        include_bytes!("jxl-fixtures/rgba8.jxl").as_slice(),
        include_bytes!("jxl-fixtures/icc.jxl"),
        include_bytes!("jxl-fixtures/linear.jxl"),
        include_bytes!("jxl-fixtures/animation.jxl"),
    ] {
        assert_eq!(
            jxl::decode_worker_rgba16_checked(source, jxl::PIXELS_MAX, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        if source != include_bytes!("jxl-fixtures/rgba8.jxl") {
            assert_eq!(
                jxl::decode_worker_integer_checked(source, jxl::PIXELS_MAX, &|| Ok(()))
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
        }
    }
    let eight = include_bytes!("jxl-fixtures/rgba8.jxl");
    assert_eq!(
        jxl::decode_worker_integer_checked(eight, 6, &|| Ok(())).unwrap(),
        jxl::decode_worker_rgba8_checked(eight, 6, &|| Ok(())).unwrap()
    );
    let mut metadata = SOURCE.to_vec();
    metadata.extend(9u32.to_be_bytes());
    metadata.extend(b"xml x");
    assert_eq!(
        jxl::decode_worker_rgba16_checked(&metadata, 4, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        jxl::decode_worker_integer_checked(&metadata, 4, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}

#[test]
fn admission_and_cancellation_never_expose_partial_output() {
    assert_eq!(
        jxl::decode_worker_rgba16_checked(SOURCE, 3, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        jxl::convert_rgba16_checked(2, 2, &RAW, [0; 32], 15, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        jxl::convert_rgba16_checked(2, 2, &RAW[..15], [0; 32], 16, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CodecFailed
    );
    let source = RAW;
    for decoding in [false, true] {
        let count = Cell::new(0);
        let check = || {
            count.set(count.get() + 1);
            Ok(())
        };
        if decoding {
            jxl::decode_worker_rgba16_checked(SOURCE, 4, &check).unwrap();
        } else {
            jxl::convert_rgba16_checked(2, 2, &RAW, [0; 32], 16, &check).unwrap();
        }
        for stop in 1..=count.get() {
            let calls = Cell::new(0);
            let check = || {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
                } else {
                    Ok(())
                }
            };
            let error = if decoding {
                jxl::decode_worker_rgba16_checked(SOURCE, 4, &check).unwrap_err()
            } else {
                jxl::convert_rgba16_checked(2, 2, &RAW, [0; 32], 16, &check).unwrap_err()
            };
            assert_eq!(error.code, ErrorCode::OperationCancelled);
            assert_eq!(RAW, source);
        }
    }
    // Fail after the first full conversion/digest chunk, while private output
    // already contains data. The public result must still contain no prefix.
    let large = RAW.repeat(1025);
    let before = large.clone();
    let calls = Cell::new(0);
    let error = jxl::convert_rgba16_checked(4100, 1, &large, [3; 32], large.len(), &|| {
        calls.set(calls.get() + 1);
        if calls.get() == 3 {
            Err(Error::new(ErrorCode::OperationCancelled, "mid-output"))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(large, before);
}
