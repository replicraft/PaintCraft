//! Independently JXRLib-authored RGB8 fixtures through the actual native helper.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{
    codec_worker::{self as wire, Codec, Owner, Pixels, Request},
    jxr,
};
use paintcraft_platform::source::PrivateSource;
use paintcraft_runtime::process::{self, Limits};
use std::{ffi::OsString, path::Path, sync::Arc, time::Duration};
use uuid::Uuid;
const SMALL: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/jxr-fixtures/rgb8-4x3.jxr");
const EDGE: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/jxr-fixtures/rgb8-17x19.jxr");
fn request(source: &[u8], pixels: u64) -> Request {
    Request::capture_for(
        Codec::JxrRgb8,
        Arc::new(source.to_vec()),
        Owner {
            session: Uuid::new_v4(),
            state: Uuid::new_v4(),
            epoch: 17,
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
        deadline: Duration::from_secs(2),
        resource_exit_code: Some(wire::RESOURCE_EXIT),
    }
}
fn run(r: &Request, check: &dyn Fn() -> Result<()>) -> Result<process::Output> {
    let source = PrivateSource::create(
        r.source.as_slice(),
        "jxr",
        wire::INPUT_BYTES_MAX as u64,
        &|| Ok(()),
    )?;
    let directory = source.path().parent().unwrap().to_path_buf();
    let args = [
        OsString::from("--jxr-source"),
        source.path().as_os_str().to_owned(),
    ];
    let result = process::run_parent_lease(
        Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
        &args,
        r.segments(),
        limits(),
        check,
    );
    // Any return from this supervisor has already killed/reaped/joined its child.
    source.close()?;
    assert!(!directory.exists());
    result
}
#[test]
fn independent_rgb8_pixels_color_policy_and_exact_metadata_survive_actual_worker() {
    for (source, width, height) in [(SMALL, 4, 3), (EDGE, 17, 19)] {
        let info = jxr::inspect_checked(source, wire::PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(
            jxr::COLOR_POLICY,
            "t832-uint-rgb-absent-context-default-srgb"
        );
        assert_eq!((info.width, info.height), (width, height));
        assert_eq!(info.resolution_x_bits, Some(0));
        assert_eq!(info.resolution_y_bits, Some(0));
        let r = request(source, wire::PIXELS_MAX);
        let output = run(&r, &|| Ok(())).unwrap();
        let frame = wire::decode(&output.bytes, &r, r.header.owner).unwrap();
        let expected: Vec<_> = (0..height)
            .flat_map(|y| {
                (0..width).flat_map(move |x| {
                    [
                        ((x * 47 + y * 13) % 256) as u8,
                        ((x * 47 + y * 13 + 71) % 256) as u8,
                        ((x * 47 + y * 13 + 142) % 256) as u8,
                        255,
                    ]
                })
            })
            .collect();
        assert_eq!(frame.pixels, Pixels::SrgbRgba8(&expected));
        assert_eq!((frame.width, frame.height), (width, height));
        assert!(frame.heap_peak > 0 && frame.heap_peak <= wire::HEAP_BYTES);
        assert_eq!(r.source.as_slice(), source);
        let mut owner = r.header.owner;
        owner.epoch += 1;
        assert_eq!(
            wire::decode(&output.bytes, &r, owner).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        let mut wrong = r.clone();
        wrong.header.codec = Codec::JxlRgba8;
        assert!(wire::decode(&output.bytes, &wrong, wrong.header.owner).is_err());
    }
}
#[test]
fn header_and_private_source_disagreements_fail_before_selected_decoder() {
    let r = request(SMALL, wire::PIXELS_MAX);
    let missing = process::run_parent_lease(
        Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
        &[],
        r.segments(),
        limits(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        wire::decode(&missing.bytes, &r, r.header.owner)
            .unwrap_err()
            .code,
        ErrorCode::CodecFailed
    );
    let wrong =
        PrivateSource::create(EDGE, "jxr", wire::INPUT_BYTES_MAX as u64, &|| Ok(())).unwrap();
    let args = [
        OsString::from("--jxr-source"),
        wrong.path().as_os_str().to_owned(),
    ];
    let output = process::run_parent_lease(
        Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
        &args,
        r.segments(),
        limits(),
        &|| Ok(()),
    )
    .unwrap();
    wrong.close().unwrap();
    assert_eq!(
        wire::decode(&output.bytes, &r, r.header.owner)
            .unwrap_err()
            .code,
        ErrorCode::CodecFailed
    );
    let r = request(SMALL, 11);
    let output = run(&r, &|| Ok(())).unwrap();
    assert_eq!(
        wire::decode(&output.bytes, &r, r.header.owner)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let mut metadata = SMALL.to_vec();
    let ifd = u32::from_le_bytes(metadata[4..8].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(metadata[ifd..ifd + 2].try_into().unwrap()) as usize;
    // Replace an admitted resolution field with ICC, not a hidden conversion.
    let pos = (0..count)
        .map(|index| ifd + 2 + index * 12)
        .find(|pos| u16::from_le_bytes(metadata[*pos..*pos + 2].try_into().unwrap()) == 0xbc82)
        .unwrap();
    metadata[pos..pos + 2].copy_from_slice(&0x8773u16.to_le_bytes());
    let r = request(&metadata, wire::PIXELS_MAX);
    let output = run(&r, &|| Ok(())).unwrap();
    assert_eq!(
        wire::decode(&output.bytes, &r, r.header.owner)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn admitted_header_truncated_entropy_and_cancel_are_confined_and_private_backing_is_removed() {
    let info = jxr::inspect_checked(SMALL, wire::PIXELS_MAX, &|| Ok(())).unwrap();
    let mut bytes = SMALL[..info.image_offset + 18].to_vec();
    let ifd = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(bytes[ifd..ifd + 2].try_into().unwrap()) as usize;
    let pos = (0..count)
        .map(|index| ifd + 2 + index * 12)
        .find(|pos| u16::from_le_bytes(bytes[*pos..*pos + 2].try_into().unwrap()) == 0xbcc1)
        .unwrap();
    bytes[pos + 8..pos + 12].copy_from_slice(&18u32.to_le_bytes());
    assert!(jxr::inspect_checked(&bytes, wire::PIXELS_MAX, &|| Ok(())).is_ok());
    let r = request(&bytes, wire::PIXELS_MAX);
    match run(&r, &|| Ok(())) {
        Ok(output) => assert!(wire::decode(&output.bytes, &r, r.header.owner).is_err()),
        Err(error) => assert!(matches!(
            error.code,
            ErrorCode::CodecFailed | ErrorCode::ResourceLimit
        )),
    }
    let r = request(EDGE, wire::PIXELS_MAX);
    let calls = std::cell::Cell::new(0);
    let result = run(&r, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == 2 {
            Err(Error::new(ErrorCode::OperationCancelled, "stop child"))
        } else {
            Ok(())
        }
    });
    assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
}
