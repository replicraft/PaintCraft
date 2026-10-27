//! Private own-output read mode. No ordinary decoder/profile promotion.
#[cfg(not(feature = "jxr-encode"))]
#[test]
fn default_worker_refuses_writer_source_argument() {
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker"))
        .args(["--jxr-writer-source", "/not-opened.jxr"])
        .stdin(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(2));
}
#[cfg(feature = "jxr-encode")]
#[path = "native_jxr_writer/matrix.rs"]
mod matrix;
#[cfg(feature = "jxr-encode")]
mod private {
    use paintcraft_core::types::{Error, ErrorCode, Result};
    use paintcraft_formats::{codec_worker as wire, jxr};
    use paintcraft_platform::source::PrivateSource;
    use paintcraft_runtime::process::{self, Limits};
    use sha2::{Digest, Sha256};
    use std::{
        ffi::OsString,
        fs,
        path::{Path, PathBuf},
        sync::Arc,
        time::Duration,
    };
    use uuid::Uuid;
    const WIDE: &[u8] = include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/jxr-fixtures/white-rgb8-4096x1.jxr"
    );
    const TALL: &[u8] = include_bytes!(
        "../../../crates/services/paintcraft-formats/tests/jxr-fixtures/white-rgb8-1x4096.jxr"
    );
    fn request(source: &[u8]) -> wire::Request {
        wire::Request::capture_for(
            wire::Codec::JxrRgb8,
            Arc::new(source.to_vec()),
            wire::Owner {
                session: Uuid::new_v4(),
                state: Uuid::new_v4(),
                epoch: 19,
                frontend: 3,
            },
            1_048_576,
        )
        .unwrap()
    }
    fn limits() -> Limits {
        Limits {
            input_bytes: wire::REQUEST_HEADER + (64 << 20),
            output_bytes: wire::OUTPUT_BYTES_MAX,
            deadline: Duration::from_secs(10),
            resource_exit_code: Some(wire::RESOURCE_EXIT),
        }
    }
    fn run(
        helper: &Path,
        request: &wire::Request,
        writer: bool,
        staged: &[u8],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<process::Output> {
        let source = PrivateSource::create(staged, "jxr", 64 << 20, check)?;
        let directory = source.path().parent().unwrap().to_path_buf();
        let args = [
            OsString::from(if writer {
                "--jxr-writer-source"
            } else {
                "--jxr-source"
            }),
            source.path().as_os_str().to_owned(),
        ];
        let output = process::run_parent_lease(helper, &args, request.segments(), limits(), check);
        source.close()?;
        assert!(!directory.exists());
        output
    }
    fn current_helper() -> &'static Path {
        Path::new(env!("CARGO_BIN_EXE_paintcraft-codec-worker"))
    }
    #[test]
    fn fixed_private_mode_reads_exact_long_pixels_and_legacy_stays_short_only() {
        for (source, size) in [(WIDE, (4096, 1)), (TALL, (1, 4096))] {
            let req = request(source);
            let output = run(current_helper(), &req, true, source, &|| Ok(())).unwrap();
            let frame = wire::decode(&output.bytes, &req, req.header.owner).unwrap();
            assert_eq!((frame.width, frame.height), size);
            assert_eq!(frame.pixels, wire::Pixels::SrgbRgba8(&vec![255; 4096 * 4]));
            let output = run(current_helper(), &req, false, source, &|| Ok(())).unwrap();
            assert_eq!(
                wire::decode(&output.bytes, &req, req.header.owner)
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
            let mut stale = req.header.owner;
            stale.epoch += 1;
            let output = run(current_helper(), &req, true, source, &|| Ok(())).unwrap();
            assert_eq!(
                wire::decode(&output.bytes, &req, stale).unwrap_err().code,
                ErrorCode::ConflictStaleBase
            );
        }
    }
    #[test]
    fn source_file_metadata_codec_and_prelaunch_cancel_refuse_without_output_adoption() {
        let req = request(WIDE);
        let mut changed = WIDE.to_vec();
        changed[0] ^= 1;
        let output = run(current_helper(), &req, true, &changed, &|| Ok(())).unwrap();
        assert!(wire::decode(&output.bytes, &req, req.header.owner).is_err());
        let named = include_bytes!(
            "../../../crates/services/paintcraft-formats/tests/jxr-fixtures/rgb8-dpi96x144.jxr"
        );
        let req = request(named);
        let output = run(current_helper(), &req, true, named, &|| Ok(())).unwrap();
        assert_eq!(
            wire::decode(&output.bytes, &req, req.header.owner)
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        let mut req = request(WIDE);
        req.header.codec = wire::Codec::JxlRgba8;
        let output = run(current_helper(), &req, true, WIDE, &|| Ok(())).unwrap();
        assert_eq!(
            wire::decode(&output.bytes, &req, req.header.owner)
                .unwrap_err()
                .code,
            ErrorCode::CodecFailed
        );
        assert_eq!(
            run(current_helper(), &request(WIDE), true, WIDE, &|| Err(
                Error::new(ErrorCode::OperationCancelled, "prelaunch cancel")
            ))
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
    }
    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|n| format!("{n:02x}")).collect()
    }
    #[test]
    #[ignore = "requires explicit immutable helper, retained matrix and pinned C reference"]
    fn retained_29_writer_files_match_selected_decoder_and_independent_c_pixels() {
        let helper = PathBuf::from(
            std::env::var_os("PAINTCRAFT_JXR_WRITER_HELPER").expect("immutable private helper"),
        );
        let expected =
            std::env::var("PAINTCRAFT_JXR_WRITER_HELPER_SHA256").expect("frozen helper SHA");
        assert!(helper.is_absolute());
        assert_eq!(hex(&Sha256::digest(fs::read(&helper).unwrap())), expected);
        let root = PathBuf::from(
            std::env::var_os("PAINTCRAFT_JXR_WRITER_MATRIX").expect("retained exact matrix"),
        );
        assert!(root.is_absolute());
        // Pin the committed writer source and its exact immutable provenance,
        // then independently pin every retained JXR/RGB leaf below.
        for (name, expected) in [
            (
                "source.tar",
                "719bca7d51fc9f53ec2d42c4782af7a97295073d10137559aba7d3934f76b8ea",
            ),
            (
                "source-receipt.json",
                "d18f8c1109fd673c0a7dd76461bd95281234d353469b8509a59dafaa9ba65df0",
            ),
            (
                "producer/proof.json",
                "f2e0ffb45cc09b66a18615e7b49727173649af3e27a1e286f68abb006522e35f",
            ),
        ] {
            assert_eq!(
                hex(&Sha256::digest(fs::read(root.join(name)).unwrap())),
                expected
            );
        }
        let source_receipt = fs::read_to_string(root.join("source-receipt.json")).unwrap();
        assert!(source_receipt.contains("3e2b65db389495d7c786c2884f832b8b2158f873"));
        let helper_bytes = fs::read(&helper).unwrap();
        let record = wire::jxr_encode::candidate::BUILD_RECORD;
        assert_eq!(
            helper_bytes
                .windows(record.len())
                .filter(|bytes| *bytes == record)
                .count(),
            1
        );
        let oracle =
            PathBuf::from(std::env::var_os("PAINTCRAFT_JXR_REFERENCE").expect("pinned C oracle"));
        assert_eq!(
            hex(&Sha256::digest(fs::read(&oracle).unwrap())),
            "8e3b71db2c53c5db5f988b8755946a30ac1070b7a6c0a8ffa6933d34d26b95ce"
        );
        let scratch =
            std::env::temp_dir().join(format!("paintcraft-jxr-writer-read-{}", Uuid::new_v4()));
        fs::create_dir(&scratch).unwrap();
        let mut count = 0;
        for case in super::matrix::CASES {
            let path = root.join(case.family).join(format!("{}.jxr", case.name));
            let bytes = fs::read(&path).unwrap();
            assert_eq!(hex(&Sha256::digest(&bytes)), case.jxr_sha256);
            let plan = jxr::assess_writer_import(&bytes, 1_048_576, &|| Ok(())).unwrap();
            let info = plan.source();
            let raw = fs::read(path.with_extension("source.rgb")).unwrap();
            assert_eq!(raw.len(), info.width as usize * info.height as usize * 3);
            assert_eq!(hex(&Sha256::digest(&raw)), case.rgb_sha256);
            let req = request(&bytes);
            let output = run(&helper, &req, true, &bytes, &|| Ok(())).unwrap();
            let frame = wire::decode(&output.bytes, &req, req.header.owner).unwrap();
            let expected_rgba: Vec<_> = raw
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect();
            assert_eq!(frame.pixels, wire::Pixels::SrgbRgba8(&expected_rgba));
            let decoded = scratch.join(format!("{count}.rgb"));
            let args = [
                OsString::from("decode"),
                info.width.to_string().into(),
                info.height.to_string().into(),
                path.as_os_str().to_owned(),
                decoded.as_os_str().to_owned(),
            ];
            let output = process::run(
                &oracle,
                &args,
                vec![],
                Limits {
                    input_bytes: 1,
                    output_bytes: 4096,
                    deadline: Duration::from_secs(10),
                    resource_exit_code: None,
                },
                &|| Ok(()),
            )
            .unwrap();
            assert!(output.status.success());
            assert_eq!(fs::read(decoded).unwrap(), raw);
            println!(
                "JXR_WRITER_READ {} file_sha={} RGB_exact=true C_exact=true",
                path.file_name().unwrap().to_string_lossy(),
                hex(&Sha256::digest(&bytes))
            );
            count += 1;
        }
        assert_eq!(count, 29);
        assert_eq!(hex(&Sha256::digest(fs::read(&helper).unwrap())), expected);
        // Raw oracle output retained for qualification; not a product output path.
        println!("JXR_WRITER_READ retained={}", scratch.display());
    }
}
