//! Owned HEIF color fixtures through the real allocator-bounded native process.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::codec_worker::{self as wire, Codec, Owner, Pixels, Request};
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
macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../crates/services/paintcraft-formats/tests/heif-color-fixtures/",
            $name
        ))
    };
}
fn request(source: &[u8], pixels: u64) -> Request {
    Request::capture_for(
        Codec::HeifSrgb8,
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
            deadline: Duration::from_secs(2),
            resource_exit_code: Some(wire::RESOURCE_EXIT),
        },
        check,
    )
}
type Case = (&'static [u8], &'static [u8], u32, u32, bool);
#[test]
fn real_heif_color_worker_matches_independent_pixels_with_exact_alpha_and_odd_edges() {
    let cases: [Case; 9] = [
        (
            fixture!("identity444.heic"),
            fixture!("identity444.rational-rgb"),
            32,
            24,
            false,
        ),
        (
            fixture!("identity444-alpha.heic"),
            fixture!("identity444.rational-rgb"),
            32,
            24,
            true,
        ),
        (
            fixture!("yuv444-full-alpha.heic"),
            fixture!("yuv444-full.rational-rgb"),
            32,
            24,
            true,
        ),
        (
            fixture!("yuv444-limited-alpha.heic"),
            fixture!("yuv444-limited.rational-rgb"),
            32,
            24,
            true,
        ),
        (
            fixture!("yuv420-full-alpha.heic"),
            fixture!("yuv420-full.rational-rgb"),
            32,
            24,
            true,
        ),
        (
            fixture!("yuv420-limited-alpha.heic"),
            fixture!("yuv420-limited.rational-rgb"),
            32,
            24,
            true,
        ),
        (
            fixture!("odd/identity444-alpha.heic"),
            fixture!("odd/identity444.rational-rgb"),
            33,
            25,
            true,
        ),
        (
            fixture!("odd/yuv444-full-alpha.heic"),
            fixture!("odd/yuv444-full.rational-rgb"),
            33,
            25,
            true,
        ),
        (
            fixture!("odd/yuv444-limited-alpha.heic"),
            fixture!("odd/yuv444-limited.rational-rgb"),
            33,
            25,
            true,
        ),
    ];
    for (source, reference, width, height, alpha) in cases {
        let request = request(source, wire::PIXELS_MAX);
        let result = run(&request, &|| Ok(())).unwrap();
        assert!(result.status.success());
        let frame = wire::decode(&result.bytes, &request, request.header.owner).unwrap();
        assert_eq!((frame.width, frame.height), (width, height));
        assert!(frame.heap_peak > 0 && frame.heap_peak <= wire::HEAP_BYTES);
        let Pixels::SrgbRgba8(rgba) = frame.pixels else {
            panic!("expected sRGB pixels")
        };
        assert_eq!(rgba.len(), width as usize * height as usize * 4);
        assert_eq!(reference.len(), width as usize * height as usize * 3);
        for (index, (pixel, expected)) in rgba
            .as_chunks::<4>()
            .0
            .iter()
            .zip(reference.as_chunks::<3>().0.iter())
            .enumerate()
        {
            for c in 0..3 {
                assert!(
                    pixel[c].abs_diff(expected[c]) <= 1,
                    "pixel{index}channel{c}"
                );
            }
            let x = index % width as usize;
            let y = index / width as usize;
            assert_eq!(
                pixel[3],
                if alpha {
                    [0, 1, 127, 128, 254, 255][(x + y) % 6]
                } else {
                    255
                }
            );
        }
        assert_eq!(request.source.as_slice(), source);
        let mut stale = request.header.owner;
        stale.epoch += 1;
        assert_eq!(
            wire::decode(&result.bytes, &request, stale)
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
        let mut oversized = result.bytes.clone();
        oversized.push(0);
        assert!(wire::decode(&oversized, &request, request.header.owner).is_err());
        let mut wrong = request.clone();
        wrong.header.codec = Codec::JxlRgba8;
        assert!(wire::decode(&result.bytes, &wrong, wrong.header.owner).is_err());
    }
}
#[test]
fn isolated_heif_color_refuses_unknown_color_metadata_corrupt_extents_and_budgets() {
    let source = fixture!("yuv420-full-alpha.heic");
    let mut nclx = source.to_vec();
    let pos = nclx.windows(4).position(|part| part == b"nclx").unwrap();
    nclx[pos + 7] = 2; // Explicit transfer13 becomes unspecified2, disagrees with VUI.
    let mut truncated = source.to_vec();
    truncated.pop();
    let gray = include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/heif-fixtures/primary.heic"
    );
    for bytes in [nclx.as_slice(), truncated.as_slice(), gray.as_slice()] {
        let r = request(bytes, wire::PIXELS_MAX);
        let result = run(&r, &|| Ok(())).unwrap();
        assert!(wire::decode(&result.bytes, &r, r.header.owner).is_err());
        assert_eq!(r.source.as_slice(), bytes);
    }
    let r = request(source, 767);
    let result = run(&r, &|| Ok(())).unwrap();
    assert_eq!(
        wire::decode(&result.bytes, &r, r.header.owner)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let mut r = request(source, wire::PIXELS_MAX);
    Arc::make_mut(&mut r.source)[0] ^= 1;
    let result = run(&r, &|| Ok(())).unwrap();
    assert!(wire::decode(&result.bytes, &r, r.header.owner).is_err());
}
#[test]
fn real_heif_color_child_is_drained_on_cancellation() {
    let r = request(fixture!("yuv420-full-alpha.heic"), wire::PIXELS_MAX);
    let checks = AtomicUsize::new(0);
    let error = run(&r, &|| {
        if checks.fetch_add(1, Ordering::SeqCst) >= 2 {
            Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
}
