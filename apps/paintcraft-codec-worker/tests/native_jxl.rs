//! Independent libjxl fixture pixels through the actual bounded native helper.
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::codec_worker::{self as wire, Codec, Owner, Pixels, Request};
use paintcraft_runtime::process::{self, Limits};
use std::{path::Path, sync::Arc, time::Duration};
use uuid::Uuid;
const RGBA: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba8.jxl");
const SINGLE: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/single.jxl");
fn request(source: &[u8], pixels: u64) -> Request {
    Request::capture_for(
        Codec::JxlRgba8,
        Arc::new(source.to_vec()),
        Owner {
            session: Uuid::new_v4(),
            state: Uuid::new_v4(),
            epoch: 19,
            frontend: 3,
        },
        pixels,
    )
    .unwrap()
}
fn limits() -> Limits {
    Limits {
        input_bytes: wire::INPUT_BYTES_MAX + wire::REQUEST_HEADER,
        output_bytes: wire::OUTPUT_BYTES_MAX,
        deadline: Duration::from_secs(10),
        resource_exit_code: Some(wire::RESOURCE_EXIT),
    }
}
fn run(r: &Request) -> process::Output {
    process::run(
        Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
        &[],
        r.segments(),
        limits(),
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn actual_helper_rgb8_and_straight_alpha_match_independent_cjxl_fixture() {
    for (source, width, height, expected) in [
        (
            RGBA,
            3,
            2,
            vec![
                10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150,
                254, 160, 170, 180, 255,
            ],
        ),
        (SINGLE, 1, 1, vec![17, 29, 91, 137]),
    ] {
        let r = request(source, 6);
        let output = run(&r);
        let frame = wire::decode(&output.bytes, &r, r.header.owner).unwrap();
        assert_eq!((frame.width, frame.height), (width, height));
        assert_eq!(frame.pixels, Pixels::SrgbRgba8(&expected));
        assert!(frame.heap_peak > 0 && frame.heap_peak <= wire::HEAP_BYTES);
        assert_eq!(r.source.as_slice(), source);
        let mut changed = r.header.owner;
        changed.epoch += 1;
        assert_eq!(
            wire::decode(&output.bytes, &r, changed).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        let mut wrong_codec = r.clone();
        wrong_codec.header.codec = Codec::HeifGray8;
        assert!(wire::decode(&output.bytes, &wrong_codec, wrong_codec.header.owner).is_err());
    }
}
#[test]
fn unsupported_raw_profiles_fail_with_canonical_refusal_without_color_conversion() {
    for source in [
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba16.jxl")
            .as_slice(),
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/icc.jxl"),
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/linear.jxl"),
        include_bytes!(
            "../../../crates/services/paintcraft-formats/tests/jxl-fixtures/animation.jxl"
        ),
    ] {
        let r = request(source, wire::PIXELS_MAX);
        assert_eq!(
            wire::decode(&run(&r).bytes, &r, r.header.owner)
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    let r = request(RGBA, 5);
    assert_eq!(
        wire::decode(&run(&r).bytes, &r, r.header.owner)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn malformed_inputs_and_cross_codec_requests_do_not_return_pixels() {
    for end in [1, 2, 20, RGBA.len() - 1] {
        let r = request(&RGBA[..end], 6);
        assert!(wire::decode(&run(&r).bytes, &r, r.header.owner).is_err());
    }
    let mut mislabeled = request(RGBA, 6);
    mislabeled.header.codec = Codec::HeifGray8;
    assert!(
        wire::decode(
            &run(&mislabeled).bytes,
            &mislabeled,
            mislabeled.header.owner
        )
        .is_err()
    );
    let heif = include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/heif-fixtures/primary.heic"
    );
    let r = request(heif, wire::PIXELS_MAX);
    assert!(wire::decode(&run(&r).bytes, &r, r.header.owner).is_err());
}
#[test]
fn cancellation_and_deadline_still_kill_the_shared_decoder_process() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let r = request(RGBA, 6);
    let polls = AtomicUsize::new(0);
    let result = process::run(
        Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
        &[],
        r.segments(),
        limits(),
        &|| {
            if polls.fetch_add(1, Ordering::Relaxed) == 0 {
                Ok(())
            } else {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel JXL"))
            }
        },
    );
    assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    let short = Limits {
        deadline: Duration::from_nanos(1),
        ..limits()
    };
    assert_eq!(
        process::run(
            Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
            &[],
            r.segments(),
            short,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
}
