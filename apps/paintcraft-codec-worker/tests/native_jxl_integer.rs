//! Original encoded fixtures enter only the actual isolated helper here.
#[path = "support/build_record.rs"]
mod build_record;
use paintcraft_core::types::ErrorCode;
use paintcraft_formats::codec_worker::{self as wire, Codec, Owner, Pixels, Request};
use paintcraft_runtime::process::{self, Limits};
use std::{path::Path, sync::Arc, time::Duration};
use uuid::Uuid;
const RGBA16: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba16.jxl");
const RGBA8: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba8.jxl");
fn request(source: &[u8], codec: Codec) -> Request {
    Request::capture_for(
        codec,
        Arc::new(source.to_vec()),
        Owner {
            session: Uuid::new_v4(),
            state: Uuid::new_v4(),
            epoch: 9,
            frontend: 3,
        },
        6,
    )
    .unwrap()
}
fn run(request: &Request) -> Vec<u8> {
    process::run_parent_lease(
        Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
        &[],
        request.segments(),
        Limits {
            input_bytes: wire::REQUEST_HEADER + wire::INPUT_BYTES_MAX,
            output_bytes: wire::OUTPUT_BYTES_MAX,
            deadline: Duration::from_secs(10),
            resource_exit_code: Some(wire::RESOURCE_EXIT),
        },
        &|| Ok(()),
    )
    .unwrap()
    .bytes
}
#[test]
fn actual_integer_family_uses_one_request_and_exact_report_for_both_source_depths() {
    build_record::assert_one(
        &std::fs::read(env!("CARGO_BIN_EXE_paintcraft-codec-worker")).unwrap(),
        &build_record::selected(),
    );
    for (source, expected, depth, changes) in [
        (
            RGBA8,
            vec![
                10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150,
                254, 160, 170, 180, 255,
            ],
            8,
            (0, 0),
        ),
        (
            RGBA16,
            vec![
                0, 0, 255, 1, 1, 127, 128, 0, 255, 4, 8, 128, 16, 32, 64, 255,
            ],
            16,
            (8, 2),
        ),
    ] {
        let request = request(source, Codec::JxlIntegerRgba8);
        let bytes = run(&request);
        let result = wire::decode(&bytes, &request, request.header.owner);
        if cfg!(feature = "jxl-integer-codec") {
            let frame = result.unwrap();
            assert_eq!(frame.pixels, Pixels::SrgbRgba8(&expected));
            let report = frame.jxl_report.unwrap();
            assert_eq!(report.source_depth, depth);
            assert_eq!(
                (report.changed_rgb_components, report.changed_alpha),
                changes
            );
            assert_eq!(report.losses().is_empty(), depth == 8);
            report
                .validate_payload(request.header, frame.width, frame.height, &expected)
                .unwrap();
            assert!(frame.heap_peak > 0 && frame.heap_peak <= wire::HEAP_BYTES);
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::CapabilityUnsupported);
        }
        assert_eq!(request.source.as_slice(), source);
    }
}
#[test]
fn old_exact8_request_never_adopts16_and_new_family_refuses_other_profiles() {
    let old = request(RGBA16, Codec::JxlRgba8);
    assert_eq!(
        wire::decode(&run(&old), &old, old.header.owner)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    for source in [
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/icc.jxl")
            .as_slice(),
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/linear.jxl")
            .as_slice(),
        include_bytes!(
            "../../../crates/services/paintcraft-formats/tests/jxl-fixtures/animation.jxl"
        )
        .as_slice(),
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/alpha-profiles/color8-alpha16.jxl").as_slice(),
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/alpha-profiles/color16-alpha8.jxl").as_slice(),
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/alpha-profiles/alpha-float.jxl").as_slice(),
        include_bytes!("../../../crates/services/paintcraft-formats/tests/jxl-fixtures/alpha-profiles/alpha-named.jxl").as_slice(),
    ] {
        let request = request(source, Codec::JxlIntegerRgba8);
        assert_eq!(
            wire::decode(&run(&request), &request, request.header.owner)
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
