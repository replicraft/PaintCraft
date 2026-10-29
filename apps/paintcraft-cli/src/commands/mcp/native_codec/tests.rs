use super::*;
use std::{fs, process::Command, sync::atomic::AtomicUsize};
#[test]
fn rpc_lifetime_drop_invalidates_blocking_publication() {
    let life = RpcLife::new();
    let state = life.0.clone();
    assert!(!state.load(Ordering::Acquire));
    drop(life);
    assert!(state.load(Ordering::Acquire));
}
#[test]
fn copied_installed_real_helper_cancellation_preserves_mcp_session_and_retires_charge() {
    cancellation_installation(None);
}
fn cancellation_installation(selected_helper: Option<&std::path::Path>) {
    let root = std::env::temp_dir().join(format!("paintcraft-mcp-cancel-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let current = std::env::current_exe().unwrap();
    let helper_name = if cfg!(windows) {
        "paintcraft-codec-worker.exe"
    } else {
        "paintcraft-codec-worker"
    };
    let helper = current
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(helper_name);
    assert!(
        helper.exists(),
        "Build actual helper first: cargo build -p paintcraft-codec-worker"
    );
    let child = root.join(current.file_name().unwrap());
    fs::copy(&current, &child).unwrap();
    let installed = root.join(helper_name);
    fs::copy(selected_helper.unwrap_or(&helper), &installed).unwrap();
    normalize_copied_test_image(&child);
    if selected_helper.is_none() {
        normalize_copied_test_image(&installed);
    }
    let target = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => panic!("release host required"),
    };
    let bytes = fs::read(&installed).unwrap();
    let (profile, variants) = helper_contract(&bytes, env!("CARGO_PKG_VERSION"))
        .expect("copied helper must have one exact current V4/V5/V6/V7 contract");
    let writer = matches!(
        profile,
        wire::jxr_encode::qualified::NATIVE_PROFILE
            | wire::jxl_integer::contracts::writer::NATIVE_PROFILE
    );
    let mut manifest = serde_json::json!({"schema_version":if writer {3} else {2},"embedded_build_record_verified":true,
        "application_version":env!("CARGO_PKG_VERSION"),"target":target,"executable":helper_name,
        "sha256":hex(&Sha256::digest(fs::read(&installed).unwrap())),"protocol_version":wire::PROTOCOL_VERSION,
        "profile":profile,"requested_heap_ceiling_bytes":wire::HEAP_BYTES,
        "qualified_import_variants":variants});
    if writer {
        manifest["encode_protocol_version"] = serde_json::json!(1);
        manifest["qualified_export_variants"] =
            serde_json::json!(wire::jxr_encode::qualified::EXPORT_VARIANTS);
    }
    fs::write(
        root.join("paintcraft-codec-worker.json"),
        manifest.to_string(),
    )
    .unwrap();
    fs::write(
        root.join("source.jxl"),
        include_bytes!(
            "../../../../../../crates/services/paintcraft-formats/tests/jxl-fixtures/rgba8.jxl"
        ),
    )
    .unwrap();
    let output = Command::new(child)
        .args([
            "--exact",
            "commands::mcp::native_codec::tests::fixture_cancelled_import",
            "--ignored",
            "--nocapture",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
fn normalize_copied_test_image(path: &std::path::Path) {
    let before = fs::metadata(path).unwrap().len();
    // The real installed-helper verifier bounds both application and helper at
    // 256MiB and includes verification in the operation deadline. Linux DWARF
    // can exceed either fixture allowance; strip it from both private copies
    // before contract extraction/hashing, preserving the real verifier limits.
    #[cfg(all(target_os = "linux", debug_assertions))]
    {
        let output = Command::new("strip")
            .arg("--strip-debug")
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "strip private fixture: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            fs::metadata(path).unwrap().len() <= 256 << 20,
            "private fixture remains beyond installed executable allowance after stripping debug metadata"
        );
    }
    eprintln!(
        "copied cancellation image {}: {before} -> {} bytes",
        path.display(),
        fs::metadata(path).unwrap().len()
    );
}
#[test]
#[ignore = "private copied-installation cancellation fixture"]
fn fixture_cancelled_import() {
    let input = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("source.jxl");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(input.parent().unwrap().join("paintcraft-codec-worker.json")).unwrap(),
    )
    .unwrap();
    let expected_codec = if matches!(
        manifest["profile"].as_str(),
        Some(
            wire::jxl_integer::contracts::base::NATIVE_PROFILE
                | wire::jxl_integer::contracts::writer::NATIVE_PROFILE
        )
    ) {
        wire::Codec::JxlIntegerRgba8
    } else {
        wire::Codec::JxlRgba8
    };
    let mut state = State {
        session: Some(Session::new(
            new_document(3, 4, Some(Rgba([255; 4])), 0, POLICY).unwrap(),
        )),
        ..Default::default()
    };
    let before = state.inspection().unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let launched = Arc::new(AtomicBool::new(false));
    let checks = Arc::new(AtomicUsize::new(0));
    let flag = cancelled.clone();
    let check: Check = Arc::new(move || {
        if flag.load(Ordering::Acquire) {
            Err(Error::new(ErrorCode::OperationCancelled, "RPC cancelled"))
        } else {
            Ok(())
        }
    });
    let flag = cancelled.clone();
    let probes = checks.clone();
    let entered = launched.clone();
    let started = std::time::Instant::now();
    let error = import_with_launch(
        &mut state,
        &input,
        &[],
        check,
        Box::new(move |launch, check| {
            assert_eq!(launch.request.header.codec, expected_codec);
            entered.store(true, Ordering::Release);
            // Real verified helper waits for its missing source while the retained
            // parent lease remains open. Second supervisor check occurs after spawn.
            process::run_parent_lease(
                launch.executable,
                launch.arguments,
                vec![Arc::new(launch.request.header.encode().to_vec())],
                process::Limits {
                    input_bytes: wire::INPUT_BYTES_MAX + wire::REQUEST_HEADER,
                    output_bytes: wire::OUTPUT_BYTES_MAX,
                    deadline: launch.remaining,
                    resource_exit_code: Some(wire::RESOURCE_EXIT),
                },
                &|| {
                    if probes.fetch_add(1, Ordering::AcqRel) > 0 {
                        flag.store(true, Ordering::Release);
                    }
                    check()
                },
            )
            .map(|output| output.bytes)
        }),
    )
    .unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::OperationCancelled,
        "actual cancellation error={error:?}; launch_entered={}; supervisor_checks={}; elapsed={:?}; application_bytes={}; helper_bytes={}",
        launched.load(Ordering::Acquire),
        checks.load(Ordering::Acquire),
        started.elapsed(),
        fs::metadata(std::env::current_exe().unwrap())
            .unwrap()
            .len(),
        fs::metadata(input.parent().unwrap().join(if cfg!(windows) {
            "paintcraft-codec-worker.exe"
        } else {
            "paintcraft-codec-worker"
        }))
        .unwrap()
        .len()
    );
    assert!(checks.load(Ordering::Acquire) >= 2);
    assert_eq!(state.inspection().unwrap(), before);
    assert!(state.import_identity.is_none());
    assert_eq!(
        preparation_budget().usage().reserved,
        0,
        "job retires only after supervisor returns reaped/joined"
    );
}

#[path = "../../../../tests/support/codec_contract.rs"]
mod codec_contract;
use codec_contract::helper_contract;

#[test]
fn copied_helper_contract_accepts_exact_finite_records_and_refuses_corrupt_or_mixed() {
    let version = env!("CARGO_PKG_VERSION");
    for (record, expected) in [
        (
            wire::native_build_record(version).unwrap(),
            wire::NATIVE_PROFILE,
        ),
        (
            wire::native_avif_build_record(version).unwrap(),
            wire::AVIF_NATIVE_PROFILE,
        ),
        (
            wire::native_avif_sdr_build_record(version).unwrap(),
            wire::AVIF_SDR_NATIVE_PROFILE,
        ),
        (
            wire::native_float_build_record(version).unwrap(),
            wire::FLOAT_NATIVE_PROFILE,
        ),
        (
            wire::jxl_integer::contracts::base::build_record(version).unwrap(),
            wire::jxl_integer::contracts::base::NATIVE_PROFILE,
        ),
        (
            wire::jxl_integer::contracts::writer::build_record(version).unwrap(),
            wire::jxl_integer::contracts::writer::NATIVE_PROFILE,
        ),
    ] {
        let (profile, variants) = helper_contract(&record, version).unwrap();
        assert_eq!(profile, expected);
        if profile == wire::FLOAT_NATIVE_PROFILE {
            assert_eq!(variants, wire::QUALIFIED_FLOAT_IMPORT_VARIANTS);
            assert_eq!(variants.len(), 7);
            assert!(variants.contains(&wire::NATIVE_HDR_SDR_IMPORT_VARIANT));
            assert!(variants.contains(&wire::NATIVE_EXR_SDR_IMPORT_VARIANT));
        }
        let mut duplicate = record.clone();
        duplicate.extend(&record);
        assert!(helper_contract(&duplicate, version).is_none());
        let mut corrupted_duplicate = record.clone();
        let mut corrupted = record.clone();
        *corrupted.last_mut().unwrap() ^= 1;
        corrupted_duplicate.extend(corrupted);
        assert!(helper_contract(&corrupted_duplicate, version).is_none());
        let mut truncated_duplicate = record.clone();
        truncated_duplicate.extend(&record[..record.len() - 1]);
        assert!(helper_contract(&truncated_duplicate, version).is_none());
        assert!(helper_contract(&record[..record.len() - 1], version).is_none());
        assert!(helper_contract(&record, "different-version").is_none());
    }
    let mut mixed = wire::native_build_record(version).unwrap();
    mixed.extend(wire::native_avif_sdr_build_record(version).unwrap());
    assert!(helper_contract(&mixed, version).is_none());
    let mut mixed_float = wire::native_avif_sdr_build_record(version).unwrap();
    mixed_float.extend(wire::native_float_build_record(version).unwrap());
    assert!(helper_contract(&mixed_float, version).is_none());
    assert!(helper_contract(b"unrelated", version).is_none());
}

#[test]
#[ignore = "requires immutable PAINTCRAFT_V9_HELPER and PAINTCRAFT_V10_HELPER"]
fn copied_integer_helpers_cancel_after_real_launch_without_mcp_publication() {
    for key in ["PAINTCRAFT_V9_HELPER", "PAINTCRAFT_V10_HELPER"] {
        let path = std::path::PathBuf::from(std::env::var_os(key).expect("qualified helper path"));
        cancellation_installation(Some(&path));
    }
}
