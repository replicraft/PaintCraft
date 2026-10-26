//! Optional private encode operation; default helpers must keep refusing it.
use paintcraft_core::types::ErrorCode;
#[cfg(feature = "jxr-encode")]
use paintcraft_core::types::Result;
use paintcraft_formats::codec_worker::{self as decode, Owner, jxr_encode as wire};
use paintcraft_runtime::process::{self, Limits};
use std::{path::Path, process::Command, sync::Arc, time::Duration};
use uuid::Uuid;
#[cfg(feature = "jxr-encode")]
#[path = "native_jxr_encode/cases.rs"]
mod cases;
#[cfg(feature = "jxr-encode")]
#[path = "native_jxr_encode/lifecycle.rs"]
mod lifecycle;
fn owner() -> Owner {
    Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 9,
        frontend: 3,
    }
}
fn rgb(w: u32, h: u32) -> Vec<u8> {
    (0..h)
        .flat_map(|y| {
            (0..w).flat_map(move |x| (0..3).map(move |c| ((x * 47 + y * 13 + c * 71) % 256) as u8))
        })
        .collect()
}
fn make_request(cap: u64) -> wire::Request {
    wire::Request::capture(
        wire::Layout::new(4, 3, cap).unwrap(),
        Arc::new(rgb(4, 3)),
        owner(),
        &|| Ok(()),
    )
    .unwrap()
}
fn helper() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker"))
}
fn limits(request: &wire::Request) -> Limits {
    Limits {
        input_bytes: wire::REQUEST_HEADER + wire::RGB_BYTES_MAX as usize,
        output_bytes: request.header().layout().response_limit().unwrap() as usize,
        deadline: Duration::from_secs(10),
        resource_exit_code: Some(decode::RESOURCE_EXIT),
    }
}
#[cfg(feature = "jxr-encode")]
fn run(request: &wire::Request, check: &dyn Fn() -> Result<()>) -> Result<process::Output> {
    process::run_parent_lease(
        helper(),
        &["--jxr-encode".into()],
        request.segments(),
        limits(request),
        check,
    )
}
#[cfg(not(feature = "jxr-encode"))]
#[test]
fn default_v7_refuses_encoder_argv_and_new_frame_without_changing_its_record() {
    let output = Command::new(helper()).arg("--jxr-encode").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let request = make_request(65_536);
    match process::run_parent_lease(
        helper(),
        &[],
        vec![Arc::new(request.header().encode().to_vec())],
        limits(&request),
        &|| Ok(()),
    ) {
        Ok(output) => {
            assert!(!output.status.success());
            assert!(output.bytes.is_empty());
        }
        Err(error) => assert_eq!(error.code, ErrorCode::CodecFailed),
    }
    let bytes = std::fs::read(helper()).unwrap();
    let record = decode::native_float_build_record(env!("CARGO_PKG_VERSION")).unwrap();
    assert_eq!(
        bytes.windows(record.len()).filter(|p| *p == record).count(),
        1
    );
    let private = wire::candidate::BUILD_RECORD_MARKER;
    assert!(!bytes.windows(private.len()).any(|p| p == private));
}
#[cfg(feature = "jxr-encode")]
#[test]
fn private_record_exactly_declares_encoder_and_all_seven_existing_imports() {
    const {
        assert!(cfg!(all(
            feature = "float-codecs",
            feature = "avif-codec",
            feature = "avif-sdr-codec"
        )));
    }
    let bytes = std::fs::read(helper()).unwrap();
    let record = wire::candidate::build_record(env!("CARGO_PKG_VERSION")).unwrap();
    assert_eq!(record, wire::candidate::BUILD_RECORD);
    assert_eq!(
        bytes.windows(record.len()).filter(|p| *p == record).count(),
        1
    );
    assert_eq!(
        wire::candidate::IMPORT_VARIANTS,
        decode::QUALIFIED_FLOAT_IMPORT_VARIANTS
    );
    assert_eq!(wire::candidate::IMPORT_VARIANTS.len(), 7);
    assert_eq!(
        wire::candidate::EXPORT_VARIANTS,
        [wire::candidate::EXPORT_VARIANT]
    );
    for marker in [
        decode::BUILD_RECORD_MARKER,
        decode::AVIF_BUILD_RECORD_MARKER,
        decode::AVIF_SDR_BUILD_RECORD_MARKER,
        decode::FLOAT_BUILD_RECORD_MARKER,
    ] {
        assert!(!bytes.windows(marker.len()).any(|p| p == marker));
    }
}
#[cfg(feature = "jxr-encode")]
#[test]
fn fixed_lossless_output_roundtrips_protocol_and_cap_refuses_without_partial_file() {
    let request = make_request(65_536);
    let output = run(&request, &|| Ok(())).unwrap();
    assert!(output.status.success());
    let frame = wire::decode_response(
        &output.bytes,
        request.header(),
        request.header().owner(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        frame.bytes(),
        include_bytes!(
            "../../../crates/services/paintcraft-formats/tests/jxr-fixtures/rgb8-4x3.jxr"
        )
    );
    assert_eq!(frame.inspection().resolution_x_bits, Some(0));
    assert_eq!(frame.inspection().resolution_y_bits, Some(0));
    assert!(frame.heap_peak() > 0 && frame.heap_peak() <= decode::HEAP_BYTES);
    for cap in [1, 133, 341] {
        let request = make_request(cap);
        let output = run(&request, &|| Ok(())).unwrap();
        assert!(output.status.success());
        let error = wire::decode_response(
            &output.bytes,
            request.header(),
            request.header().owner(),
            &|| Ok(()),
        )
        .unwrap_err();
        // Only the typed capped-sink fault is a resource refusal.
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(&output.bytes[20..28], &[0; 8]);
    }
}
#[cfg(feature = "jxr-encode")]
#[test]
fn bad_header_hash_body_and_cross_direction_are_never_encoded() {
    let good = make_request(65_536);
    for index in [0, 8, 10, 11, 12, 20, 24, 32, 80, 81, 97, 129, 161] {
        let mut header = good.header().encode();
        header[index] ^= 1;
        let output = process::run_parent_lease(
            helper(),
            &["--jxr-encode".into()],
            vec![Arc::new(header.to_vec())],
            limits(&good),
            &|| Ok(()),
        );
        match output {
            Ok(output) => {
                assert!(!output.status.success());
                assert!(output.bytes.is_empty());
            }
            Err(error) => assert_eq!(error.code, ErrorCode::CodecFailed),
        }
    }
    for body in [vec![0; 36], vec![0; 35]] {
        let error = process::run(
            helper(),
            &["--jxr-encode".into()],
            vec![Arc::new(good.header().encode().to_vec()), Arc::new(body)],
            limits(&good),
            &|| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::CodecFailed);
    }
    let decoded =
        decode::Request::capture_for(decode::Codec::JxlRgba8, Arc::new(vec![1]), owner(), 1)
            .unwrap();
    let output = process::run_parent_lease(
        helper(),
        &["--jxr-encode".into()],
        vec![Arc::new(decoded.header.encode())],
        limits(&good),
        &|| Ok(()),
    );
    match output {
        Ok(output) => {
            assert_eq!(output.status.code(), Some(2));
            assert!(output.bytes.is_empty());
        }
        Err(error) => assert_eq!(error.code, ErrorCode::CodecFailed),
    }
    let output = Command::new(helper())
        .args(["--jxr-encode", "unused-output-path"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
