use super::*;
fn request() -> Request {
    Request::capture(
        Arc::new(vec![1, 2, 3]),
        Owner {
            session: Uuid::new_v4(),
            state: Uuid::new_v4(),
            epoch: 17,
            frontend: 3,
        },
        16,
    )
    .unwrap()
}
fn response(r: &Request) -> Vec<u8> {
    success(
        r.header,
        Success {
            width: 2,
            height: 1,
            pixels: Pixels::NativeGray8 {
                gray: &[0, 255],
                alpha: Some(&[17, 243]),
                encoding: NativeEncoding {
                    primaries: 2,
                    transfer: 2,
                    matrix: 2,
                    full_range: true,
                },
            },
        },
        4096,
    )
    .unwrap()
}
#[test]
fn exact_protocol_round_trip_preserves_native_planes_and_capture() {
    let r = request();
    assert_eq!(r.header.encode().len(), REQUEST_HEADER);
    assert_eq!(Header::decode(&r.header.encode()).unwrap(), r.header);
    assert!(r.header.verify_source(&[1, 2, 3]).is_ok());
    assert!(r.header.verify_source(&[1, 2, 4]).is_err());
    assert!(r.reservation_bytes() > HEAP_BYTES + r.header.length);
    let out = response(&r);
    assert_eq!(
        out.len(),
        RESPONSE_HEADER + env!("CARGO_PKG_VERSION").len() + 4
    );
    let frame = decode(&out, &r, r.header.owner).unwrap();
    let Pixels::NativeGray8 {
        gray,
        alpha,
        encoding,
    } = frame.pixels
    else {
        panic!()
    };
    assert_eq!(gray, [0, 255]);
    assert_eq!(alpha.unwrap(), [17, 243]);
    assert_eq!(frame.heap_peak, 4096);
    assert_eq!(encoding.matrix, 2);
}
#[test]
fn foreign_source_job_document_frontend_and_epoch_are_refused() {
    let r = request();
    let out = response(&r);
    for owner in [
        Owner {
            session: Uuid::new_v4(),
            ..r.header.owner
        },
        Owner {
            state: Uuid::new_v4(),
            ..r.header.owner
        },
        Owner {
            epoch: 18,
            ..r.header.owner
        },
        Owner {
            frontend: 1,
            ..r.header.owner
        },
    ] {
        assert_eq!(
            decode(&out, &r, owner).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
    }
    let different = request();
    assert_eq!(
        decode(&out, &different, different.header.owner)
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    for offset in [47, 88, 104] {
        let mut tampered = out.clone();
        tampered[offset] ^= 1;
        assert_eq!(
            decode(&tampered, &r, r.header.owner).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
    }
}
#[test]
fn malformed_lengths_models_versions_and_unaccounted_heap_fail_closed() {
    let r = request();
    let out = response(&r);
    for len in 0..out.len() {
        assert!(decode(&out[..len], &r, r.header.owner).is_err());
    }
    let mut extra = out.clone();
    extra.push(0);
    assert!(decode(&extra, &r, r.header.owner).is_err());
    for (offset, value) in [
        (11, 2),
        (20, 2),
        (27, 0),
        (28, 255),
        (38, 255),
        (39, 255),
        (RESPONSE_HEADER, b'x'),
    ] {
        let mut malformed = out.clone();
        malformed[offset] = value;
        assert!(
            decode(&malformed, &r, r.header.owner).is_err(),
            "offset {offset}"
        );
    }
    for len in 0..REQUEST_HEADER {
        assert!(Header::decode(&r.header.encode()[..len]).is_err());
    }
}
#[test]
fn framed_failures_preserve_error_codes_and_never_carry_pixels() {
    let r = request();
    for code in [
        ErrorCode::CapabilityUnsupported,
        ErrorCode::CodecFailed,
        ErrorCode::ResourceLimit,
        ErrorCode::OperationCancelled,
    ] {
        let out = error(r.header, &Error::new(code, "private decoder failure"), 9).unwrap();
        assert_eq!(decode(&out, &r, r.header.owner).unwrap_err().code, code);
    }
    let mut illegal = response(&r);
    illegal[10] = 2;
    assert!(decode(&illegal, &r, r.header.owner).is_err());
}

#[test]
fn explicit_codec_binds_sample_model_and_prevents_cross_codec_confusion() {
    let mut r = request();
    r.header.codec = Codec::JxlRgba8;
    let bytes = success(
        r.header,
        Success {
            width: 2,
            height: 1,
            pixels: Pixels::SrgbRgba8(&[10, 20, 30, 0, 40, 50, 60, 137]),
        },
        17,
    )
    .unwrap();
    let frame = decode(&bytes, &r, r.header.owner).unwrap();
    assert_eq!(
        frame.pixels,
        Pixels::SrgbRgba8(&[10, 20, 30, 0, 40, 50, 60, 137])
    );
    let mut foreign = r.clone();
    foreign.header.codec = Codec::HeifGray8;
    assert!(decode(&bytes, &foreign, foreign.header.owner).is_err());
    assert!(
        success(
            foreign.header,
            Success {
                width: 2,
                height: 1,
                pixels: Pixels::SrgbRgba8(&[0; 8])
            },
            0
        )
        .is_err()
    );
    for offset in [20, 22, 24, 26, 27, 32] {
        let mut changed = bytes.clone();
        changed[offset] ^= 1;
        assert!(
            decode(&changed, &r, r.header.owner).is_err(),
            "profile offset{offset}"
        );
    }
    let failure = error(
        r.header,
        &Error::new(ErrorCode::CapabilityUnsupported, "ICC conversion disabled"),
        17,
    )
    .unwrap();
    assert_eq!(
        decode(&failure, &r, r.header.owner).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn largest_rgba_payload_and_version_fit_the_reserved_output_allowance() {
    let mut r = request();
    r.header.codec = Codec::JxlRgba8;
    r.header.pixels = PIXELS_MAX as u32;
    let rgba = vec![137; PIXELS_MAX as usize * 4];
    let bytes = success(
        r.header,
        Success {
            width: 2048,
            height: 2048,
            pixels: Pixels::SrgbRgba8(&rgba),
        },
        16 << 20,
    )
    .unwrap();
    assert!(bytes.len() <= OUTPUT_BYTES_MAX);
    let frame = decode(&bytes, &r, r.header.owner).unwrap();
    assert_eq!(frame.pixels, Pixels::SrgbRgba8(&rgba));
    assert!(r.reservation_bytes() >= HEAP_BYTES + bytes.len() as u64 + r.source.len() as u64);
}

#[test]
fn header_only_validation_matches_request_and_rejects_all_identity_and_length_changes() {
    let request = request();
    let output = response(&request);
    let original = decode(&output, &request, request.header.owner).unwrap();
    let header = decode_with_header(&output, &request.header, request.header.owner).unwrap();
    assert_eq!(
        (
            original.width,
            original.height,
            original.pixels,
            original.heap_peak
        ),
        (header.width, header.height, header.pixels, header.heap_peak)
    );
    for offset in [47, 88, 104, RESPONSE_HEADER, 36, 28] {
        let mut changed = output.clone();
        changed[offset] ^= 1;
        let expected = decode(&changed, &request, request.header.owner).unwrap_err();
        assert_eq!(
            decode_with_header(&changed, &request.header, request.header.owner)
                .unwrap_err()
                .code,
            expected.code
        );
    }
    let mut short = output.clone();
    short.pop();
    assert!(decode_with_header(&short, &request.header, request.header.owner).is_err());
    let mut foreign = request.header.owner;
    foreign.epoch += 1;
    assert!(decode_with_header(&output, &request.header, foreign).is_err());
    let mut wrong = request.header;
    wrong.nonce = Uuid::new_v4();
    assert!(decode_with_header(&output, &wrong, request.header.owner).is_err());
    wrong = request.header;
    wrong.sha256[0] ^= 1;
    assert!(decode_with_header(&output, &wrong, request.header.owner).is_err());
}

#[test]
fn heif_color_code4_is_native_only_and_requires_the_explicit_rgba_model() {
    let mut r = request();
    r.header.codec = Codec::HeifSrgb8;
    let captured = Request::capture_for(Codec::HeifSrgb8, r.source.clone(), r.header.owner, 16);
    if cfg!(target_arch = "wasm32") {
        assert!(captured.is_err());
        assert!(Header::decode(&r.header.encode()).is_err());
        return;
    }
    assert!(captured.is_ok());
    assert_eq!(Header::decode(&r.header.encode()).unwrap(), r.header);
    let rgba = [1, 2, 3, 0, 4, 5, 6, 255];
    let out = success(
        r.header,
        Success {
            width: 2,
            height: 1,
            pixels: Pixels::SrgbRgba8(&rgba),
        },
        4096,
    )
    .unwrap();
    assert_eq!(
        decode(&out, &r, r.header.owner).unwrap().pixels,
        Pixels::SrgbRgba8(&rgba)
    );
    let mut invalid = out.clone();
    invalid[22] = 2; // Primaries1 becomes unspecified2.
    assert!(decode(&invalid, &r, r.header.owner).is_err());
    invalid = out;
    invalid.pop();
    assert!(decode(&invalid, &r, r.header.owner).is_err());
    assert!(
        success(
            r.header,
            Success {
                width: 2,
                height: 1,
                pixels: Pixels::NativeGray8 {
                    gray: &[0, 255],
                    alpha: None,
                    encoding: NativeEncoding {
                        primaries: 1,
                        transfer: 13,
                        matrix: 0,
                        full_range: true
                    }
                }
            },
            4096
        )
        .is_err()
    );
}
