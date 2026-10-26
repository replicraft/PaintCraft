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
    fs::copy(helper, &installed).unwrap();
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
    let manifest = serde_json::json!({"schema_version":2,"embedded_build_record_verified":true,
        "application_version":env!("CARGO_PKG_VERSION"),"target":target,"executable":helper_name,
        "sha256":hex(&Sha256::digest(fs::read(&installed).unwrap())),"protocol_version":wire::PROTOCOL_VERSION,
        "profile":profile,"requested_heap_ceiling_bytes":wire::HEAP_BYTES,
        "qualified_import_variants":variants});
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
#[test]
#[ignore = "private copied-installation cancellation fixture"]
fn fixture_cancelled_import() {
    let input = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("source.jxl");
    let mut state = State {
        session: Some(Session::new(
            new_document(3, 4, Some(Rgba([255; 4])), 0, POLICY).unwrap(),
        )),
        ..Default::default()
    };
    let before = state.inspection().unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
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
    let error = import_with_launch(
        &mut state,
        &input,
        &[],
        check,
        Box::new(move |launch, check| {
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
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert!(checks.load(Ordering::Acquire) >= 2);
    assert_eq!(state.inspection().unwrap(), before);
    assert!(state.import_identity.is_none());
    assert_eq!(
        preparation_budget().usage().reserved,
        0,
        "job retires only after supervisor returns reaped/joined"
    );
}

fn helper_contract(bytes: &[u8], version: &str) -> Option<(&'static str, &'static [&'static str])> {
    let records = [
        (
            wire::BUILD_RECORD_MARKER,
            wire::native_build_record(version).ok()?,
            wire::NATIVE_PROFILE,
            wire::QUALIFIED_NATIVE_IMPORT_VARIANTS,
        ),
        (
            wire::AVIF_BUILD_RECORD_MARKER,
            wire::native_avif_build_record(version).ok()?,
            wire::AVIF_NATIVE_PROFILE,
            wire::QUALIFIED_AVIF_IMPORT_VARIANTS,
        ),
        (
            wire::AVIF_SDR_BUILD_RECORD_MARKER,
            wire::native_avif_sdr_build_record(version).ok()?,
            wire::AVIF_SDR_NATIVE_PROFILE,
            wire::QUALIFIED_AVIF_SDR_IMPORT_VARIANTS,
        ),
        (
            wire::FLOAT_BUILD_RECORD_MARKER,
            wire::native_float_build_record(version).ok()?,
            wire::FLOAT_NATIVE_PROFILE,
            wire::QUALIFIED_FLOAT_IMPORT_VARIANTS,
        ),
    ];
    let mut result = None;
    for (marker, record, profile, variants) in records {
        let positions = bytes
            .windows(marker.len())
            .enumerate()
            .filter_map(|(at, value)| (value == marker).then_some(at))
            .collect::<Vec<_>>();
        if positions.is_empty() {
            continue;
        }
        if result.is_some()
            || positions
                .iter()
                .any(|at| bytes.get(*at..*at + record.len()) != Some(record.as_slice()))
        {
            return None;
        }
        result = Some((profile, variants));
    }
    result
}

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
        assert_eq!(helper_contract(&duplicate, version).unwrap().0, expected);
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
