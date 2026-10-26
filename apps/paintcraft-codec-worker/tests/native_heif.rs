//! Actual subprocess qualification, without GUI initialization or document writes.
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_formats::{
    codec_worker::{self as wire, Owner, Request},
    heif,
};
use paintcraft_runtime::process::{self, Limits};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;
const PRIMARY: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/heif-fixtures/primary.heic");
const GRID: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/heif-fixtures/grid.heic");
const ALPHA: &[u8] =
    include_bytes!("../../../crates/services/paintcraft-formats/tests/heif-fixtures/alpha.heic");
fn request(bytes: &[u8]) -> Request {
    Request::capture(
        Arc::new(bytes.to_vec()),
        Owner {
            session: Uuid::new_v4(),
            state: Uuid::new_v4(),
            epoch: 1,
            frontend: 3,
        },
        heif::PIXELS_MAX,
    )
    .unwrap()
}
fn limits() -> Limits {
    Limits {
        input_bytes: heif::INPUT_BYTES_MAX + wire::REQUEST_HEADER,
        output_bytes: wire::OUTPUT_BYTES_MAX,
        deadline: Duration::from_secs(10),
        resource_exit_code: Some(wire::RESOURCE_EXIT),
    }
}
fn run(r: &Request) -> process::Output {
    process::run_parent_lease(
        Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
        &[],
        r.segments(),
        limits(),
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn actual_worker_primary_grid_alpha_match_independent_native_sample_oracle() {
    for (source, width, alpha) in [(PRIMARY, 32, false), (GRID, 64, false), (ALPHA, 32, true)] {
        let r = request(source);
        let output = run(&r);
        let frame = wire::decode(&output.bytes, &r, r.header.owner).unwrap();
        assert_eq!((frame.width, frame.height), (width, 24));
        let expected: Vec<u8> = (0..24)
            .flat_map(|y| {
                (0..width).map(move |x| {
                    if x < 32 {
                        ((x * 7 + y * 11) % 256) as u8
                    } else {
                        17
                    }
                })
            })
            .collect();
        let wire::Pixels::NativeGray8 {
            gray,
            alpha: decoded_alpha,
            encoding,
        } = frame.pixels
        else {
            panic!()
        };
        assert_eq!(gray, expected);
        assert_eq!(decoded_alpha.is_some(), alpha);
        if let Some(a) = decoded_alpha {
            assert!(a.iter().all(|sample| *sample == 17));
        }
        assert_eq!(
            (
                encoding.primaries,
                encoding.transfer,
                encoding.matrix,
                encoding.full_range
            ),
            (2, 2, 2, true)
        );
        assert!(frame.heap_peak > 0 && frame.heap_peak <= wire::HEAP_BYTES);
        assert_eq!(r.source.as_slice(), source);
        assert_eq!(
            wire::decode(
                &output.bytes,
                &r,
                Owner {
                    epoch: 2,
                    ..r.header.owner
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::ConflictStaleBase
        );
    }
}
#[test]
fn coded_dimension_mismatch_and_malformed_pps_remain_private_failures() {
    let mut smaller = PRIMARY.to_vec();
    let marker = smaller.windows(4).position(|b| b == b"ispe").unwrap();
    // Width is a declaration, and must agree with the coded SPS (32).
    smaller[marker + 8..marker + 12].copy_from_slice(&31u32.to_be_bytes());
    let r = request(&smaller);
    assert!(wire::decode(&run(&r).bytes, &r, r.header.owner).is_err());
    let mut pps = PRIMARY.to_vec();
    let parsed = heif::inspect_checked(&pps, heif::PIXELS_MAX, &|| Ok(())).unwrap();
    let data = parsed
        .items
        .iter()
        .flat_map(|item| &item.properties)
        .find_map(|a| match a.property {
            heif::Property::HevcConfig { bytes, .. } => Some(bytes),
            _ => None,
        })
        .unwrap();
    let base = data.as_ptr() as usize - pps.as_ptr() as usize;
    let mut pos = 23;
    let arrays = data[22];
    let mut corrupt = None;
    for _ in 0..arrays {
        let kind = data[pos] & 63;
        pos += 1;
        let count = u16::from_be_bytes(data[pos..pos + 2].try_into().unwrap());
        pos += 2;
        for _ in 0..count {
            let n = u16::from_be_bytes(data[pos..pos + 2].try_into().unwrap()) as usize;
            pos += 2;
            if kind == 34 {
                corrupt = Some((base + pos + 2, n - 2));
            }
            pos += n;
        }
    }
    let (start, n) = corrupt.unwrap();
    pps[start..start + n].fill(0);
    let r = request(&pps);
    assert!(wire::decode(&run(&r).bytes, &r, r.header.owner).is_err());
    assert_eq!(PRIMARY, request(PRIMARY).source.as_slice());
}
#[test]
fn cancellation_and_deadline_terminate_child_before_returning() {
    let r = request(GRID);
    let polls = AtomicUsize::new(0);
    let error = process::run_parent_lease(
        Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
        &[],
        r.segments(),
        limits(),
        &|| {
            if polls.fetch_add(1, Ordering::Relaxed) > 0 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "test cancellation",
                ))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    let mut timeout = limits();
    timeout.deadline = Duration::from_nanos(1);
    assert_eq!(
        process::run_parent_lease(
            Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
            &[],
            r.segments(),
            timeout,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(r.source.as_slice(), GRID);
}

#[test]
fn existing_executor_reserves_full_fixed_heap_until_owned_completion_consumed() {
    let r = request(PRIMARY);
    let charge = r.reservation_bytes();
    assert!(charge >= wire::HEAP_BYTES + PRIMARY.len() as u64 + wire::OUTPUT_BYTES_MAX as u64);
    let budget = paintcraft_runtime::Limits {
        workers: 1,
        pending_jobs: 1,
        transient_bytes: charge,
    };
    let mut executor = paintcraft_runtime::Executor::new(budget).unwrap();
    let mut foreign: paintcraft_runtime::Executor<process::Output> =
        paintcraft_runtime::Executor::new(budget).unwrap();
    let captured = r.clone();
    let handle = executor
        .submit(
            charge,
            move |cancel| {
                process::run_parent_lease(
                    Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker")),
                    &[],
                    captured.segments(),
                    limits(),
                    &|| cancel.check(),
                )
            },
            || {},
        )
        .unwrap();
    assert_eq!(executor.reserved_bytes(), charge);
    assert!(foreign.poll_for(handle).is_none());
    let start = std::time::Instant::now();
    let result = loop {
        if let Some(result) = executor.poll_for(handle) {
            break result.unwrap();
        }
        assert!(start.elapsed() < Duration::from_secs(15));
        std::thread::sleep(Duration::from_millis(2));
    };
    assert_eq!(executor.reserved_bytes(), 0);
    let frame = wire::decode(&result.bytes, &r, r.header.owner).unwrap();
    assert!(matches!(frame.pixels, wire::Pixels::NativeGray8 { gray, .. } if gray.len() == 32*24));
}

#[test]
fn unsupported_profile_uses_canonical_protocol_refusal_not_generic_exit() {
    let mut source = PRIMARY.to_vec();
    let config = source.windows(4).position(|v| v == b"hvcC").unwrap() + 4;
    // Container explicitly declares 4:2:0; the initial worker profile accepts
    // only native monochrome. This is a benign unsupported subtype, not panic.
    source[config + 16] = (source[config + 16] & !3) | 1;
    let r = request(&source);
    let output = run(&r);
    assert_eq!(
        wire::decode(&output.bytes, &r, r.header.owner)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}

#[test]
fn linked_helper_contains_cross_target_packaging_build_record() {
    let header = request(PRIMARY).header.encode();
    assert_eq!(&header[..8], b"PCDEC002");
    assert_eq!(&header[8..10], &2u16.to_be_bytes());
    assert_eq!(wire::HEAP_BYTES, 268435456);
    let bytes = std::fs::read(env!("CARGO_BIN_EXE_paintcraft-codec-worker")).unwrap();
    #[cfg(not(feature = "avif-codec"))]
    let record = concat!(
        "PAINTCRAFT_CODEC_WORKER_BUILD_V4\0",
        env!("CARGO_PKG_VERSION"),
        "\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0END_CODEC_WORKER_BUILD\0"
    )
    .as_bytes();
    #[cfg(all(feature = "avif-codec", not(feature = "avif-sdr-codec")))]
    let record = concat!(
        "PAINTCRAFT_CODEC_WORKER_BUILD_V5_AVIF\0",
        env!("CARGO_PKG_VERSION"),
        "\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0AVIF_DAV1D154_ALLOC1_ASM0_8BIT1\0END_CODEC_WORKER_BUILD\0"
    )
    .as_bytes();
    #[cfg(all(feature = "avif-sdr-codec",not(feature="float-codecs")))]
    let record = concat!(
        "PAINTCRAFT_CODEC_WORKER_BUILD_V6_AVIF_SDR\0", env!("CARGO_PKG_VERSION"),
        "\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0AVIF_DAV1D154_ALLOC1_ASM0_8BIT1\0AVIF_SDR709_YUV444_420_CSP12_WORK1\0END_CODEC_WORKER_BUILD\0",
    ).as_bytes();
    #[cfg(not(feature = "avif-codec"))]
    assert_eq!(
        record,
        wire::native_build_record(env!("CARGO_PKG_VERSION")).unwrap()
    );
    #[cfg(all(feature = "avif-codec", not(feature = "avif-sdr-codec")))]
    assert_eq!(
        record,
        wire::native_avif_build_record(env!("CARGO_PKG_VERSION")).unwrap()
    );
    #[cfg(all(feature = "avif-sdr-codec", not(feature = "float-codecs")))]
    assert_eq!(
        record,
        wire::native_avif_sdr_build_record(env!("CARGO_PKG_VERSION")).unwrap()
    );
    #[cfg(all(feature = "float-codecs", not(feature = "jxr-encode")))]
    let record_buffer = wire::native_float_build_record(env!("CARGO_PKG_VERSION")).unwrap();
    #[cfg(feature = "jxr-encode")]
    let record_buffer =
        wire::jxr_encode::candidate::build_record(env!("CARGO_PKG_VERSION")).unwrap();
    #[cfg(feature = "float-codecs")]
    let record = record_buffer.as_slice();
    assert!(
        bytes
            .windows(record.len())
            .any(|candidate| candidate == record)
    );
}
