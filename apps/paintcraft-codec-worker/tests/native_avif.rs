//! Private code5 protocol qualification through the real parent-lease supervisor.
//! No frontend gate or V4 Job adoption is enabled by this test.
#[cfg(feature = "avif-codec")]
use paintcraft_core::types::Error;
use paintcraft_core::types::{ErrorCode, Result};
#[cfg(feature = "avif-codec")]
use paintcraft_formats::codec_worker::Pixels;
use paintcraft_formats::codec_worker::{self as wire, Codec, Owner, Request};
use paintcraft_runtime::process::{self, Limits};
#[cfg(feature = "avif-codec")]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{path::Path, sync::Arc, time::Duration};
use uuid::Uuid;
macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../crates/services/paintcraft-formats/tests/avif-fixtures/",
            $name
        ))
    };
}
fn request(source: &[u8], pixels: u64) -> Request {
    Request::capture_for(
        Codec::AvifSrgb8,
        Arc::new(source.to_vec()),
        Owner {
            session: Uuid::new_v4(),
            state: Uuid::new_v4(),
            epoch: 4,
            frontend: 3,
        },
        pixels,
    )
    .unwrap()
}
fn run(request: &Request, check: &dyn Fn() -> Result<()>) -> Result<process::Output> {
    process::run_parent_lease(
        Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
        &[],
        request.segments(),
        Limits {
            input_bytes: wire::INPUT_BYTES_MAX + wire::REQUEST_HEADER,
            output_bytes: wire::OUTPUT_BYTES_MAX,
            deadline: Duration::from_secs(3),
            resource_exit_code: Some(wire::RESOURCE_EXIT),
        },
        check,
    )
}
#[cfg(not(feature = "avif-codec"))]
#[test]
fn default_worker_cannot_claim_the_optional_avif_decoder() {
    let request = request(fixture!("opaque.avif"), 256);
    let response = run(&request, &|| Ok(())).unwrap();
    assert!(
        response.status.success(),
        "helper exited after response: {:?}",
        response.status
    );
    assert_eq!(
        wire::decode(&response.bytes, &request, request.header.owner)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}
#[cfg(feature = "avif-codec")]
#[test]
fn actual_native_rgb_and_straight_alpha_match_independent_libaom() {
    for (source, expected) in [
        (
            fixture!("opaque.avif").as_slice(),
            fixture!("opaque.libaom.rgba").as_slice(),
        ),
        (
            fixture!("alpha.avif").as_slice(),
            fixture!("alpha.libaom.rgba").as_slice(),
        ),
    ] {
        let request = request(source, 256);
        let response = run(&request, &|| Ok(())).unwrap();
        assert!(
            response.status.success(),
            "helper exited after response: {:?}",
            response.status
        );
        let frame = wire::decode(&response.bytes, &request, request.header.owner).unwrap();
        assert_eq!((frame.width, frame.height), (16, 16));
        assert!(frame.heap_peak > 0 && frame.heap_peak <= wire::HEAP_BYTES);
        let Pixels::SrgbRgba8(rgba) = frame.pixels else {
            panic!("explicit sRGB output")
        };
        assert_eq!(rgba, expected);
        let mut foreign = request.header.owner;
        foreign.epoch += 1;
        assert_eq!(
            wire::decode(&response.bytes, &request, foreign)
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
    }
}
#[cfg(feature = "avif-codec")]
#[test]
fn actual_native_refuses_budget_source_mismatch_corruption_and_cancel() {
    let small = request(fixture!("alpha.avif"), 255);
    let response = run(&small, &|| Ok(())).unwrap();
    assert!(response.status.success());
    assert_eq!(
        wire::decode(&response.bytes, &small, small.header.owner)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let mut wrong = request(fixture!("opaque.avif"), 256);
    wrong.header.sha256[0] ^= 1;
    let response = run(&wrong, &|| Ok(())).unwrap();
    assert!(response.status.success());
    assert!(wire::decode(&response.bytes, &wrong, wrong.header.owner).is_err());
    let mut broken = fixture!("opaque.avif").to_vec();
    broken.truncate(broken.len() - 1);
    let broken = request(&broken, 256);
    let response = run(&broken, &|| Ok(())).unwrap();
    assert!(response.status.success());
    assert!(wire::decode(&response.bytes, &broken, broken.header.owner).is_err());
    let valid = request(fixture!("alpha.avif"), 256);
    let calls = AtomicUsize::new(0);
    let error = run(&valid, &|| {
        if calls.fetch_add(1, Ordering::SeqCst) > 0 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel before publication",
            ))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
}
