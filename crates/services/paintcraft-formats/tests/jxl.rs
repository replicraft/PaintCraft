use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::jxl::{self, Samples};
use std::cell::Cell;

fn rgba(width: u32, height: u32) -> Vec<u8> {
    (0..width * height)
        .flat_map(|i| {
            [
                (i * 17) as u8,
                (i * 31) as u8,
                (i * 73) as u8,
                [0, 1, 127, 255][i as usize % 4],
            ]
        })
        .collect()
}
#[test]
fn lossless_rgba8_preserves_hidden_rgb_and_non_power_of_two_edges() {
    for (w, h) in [(2, 2), (3, 7), (255, 257)] {
        let source = rgba(w, h);
        let frozen = source.clone();
        let encoded = jxl::encode_rgba8(w, h, &source).unwrap();
        assert!(jxl::sniff(&encoded));
        let decoded = jxl::decode_with_pixel_limit(&encoded, u64::from(w) * u64::from(h)).unwrap();
        assert_eq!((decoded.width, decoded.height), (w, h));
        assert_eq!(decoded.samples, Samples::Rgba8(source.clone()));
        assert!(decoded.losses.is_empty());
        assert_eq!(source, frozen);
    }
}
#[test]
fn lossless_rgba16_preserves_native_endian_sample_values_without_downcast() {
    let samples: Vec<u16> = (0..2 * 3)
        .flat_map(|i| [i * 257, i * 5001, 65535 - i * 997, 65535])
        .collect();
    let encoded = jxl::encode_rgba16(2, 3, &samples).unwrap();
    let info = jxl::inspect_checked(&encoded, 6, &|| Ok(())).unwrap();
    assert_eq!(info.bits_per_sample, 16);
    assert!(!info.floating_point);
    let decoded = jxl::decode_with_pixel_limit(&encoded, 6).unwrap();
    assert_eq!(decoded.samples, Samples::Rgba16(samples.clone()));
    assert_eq!(
        encoded,
        include_bytes!("jxl-fixtures/opaque16-zune-0.5.2.jxl")
    );
}
#[test]
fn input_output_geometry_and_resource_limits_are_explicit() {
    for (w, h) in [(0, 0), (0, 1), (1, 0)] {
        assert_eq!(
            jxl::encode_rgba8(w, h, &[]).unwrap_err().code,
            ErrorCode::CodecFailed
        );
        assert_eq!(
            jxl::encode_rgba16(w, h, &[]).unwrap_err().code,
            ErrorCode::CodecFailed
        );
    }
    assert_eq!(
        jxl::encode_rgba8(2, 2, &[0; 15]).unwrap_err().code,
        ErrorCode::CodecFailed
    );
    assert_eq!(
        jxl::encode_rgba8(65536, 65536, &[]).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let encoded = jxl::encode_rgba8(3, 7, &rgba(3, 7)).unwrap();
    assert_eq!(
        jxl::decode_with_pixel_limit(&encoded, 20).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(!jxl::sniff(b"not jpeg xl"));
    assert!(jxl::decode_with_pixel_limit(b"not jpeg xl", 10).is_err());
}
#[test]
fn cancellation_is_not_reclassified_or_retried() {
    let error = || Error::new(ErrorCode::OperationCancelled, "test cancellation");
    let source = rgba(17, 19);
    let encoded = jxl::encode_rgba8(17, 19, &source).unwrap();
    for checkpoint in [0, 2, 5, 10] {
        let calls = Cell::new(0);
        let check = || {
            let n = calls.get();
            calls.set(n + 1);
            if n >= checkpoint {
                Err(error())
            } else {
                Ok(())
            }
        };
        let result = jxl::decode_checked(&encoded, 323, &check);
        assert_eq!(
            result.unwrap_err().code,
            ErrorCode::OperationCancelled,
            "checkpoint {checkpoint}"
        );
    }
    assert_eq!(
        jxl::encode_rgba8_checked(17, 19, &source, &|| Err(error()))
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
}
#[test]
fn every_truncation_of_small_codestream_is_rejected() {
    let encoded = jxl::encode_rgba8(2, 2, &rgba(2, 2)).unwrap();
    for end in 0..encoded.len() {
        assert!(
            jxl::decode_with_pixel_limit(&encoded[..end], 4).is_err(),
            "accepted truncated length {end}/{}",
            encoded.len()
        );
    }
}

#[test]
fn single_axis_and_nonopaque_rgba16_preserve_all_samples_and_dimensions() {
    for (width, height) in [(1, 1), (1, 6), (6, 1), (3, 2), (255, 257)] {
        let rgba16: Vec<u16> = (0..width * height)
            .flat_map(|i| {
                [
                    ((i * 997 + 257) % 65536) as u16,
                    ((i * 5001 + 32768) % 65536) as u16,
                    (65535 - i % 65536) as u16,
                    [0, 1, 256, 32768, 65534, 65535][i as usize % 6],
                ]
            })
            .collect();
        let rgba8: Vec<_> = rgba16.iter().map(|v| (v >> 8) as u8).collect();
        let frozen = rgba16.clone();
        let frozen8 = rgba8.clone();
        for sixteen in [false, true] {
            let encoded = if sixteen {
                jxl::encode_rgba16(width, height, &rgba16)
            } else {
                jxl::encode_rgba8(width, height, &rgba8)
            }
            .unwrap();
            let info =
                jxl::inspect_checked(&encoded, u64::from(width) * u64::from(height), &|| Ok(()))
                    .unwrap();
            assert_eq!((info.width, info.height), (width, height));
            assert_eq!(info.bits_per_sample, if sixteen { 16 } else { 8 });
            assert!(!info.associated_alpha);
            let decoded =
                jxl::decode_with_pixel_limit(&encoded, u64::from(width) * u64::from(height))
                    .unwrap();
            assert_eq!(
                decoded.samples,
                if sixteen {
                    Samples::Rgba16(rgba16.clone())
                } else {
                    Samples::Rgba8(rgba8.clone())
                }
            );
            assert!(decoded.losses.is_empty());
        }
        assert_eq!(rgba16, frozen);
        assert_eq!(rgba8, frozen8);
    }
}
#[test]
fn nonopaque_rgba16_cancellation_and_malformed_inputs_return_no_output() {
    let source = [257, 32768, 65535, 1];
    for checkpoint in [0, 1, 2] {
        let calls = Cell::new(0);
        let check = || {
            let n = calls.get();
            calls.set(n + 1);
            if n >= checkpoint {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "test cancellation",
                ))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            jxl::encode_rgba16_checked(1, 1, &source, &check)
                .unwrap_err()
                .code,
            ErrorCode::OperationCancelled
        );
        assert_eq!(source, [257, 32768, 65535, 1]);
    }
    assert_eq!(
        jxl::encode_rgba16(1, 1, &source[..3]).unwrap_err().code,
        ErrorCode::CodecFailed
    );
    assert_eq!(
        jxl::encode_rgba16(65536, 65536, &[]).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}
