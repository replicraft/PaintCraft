//! Synthetic vectors authored from Microsoft's DDS and BC structure definitions.
use super::*;
use std::cell::Cell;
fn put(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn legacy(width: u32, height: u32, bgra: bool, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 128];
    bytes[..4].copy_from_slice(b"DDS ");
    for (offset, value) in [
        (4, 124),
        (8, 0x100f),
        (12, height),
        (16, width),
        (20, width * 4),
        (28, 1),
        (76, 32),
        (80, 0x41),
        (88, 32),
        (92, if bgra { 0xff0000 } else { 0xff }),
        (96, 0xff00),
        (100, if bgra { 0xff } else { 0xff0000 }),
        (104, 0xff000000),
        (108, 0x1000),
    ] {
        put(&mut bytes, offset, value);
    }
    bytes.extend_from_slice(payload);
    bytes
}
fn dx10(width: u32, height: u32, format: u32, payload: &[u8]) -> Vec<u8> {
    let mut bytes = legacy(width, height, false, &[]);
    bytes[80..108].fill(0);
    put(&mut bytes, 80, 4);
    bytes[84..88].copy_from_slice(b"DX10");
    for value in [format, 3, 0, 1, 1] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(payload);
    bytes
}
fn bc(fourcc: &[u8; 4], payload: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut bytes = legacy(width, height, false, &[]);
    put(&mut bytes, 8, 0x81007);
    put(&mut bytes, 20, payload.len() as u32);
    bytes[80..108].fill(0);
    put(&mut bytes, 80, 4);
    bytes[84..88].copy_from_slice(fourcc);
    bytes.extend_from_slice(payload);
    bytes
}
fn decoded(bytes: &[u8]) -> Decoded {
    decode_with_pixel_limit(bytes, PIXELS_MAX).unwrap()
}
#[test]
fn manually_authored_rgba_and_bgra_preserve_straight_and_hidden_rgb() {
    let rgba = [
        17, 29, 91, 0, 33, 47, 255, 1, 1, 128, 192, 127, 255, 0, 91, 255,
    ];
    for bgra in [false, true] {
        let input: Vec<u8> = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| if bgra { [p[2], p[1], p[0], p[3]] } else { *p })
            .collect();
        let bytes = legacy(2, 2, bgra, &input);
        let frozen = bytes.clone();
        let result = decoded(&bytes);
        assert_eq!(result.rgba, rgba);
        assert_eq!(result.info.width, 2);
        assert_eq!(result.info.height, 2);
        assert_eq!(result.info.transfer, Transfer::Unspecified);
        assert_eq!(result.info.alpha, SourceAlpha::Unspecified);
        assert_eq!(bytes, frozen);
    }
}
#[test]
fn dx10_transfer_and_alpha_are_not_inferred_from_the_codec_format_enum() {
    let pixels = [1, 2, 3, 0, 4, 5, 6, 127];
    for (format, transfer) in [(28, Transfer::Linear), (29, Transfer::Srgb)] {
        let result = decoded(&dx10(2, 1, format, &pixels));
        assert_eq!(result.rgba, pixels);
        assert_eq!(result.info.transfer, transfer);
        assert_eq!(result.info.alpha, SourceAlpha::Straight);
    }
    let mut opaque = dx10(2, 1, 29, &pixels);
    put(&mut opaque, 144, 3);
    let result = decoded(&opaque);
    assert_eq!(result.rgba, [1, 2, 3, 255, 4, 5, 6, 255]);
    assert_eq!(result.info.alpha, SourceAlpha::Opaque);
}
#[test]
fn manual_bc1_endpoints_selectors_and_transparency_match_expected_pixels() {
    // RGB565 endpoints red F800 and green07E0, selectors0/1/2/3 per row.
    let block = [0, 0xf8, 0xe0, 7, 0xe4, 0xe4, 0xe4, 0xe4];
    let bytes = bc(b"DXT1", &block, 4, 4);
    let result = decoded(&bytes);
    let row = [
        255, 0, 0, 255, 0, 255, 0, 255, 170, 85, 0, 255, 85, 170, 0, 255,
    ];
    assert_eq!(result.rgba, row.repeat(4));
    assert_eq!(result.info.encoding, Encoding::Bc1);
    let transparent = bc(b"DXT1", &[0, 0, 255, 255, 255, 255, 255, 255], 4, 4);
    assert_eq!(decoded(&transparent).rgba, [0u8; 64]);
    // Cropped edge still consumes one complete physical 4x4 block.
    assert_eq!(
        decoded(&bc(b"DXT1", &block, 3, 2)).rgba,
        row[..12].repeat(2)
    );
}
#[test]
fn manual_bc2_and_bc3_preserve_alpha_without_unpremultiplication() {
    let mut block = vec![0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe];
    block.extend_from_slice(&[0, 0xf8, 0xe0, 7, 0, 0, 0, 0]);
    let result = decoded(&bc(b"DXT3", &block, 4, 4));
    let expected: Vec<u8> = (0..16).flat_map(|i| [255, 0, 0, i * 17]).collect();
    assert_eq!(result.rgba, expected);
    assert_eq!(result.info.encoding, Encoding::Bc2);
    let mut block = vec![255, 0, 0, 0, 0, 0, 0, 0];
    block.extend_from_slice(&[0, 0xf8, 0xe0, 7, 0, 0, 0, 0]);
    let result = decoded(&bc(b"DXT5", &block, 4, 4));
    assert_eq!(result.rgba, [255, 0, 0, 255].repeat(16));
    assert_eq!(result.info.encoding, Encoding::Bc3);
}
#[test]
fn texture_capabilities_and_unqualified_precision_are_refused_before_pixels() {
    let base = dx10(2, 2, 28, &[0; 16]);
    for (offset, value) in [
        (28, 2),
        (132, 4),
        (132, 2),
        (136, 4),
        (140, 2),
        (144, 2),
        (144, 4),
        (128, 2),
        (128, 27),
    ] {
        let mut bytes = base.clone();
        put(&mut bytes, offset, value);
        assert_eq!(
            decode_with_pixel_limit(&bytes, 4).unwrap_err().code,
            ErrorCode::CapabilityUnsupported,
            "offset {offset} value {value}"
        );
    }
    let mut cube = legacy(2, 2, false, &[0; 16]);
    put(&mut cube, 112, 0x600);
    assert_eq!(
        decode_with_pixel_limit(&cube, 4).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    for fourcc in [b"DXT2", b"DXT4"] {
        let bytes = bc(fourcc, &[0; 16], 4, 4);
        assert_eq!(
            decode_with_pixel_limit(&bytes, 16).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
#[test]
fn exact_full_body_consumption_and_strict_header_sizes_are_required() {
    let bytes = legacy(2, 2, false, &[0; 16]);
    for end in 0..bytes.len() {
        assert!(
            decode_with_pixel_limit(&bytes[..end], 4).is_err(),
            "accepted prefix {end}"
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode_with_pixel_limit(&trailing, 4).is_err());
    for (offset, value) in [(4, 123), (76, 31)] {
        let mut bad = bytes.clone();
        put(&mut bad, offset, value);
        assert_eq!(
            decode_with_pixel_limit(&bad, 4).unwrap_err().code,
            ErrorCode::CodecFailed
        );
    }
    let mut padded = bytes;
    put(&mut padded, 20, 12);
    assert_eq!(
        decode_with_pixel_limit(&padded, 4).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn pixel_limits_precede_output_and_cancellation_is_never_retried() {
    let bytes = legacy(2, 2, false, &[0; 16]);
    assert_eq!(
        decode_with_pixel_limit(&bytes, 3).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let mut huge = bytes.clone();
    put(&mut huge, 16, u32::MAX);
    assert_eq!(
        decode_with_pixel_limit(&huge, PIXELS_MAX).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let calls = Cell::new(0usize);
    decode_checked(&bytes, 4, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    let total = calls.get();
    assert!(total > 2);
    for after in [0, 1, 2, total / 2, total - 2, total - 1] {
        let calls = Cell::new(0usize);
        let check = || {
            let n = calls.get();
            calls.set(n + 1);
            if n >= after {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "test cancellation",
                ))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            decode_checked(&bytes, 4, &check).unwrap_err().code,
            ErrorCode::OperationCancelled,
            "checkpoint {after}"
        );
    }
}
#[test]
fn exact_uncompressed_writer_has_independent_header_and_pixel_assertions() {
    let source = [
        17, 29, 91, 0, 33, 47, 255, 1, 1, 128, 192, 127, 255, 0, 91, 255,
    ];
    let frozen = source;
    for transfer in [Transfer::Linear, Transfer::Srgb] {
        let bytes = encode_rgba8(2, 2, &source, transfer).unwrap();
        let int = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        assert_eq!(bytes.len(), 148 + source.len());
        assert_eq!(&bytes[..4], b"DDS ");
        assert_eq!(int(4), 124);
        assert_eq!(int(12), 2);
        assert_eq!(int(16), 2);
        assert_eq!(int(76), 32);
        assert_eq!(&bytes[84..88], b"DX10");
        assert_eq!(int(128), if transfer == Transfer::Srgb { 29 } else { 28 });
        assert_eq!(int(132), 3);
        assert_eq!(int(136), 0);
        assert_eq!(int(140), 1);
        assert_eq!(int(144), 1);
        assert_eq!(&bytes[148..], source);
        assert_eq!(source, frozen);
        let read = decoded(&bytes);
        assert_eq!(read.rgba, source);
        assert_eq!(read.info.transfer, transfer);
        assert_eq!(read.info.alpha, SourceAlpha::Straight);
    }
    assert_eq!(
        &encode_rgba8(1, 1, &[13, 27, 79, 0], Transfer::Srgb).unwrap()[148..],
        [13, 27, 79, 0]
    );
}
#[test]
fn writer_admission_and_cancellation_never_publish_partial_bytes() {
    assert_eq!(
        encode_rgba8(1, 1, &[0; 4], Transfer::Unspecified)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        encode_rgba8(1, 1, &[0; 3], Transfer::Srgb)
            .unwrap_err()
            .code,
        ErrorCode::CodecFailed
    );
    assert_eq!(
        encode_rgba8(u32::MAX, u32::MAX, &[], Transfer::Linear)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        encode_rgba8(0, 1, &[], Transfer::Linear).unwrap_err().code,
        ErrorCode::CodecFailed
    );
    let calls = Cell::new(0usize);
    encode_rgba8_checked(2, 2, &[0; 16], Transfer::Srgb, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    let total = calls.get();
    for after in [0, 1, total / 2, total - 1] {
        let calls = Cell::new(0usize);
        let check = || {
            let n = calls.get();
            calls.set(n + 1);
            if n >= after {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel output"))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            encode_rgba8_checked(2, 2, &[0; 16], Transfer::Srgb, &check)
                .unwrap_err()
                .code,
            ErrorCode::OperationCancelled
        );
    }
    for (offset, value) in [(136, 2), (144, 0x101)] {
        let mut bytes = dx10(2, 2, 28, &[0; 16]);
        put(&mut bytes, offset, value);
        assert_eq!(
            decode_with_pixel_limit(&bytes, 4).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
