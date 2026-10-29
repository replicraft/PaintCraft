//! Actual opt-in V6/code6 child producer; no frontend admission or Job adoption.
#[path = "support/build_record.rs"]
mod build_record;
use paintcraft_core::types::{ErrorCode, Result};
use paintcraft_formats::codec_worker::{self as wire, Codec, Owner, Request};
use paintcraft_runtime::process::{self, Limits};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/services/paintcraft-formats/tests/avif-sdr-fixtures")
            .join(name),
    )
    .unwrap()
}
fn request(bytes: &[u8], pixels: u64) -> Request {
    Request::capture_for(
        Codec::AvifSdr8,
        Arc::new(bytes.to_vec()),
        Owner {
            session: Uuid::new_v4(),
            state: Uuid::new_v4(),
            epoch: 9,
            frontend: 3,
        },
        pixels,
    )
    .unwrap()
}
fn run(
    request: &Request,
    output_bytes: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<process::Output> {
    process::run_parent_lease(
        Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
        &[],
        request.segments(),
        Limits {
            input_bytes: wire::INPUT_BYTES_MAX + wire::REQUEST_HEADER,
            output_bytes,
            deadline: Duration::from_secs(3),
            resource_exit_code: Some(wire::RESOURCE_EXIT),
        },
        check,
    )
}
#[cfg(not(feature = "avif-sdr-codec"))]
#[test]
fn v4_and_v5_helpers_cannot_claim_the_opt_in_sdr_producer() {
    let request = request(
        &fixture("420-vertical-full-even-alpha.avif"),
        wire::PIXELS_MAX,
    );
    let response = run(&request, wire::OUTPUT_BYTES_MAX, &|| Ok(())).unwrap();
    assert!(response.status.success());
    assert_eq!(
        wire::decode(&response.bytes, &request, request.header.owner)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let bytes = std::fs::read(env!("CARGO_BIN_EXE_paintcraft-codec-worker")).unwrap();
    build_record::assert_one(&bytes, &build_record::selected());
}
#[cfg(feature = "avif-sdr-codec")]
#[test]
fn actual_v6_producer_matches_independent_fraction_rgba_for_all24_coded_pictures() {
    for size in ["even", "odd"] {
        for (layout, siting) in [
            ("444", "fullchroma"),
            ("420", "vertical"),
            ("420", "colocated"),
        ] {
            for range in ["full", "limited"] {
                for alpha in ["", "-alpha"] {
                    let stem = format!("{layout}-{siting}-{range}-{size}{alpha}");
                    let bytes = fixture(&format!("{stem}.avif"));
                    let request = request(&bytes, wire::PIXELS_MAX);
                    let response = run(&request, wire::OUTPUT_BYTES_MAX, &|| Ok(())).unwrap();
                    assert!(response.status.success(), "{stem}: {:?}", response.status);
                    let frame =
                        wire::decode(&response.bytes, &request, request.header.owner).unwrap();
                    assert_eq!(
                        (frame.width, frame.height),
                        if size == "odd" { (17, 13) } else { (16, 16) }
                    );
                    assert!(frame.heap_peak > 0 && frame.heap_peak <= wire::HEAP_BYTES);
                    let wire::Pixels::SrgbRgba8(pixels) = frame.pixels else {
                        panic!("typed SDR output")
                    };
                    assert_eq!(pixels, fixture(&format!("{stem}.rational.rgba")), "{stem}");
                    assert_eq!(request.source.as_slice(), bytes);
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
        }
    }
}
#[cfg(feature = "avif-sdr-codec")]
#[test]
fn actual_v6_producer_refuses_unknown_siting_identity_and_conflicting_metadata() {
    for size in ["even", "odd"] {
        for range in ["full", "limited"] {
            for alpha in ["", "-alpha"] {
                let request = request(
                    &fixture(&format!("420-unknown-{range}-{size}{alpha}.avif")),
                    wire::PIXELS_MAX,
                );
                let output = run(&request, wire::OUTPUT_BYTES_MAX, &|| Ok(())).unwrap();
                assert!(output.status.success());
                assert_eq!(
                    wire::decode(&output.bytes, &request, request.header.owner)
                        .unwrap_err()
                        .code,
                    ErrorCode::CapabilityUnsupported
                );
            }
        }
    }
    let identity = include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/avif-fixtures/alpha.avif"
    );
    let mut mismatch = fixture("444-fullchroma-full-even-alpha.avif");
    let color = mismatch
        .windows(4)
        .position(|bytes| bytes == b"colr")
        .unwrap()
        + 4;
    mismatch[color + 9] = 0;
    for bytes in [identity.as_slice(), mismatch.as_slice()] {
        let request = request(bytes, wire::PIXELS_MAX);
        let output = run(&request, wire::OUTPUT_BYTES_MAX, &|| Ok(())).unwrap();
        assert!(output.status.success());
        assert_eq!(
            wire::decode(&output.bytes, &request, request.header.owner)
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
#[cfg(feature = "avif-sdr-codec")]
#[test]
fn actual_v6_producer_preserves_source_budget_and_cancellation_boundaries() {
    use paintcraft_core::types::Error;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let bytes = fixture("420-colocated-limited-odd-alpha.avif");
    let small = request(&bytes, 220);
    let output = run(&small, wire::OUTPUT_BYTES_MAX, &|| Ok(())).unwrap();
    assert!(output.status.success());
    assert_eq!(
        wire::decode(&output.bytes, &small, small.header.owner)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let mut wrong = request(&bytes, wire::PIXELS_MAX);
    wrong.header.sha256[0] ^= 1;
    let output = run(&wrong, wire::OUTPUT_BYTES_MAX, &|| Ok(())).unwrap();
    assert!(output.status.success());
    assert!(wire::decode(&output.bytes, &wrong, wrong.header.owner).is_err());
    let valid = request(&bytes, wire::PIXELS_MAX);
    assert_eq!(
        run(&valid, wire::RESPONSE_HEADER - 1, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let calls = AtomicUsize::new(0);
    assert_eq!(
        run(&valid, wire::OUTPUT_BYTES_MAX, &|| {
            if calls.fetch_add(1, Ordering::SeqCst) > 0 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel before publication",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(valid.source.as_slice(), bytes);
}

#[cfg(all(feature = "avif-sdr-codec", not(feature = "float-codecs")))]
#[test]
fn actual_v6_build_record_is_unique_and_does_not_claim_legacy_profiles() {
    let bytes = std::fs::read(env!("CARGO_BIN_EXE_paintcraft-codec-worker")).unwrap();
    let expected = wire::native_avif_sdr_build_record(env!("CARGO_PKG_VERSION")).unwrap();
    build_record::assert_one(&bytes, &expected);
    assert_eq!(wire::QUALIFIED_AVIF_SDR_IMPORT_VARIANTS.len(), 5);
    assert!(
        wire::QUALIFIED_AVIF_SDR_IMPORT_VARIANTS.contains(&wire::NATIVE_AVIF_SDR_IMPORT_VARIANT)
    );
    assert!(wire::AVIF_SDR_NATIVE_PROFILE.len() <= 128 && expected.len() <= 512);
}
