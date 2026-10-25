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
    let mut transparent = samples;
    transparent[3] = 256;
    assert_eq!(
        jxl::encode_rgba16(2, 3, &transparent).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn input_output_geometry_and_single_dimension_writer_limits_are_explicit() {
    for (w, h) in [(1, 1), (1, 5), (5, 1)] {
        assert_eq!(
            jxl::encode_rgba8(w, h, &rgba(w, h)).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
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
