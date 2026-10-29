//! Actual opt-in V7 float process qualification, never a public import route.
#[path = "support/build_record.rs"]
mod build_record;
#[cfg(feature = "float-codecs")]
use paintcraft_core::types::{
    Error,
    json::{self, Value},
};
use paintcraft_core::types::{ErrorCode, Result};
use paintcraft_formats::{
    codec_worker::{self as wire, Codec, Owner, Request},
    precision::Settings,
};
#[cfg(feature = "float-codecs")]
use paintcraft_formats::{float_import, precision};
use paintcraft_runtime::process::{self, Limits};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/services/paintcraft-formats/tests")
}
fn load(path: impl AsRef<Path>) -> Vec<u8> {
    std::fs::read(path).unwrap()
}
fn owner() -> Owner {
    Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 4,
        frontend: 3,
    }
}
fn request(bytes: Vec<u8>, settings: Settings, codec: Codec, pixels: u64) -> Request {
    wire::float::capture(codec, Arc::new(bytes), settings, owner(), pixels).unwrap()
}
fn run(request: &Request, limit: usize, check: &dyn Fn() -> Result<()>) -> Result<process::Output> {
    process::run_parent_lease(
        Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
        &[],
        request.segments(),
        Limits {
            input_bytes: wire::REQUEST_HEADER + wire::float::INPUT_BYTES_MAX,
            output_bytes: limit,
            deadline: Duration::from_secs(10),
            resource_exit_code: Some(wire::RESOURCE_EXIT),
        },
        check,
    )
}
#[cfg(feature = "float-codecs")]
fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
#[cfg(feature = "float-codecs")]
fn settings(value: &Value) -> Settings {
    let space = |v| match v {
        "rec709" => precision::LinearSpace::Rec709D65,
        "radiance" => precision::LinearSpace::RadianceEqualEnergy,
        _ => panic!("space"),
    };
    Settings {
        curve: if text(value, "curve") == Some("reinhard") {
            precision::Curve::ReinhardPerChannel
        } else {
            precision::Curve::Clip
        },
        negative: if text(value, "negative") == Some("reject") {
            precision::Negative::Reject
        } else {
            precision::Negative::ClipToBlack
        },
        exposure_milli_ev: value.get("exposure_milli_ev").unwrap().as_i64().unwrap() as i32,
        source_assignment: text(value, "assignment").map(space),
        alpha_assignment: text(value, "alpha_assignment").map(|v| {
            if v == "straight" {
                precision::AlphaAssignment::Straight
            } else {
                precision::AlphaAssignment::Premultiplied
            }
        }),
        alpha_output: value.get("background").and_then(Value::as_array).map_or(
            precision::AlphaOutput::Preserve,
            |v| precision::AlphaOutput::Flatten {
                srgb_background: std::array::from_fn(|i| v[i].as_i64().unwrap() as u8),
            },
        ),
    }
}
#[cfg(not(feature = "float-codecs"))]
#[test]
fn default_helper_refuses_private_float_without_output() {
    for codec in [Codec::HdrSdr8, Codec::ExrSdr8] {
        let request = request(
            load(root().join("radiance-fixtures/literal-2x2.hdr")),
            Settings::default(),
            codec,
            4,
        );
        let output = run(&request, wire::float::OUTPUT_BYTES_MAX, &|| Ok(())).unwrap();
        assert!(output.status.success());
        assert_eq!(
            wire::decode(&output.bytes, &request, request.header.owner)
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
#[cfg(feature = "float-codecs")]
#[test]
fn actual_v7_all17_external_reference_cases_validate_without_parent_float_decoding() {
    use paintcraft_core::types::resources::SharedBudget;
    let manifest = json::parse(&load(root().join("float-import-fixtures/manifest.json"))).unwrap();
    for case in manifest.get("cases").unwrap().as_array().unwrap() {
        let id = text(case, "id").unwrap();
        let family = text(case, "family").unwrap();
        let name = text(case, "file").unwrap();
        let directory = if name == "aspect-2.hdr" {
            "float-import-fixtures"
        } else if family == "hdr" {
            "radiance-fixtures"
        } else {
            "exr-fixtures"
        };
        let source = load(root().join(directory).join(name));
        let before = source.clone();
        let settings = settings(case.get("settings").unwrap());
        let codec = if family == "hdr" {
            Codec::HdrSdr8
        } else {
            Codec::ExrSdr8
        };
        let request = request(source.clone(), settings, codec, wire::float::PIXELS_MAX);
        let output = run(&request, wire::output_limit(codec), &|| Ok(())).unwrap();
        assert!(output.status.success(), "{id}: {:?}", output.status);
        let frame = wire::decode(&output.bytes, &request, request.header.owner);
        if let Some(error) = text(case, "expected_error") {
            assert_eq!(format!("{:?}", frame.unwrap_err().code), error, "{id}");
            continue;
        }
        let frame = frame.unwrap_or_else(|e| panic!("{id}: {e}"));
        let wire::Pixels::SrgbRgba8(rgba) = frame.pixels else {
            panic!("float model")
        };
        assert_eq!(
            rgba,
            load(
                root()
                    .join("float-import-fixtures")
                    .join(text(case, "expected_rgba_file").unwrap())
            ),
            "{id}"
        );
        let budget = SharedBudget::new(8 << 20);
        let plan = if family == "hdr" {
            float_import::plan_hdr(
                &source,
                wire::float::PIXELS_MAX,
                settings,
                budget.clone(),
                &|| Ok(()),
            )
        } else {
            float_import::plan_exr(
                &source,
                wire::float::PIXELS_MAX,
                settings,
                budget.clone(),
                &|| Ok(()),
            )
        }
        .unwrap();
        let checked = plan
            .accept_report(
                frame.float_report.unwrap(),
                (frame.width, frame.height),
                rgba,
                &|| Ok(()),
            )
            .unwrap();
        assert_eq!(checked.assessment().binding().settings(), settings);
        assert_eq!(source, before);
        assert!(frame.heap_peak <= wire::HEAP_BYTES);
        drop(checked);
        assert_eq!(budget.usage().reserved, 0);
        println!("V7 {id}: exact independent RGBA/assessment");
    }
}
#[cfg(feature = "float-codecs")]
#[test]
fn v7_refuses_malformed_request_response_owner_shapes_and_quota_declarations() {
    let source = load(root().join("radiance-fixtures/literal-2x2.hdr"));
    let valid = request(source, Settings::default(), Codec::HdrSdr8, 4);
    let output = run(&valid, wire::output_limit(valid.header.codec), &|| Ok(())).unwrap();
    assert!(output.status.success());
    for at in [10, 15, 20, 23, 31] {
        let mut envelope = valid.source.as_ref().clone();
        envelope[at] = 255;
        let invalid =
            Request::capture_for(Codec::HdrSdr8, Arc::new(envelope), valid.header.owner, 4)
                .unwrap();
        let result = run(&invalid, wire::float::OUTPUT_BYTES_MAX, &|| Ok(())).unwrap();
        assert!(result.status.success());
        assert!(wire::decode(&result.bytes, &invalid, invalid.header.owner).is_err());
    }
    for at in [11, 12, 20, 28, 36, 39, 48, 89, 105] {
        let mut changed = output.bytes.clone();
        changed[at] ^= 1;
        assert!(
            wire::decode(&changed, &valid, valid.header.owner).is_err(),
            "response byte{at}"
        );
    }
    let mut foreign = valid.header.owner;
    foreign.epoch += 1;
    assert_eq!(
        wire::decode(&output.bytes, &valid, foreign)
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    let mut header = valid.header;
    header.length = wire::float::INPUT_BYTES_MAX as u64 + 1;
    assert_eq!(
        wire::Header::decode(&header.encode()).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    header = valid.header;
    header.pixels = wire::float::PIXELS_MAX as u32 + 1;
    assert_eq!(
        wire::Header::decode(&header.encode()).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(run(&valid, wire::RESPONSE_HEADER, &|| Ok(())).is_err());
    assert_eq!(
        run(&valid, wire::float::OUTPUT_BYTES_MAX, &|| Err(Error::new(
            ErrorCode::OperationCancelled,
            "cancel before result"
        )))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    let bytes = load(env!("CARGO_BIN_EXE_paintcraft-codec-worker"));
    build_record::assert_one(&bytes, &build_record::selected());
    assert_eq!(wire::QUALIFIED_AVIF_SDR_IMPORT_VARIANTS.len(), 5);
    assert_eq!(wire::QUALIFIED_FLOAT_IMPORT_VARIANTS.len(), 7);
}
#[cfg(feature = "float-codecs")]
#[test]
fn request_read_cancel_deadline_and_complete_request_lease_loss_stop_the_real_helper() {
    use std::{
        io::Write,
        process::{Command, Stdio},
        time::Instant,
    };
    let read_request = request(
        load(root().join("radiance-fixtures/literal-2x2.hdr")),
        Settings::default(),
        Codec::HdrSdr8,
        4,
    );
    // Header-only input deliberately leaves the child reading its declared
    // request body. These are request-read cancellation/deadline observations.
    let start = Instant::now();
    let error = process::run_parent_lease(
        Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
        &[],
        vec![Arc::new(read_request.header.encode())],
        Limits {
            input_bytes: wire::float::INPUT_BYTES_MAX + wire::REQUEST_HEADER,
            output_bytes: wire::float::OUTPUT_BYTES_MAX,
            deadline: Duration::from_secs(2),
            resource_exit_code: Some(wire::RESOURCE_EXIT),
        },
        &|| {
            if start.elapsed() > Duration::from_millis(50) {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "request-read cancellation",
                ))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    let error = process::run_parent_lease(
        Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
        &[],
        vec![Arc::new(read_request.header.encode())],
        Limits {
            input_bytes: wire::float::INPUT_BYTES_MAX + wire::REQUEST_HEADER,
            output_bytes: wire::float::OUTPUT_BYTES_MAX,
            deadline: Duration::from_millis(50),
            resource_exit_code: Some(wire::RESOURCE_EXIT),
        },
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    // A full admitted million-pixel request exercises the real worker's lease
    // hook. EOF may occur during preparation or output; no mid-decode claim.
    let mut source = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 1000 +X 1000\n".to_vec();
    for _ in 0..1_000_000 {
        source.extend([128, 128, 128, 128]);
    }
    let request = request(source, Settings::default(), Codec::HdrSdr8, 1_000_000);
    let mut child = Command::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut writer = child.stdin.take().unwrap();
    writer.write_all(&request.header.encode()).unwrap();
    writer.write_all(&request.source).unwrap();
    drop(writer);
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("worker did not stop after complete-request lease loss");
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(status.code(), Some(76));
}
