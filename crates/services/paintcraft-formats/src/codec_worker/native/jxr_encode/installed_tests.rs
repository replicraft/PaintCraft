//! Explicit actual private-installation qualification; never relabel a decoder.
use super::*;
use paintcraft_core::types::{
    json::{self, Value},
    resources::SharedBudget,
};
use paintcraft_runtime::process;
use std::{cell::Cell, fs, process::Command};
const INNER: &str = "codec_worker::native::jxr_encode::installed_tests::installed_fixture";
fn target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => panic!("supported native target required"),
    }
}
fn manifest(helper: &Path) -> Value {
    Value::object([
        ("schema_version", Value::Int(3)),
        ("embedded_build_record_verified", Value::Bool(true)),
        ("application_version", Value::str(env!("CARGO_PKG_VERSION"))),
        ("target", Value::str(target())),
        (
            "executable",
            Value::str(helper.file_name().unwrap().to_str().unwrap()),
        ),
        (
            "sha256",
            Value::str(hex(&Sha256::digest(fs::read(helper).unwrap()))),
        ),
        ("protocol_version", Value::Int(2)),
        ("profile", Value::str(wire::candidate::NATIVE_PROFILE)),
        (
            "requested_heap_ceiling_bytes",
            Value::Int(HEAP_BYTES as i64),
        ),
        (
            "qualified_import_variants",
            Value::Array(
                wire::candidate::IMPORT_VARIANTS
                    .iter()
                    .map(|v| Value::str(*v))
                    .collect(),
            ),
        ),
        ("encode_protocol_version", Value::Int(1)),
        (
            "qualified_export_variants",
            Value::Array(
                wire::candidate::EXPORT_VARIANTS
                    .iter()
                    .map(|v| Value::str(*v))
                    .collect(),
            ),
        ),
    ])
}
#[test]
#[ignore = "requires an actual private encoder helper and explicit independent JXR reference"]
fn actual_private_installed_encoder_parent_lifetime_and_pixels() {
    let source = PathBuf::from(
        std::env::var_os("PAINTCRAFT_JXR_ENCODE_HELPER").expect("explicit private producer helper"),
    );
    let reference = std::env::var_os("PAINTCRAFT_JXR_REFERENCE")
        .expect("explicit pinned independent reference");
    let root = std::env::temp_dir().join(format!("paintcraft-jxr-parent-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let current = std::env::current_exe().unwrap();
    let child = root.join(current.file_name().unwrap());
    fs::copy(&current, &child).unwrap();
    let helper = root.join(if cfg!(windows) {
        "paintcraft-codec-worker.exe"
    } else {
        "paintcraft-codec-worker"
    });
    let bytes = fs::read(&source).unwrap();
    let record = wire::candidate::build_record(env!("CARGO_PKG_VERSION")).unwrap();
    let positions = bytes
        .windows(wire::candidate::BUILD_RECORD_MARKER.len())
        .enumerate()
        .filter_map(|(at, part)| (part == wire::candidate::BUILD_RECORD_MARKER).then_some(at))
        .collect::<Vec<_>>();
    assert!(
        !positions.is_empty(),
        "actual candidate record required; production decoder cannot qualify this test"
    );
    assert!(
        positions
            .iter()
            .all(|at| bytes.get(*at..*at + record.len()) == Some(record.as_slice()))
    );
    fs::copy(&source, &helper).unwrap();
    // Preserve the actual producer descriptor; never relabel a production decoder.
    let producer_manifest = source.with_file_name(codec_helper::MANIFEST_NAME);
    let descriptor = fs::read(&producer_manifest).expect("actual producer schema3 descriptor");
    assert_eq!(json::parse(&descriptor).unwrap(), manifest(&helper));
    let fixture_manifest = root.join(codec_helper::MANIFEST_NAME);
    fs::copy(&producer_manifest, &fixture_manifest).unwrap();
    // Only the private fixture copy must permit the later tamper negative.
    // Keep the immutable producer descriptor and helper completely untouched.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&fixture_manifest, fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[cfg(not(unix))]
    {
        let mut permissions = fs::metadata(&fixture_manifest).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&fixture_manifest, permissions).unwrap();
    }
    let output = Command::new(&child)
        .args([
            "--exact",
            INNER,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("PAINTCRAFT_JXR_ENCODE_FIXTURE", &root)
        .env("PAINTCRAFT_JXR_REFERENCE", reference)
        .output()
        .unwrap();
    fs::write(root.join("parent.log"), &output.stdout).unwrap();
    fs::write(root.join("parent.stderr"), &output.stderr).unwrap();
    eprintln!("private JXR parent evidence {}", root.display());
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn owner() -> Owner {
    Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 7,
        frontend: 3,
    }
}
fn rgb(width: u32, height: u32) -> Vec<u8> {
    (0..height)
        .flat_map(|y| {
            (0..width).flat_map(move |x| {
                (0..3).map(move |channel| ((x * 47 + y * 13 + channel * 71) % 256) as u8)
            })
        })
        .collect()
}
fn launch(context: JxrEncodeLaunch<'_>, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    assert_eq!(context.arguments, [OsString::from("--jxr-encode")]);
    let layout = context.request.header().layout();
    process::run_parent_lease(
        context.executable,
        context.arguments,
        context.request.segments(),
        process::Limits {
            input_bytes: wire::REQUEST_HEADER + layout.rgb_bytes() as usize,
            output_bytes: layout.response_limit()? as usize,
            deadline: context.remaining,
            resource_exit_code: Some(RESOURCE_EXIT),
        },
        check,
    )
    .map(|output| output.bytes)
}
#[test]
#[ignore = "private outer installation only"]
fn installed_fixture() {
    let root =
        PathBuf::from(std::env::var_os("PAINTCRAFT_JXR_ENCODE_FIXTURE").expect("private fixture"));
    let reference = std::env::var_os("PAINTCRAFT_JXR_REFERENCE").unwrap();
    for (width, height) in [(1, 1), (4, 3), (17, 19), (33, 9)] {
        let expected = owner();
        let current = Cell::new(expected);
        let layout = wire::Layout::new(width, height, wire::ENCODED_BYTES_MAX).unwrap();
        let quote = layout.reservation_bytes().unwrap();
        let budget = SharedBudget::new(quote);
        let source = Arc::new(rgb(width, height));
        let source_hash: [u8; 32] = Sha256::digest(source.as_slice()).into();
        let response_pointer = Cell::new(0usize);
        let prepared = JxrEncodeJob {
            layout,
            owner: expected,
            deadline: Duration::from_secs(20),
        }
        .prepare(
            Arc::clone(&source),
            ResourceLease::new(budget.reserve(quote).unwrap()),
            &|| Ok(()),
            &|| Ok(current.get()),
            |actual, profile| {
                assert_eq!(actual, layout);
                assert_eq!(profile, wire::PROFILE);
                Ok(())
            },
            |context, check| {
                let bytes = launch(context, check)?;
                response_pointer.set(bytes.as_ptr() as usize);
                Ok(bytes)
            },
        )
        .unwrap();
        assert_eq!(budget.usage().reserved, quote);
        assert_eq!(
            prepared.bytes().as_ptr() as usize,
            response_pointer.get() + wire::RESPONSE_HEADER + env!("CARGO_PKG_VERSION").len(),
            "validated encoded body keeps the supervisor's one response allocation"
        );
        assert_eq!(
            Arc::strong_count(&source),
            1,
            "retired request does not retain RGB storage"
        );
        assert_eq!(prepared.header().rgb_sha256(), source_hash);
        assert_eq!(
            prepared.sha256(),
            <[u8; 32]>::from(Sha256::digest(prepared.bytes()))
        );
        assert_eq!(prepared.inspection().resolution_x_bits, Some(0));
        assert_eq!(prepared.inspection().resolution_y_bits, Some(0));
        let output = root.join(format!("parent-{width}x{height}.jxr"));
        let result = paintcraft_platform::save::save_with_preparation(
            &output,
            false,
            wire::ENCODED_BYTES_MAX,
            |sink| {
                sink.write_all(prepared.bytes())
                    .map_err(|e| Error::new(ErrorCode::StorageFailed, e.to_string()))
            },
            |_| prepared.check_current(&|| Ok(()), &|| Ok(current.get())),
        );
        assert!(result.is_success());
        assert_eq!(
            budget.usage().reserved,
            quote,
            "lease remains through publication receipt"
        );
        let decoded = root.join(format!("parent-{width}x{height}.rgb"));
        let oracle = Command::new(&reference)
            .args([
                "decode",
                &width.to_string(),
                &height.to_string(),
                output.to_str().unwrap(),
                decoded.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            oracle.status.success(),
            "{}",
            String::from_utf8_lossy(&oracle.stderr)
        );
        assert_eq!(fs::read(&decoded).unwrap(), source.as_slice());
        eprintln!(
            "{width}x{height} RGB={} JXR={} heap={} quote={quote}",
            hex(&source_hash),
            hex(&prepared.sha256()),
            prepared.heap_peak()
        );
        // The current-owner guard must also run at final Save preparation.
        let refused = root.join(format!("stale-{width}x{height}.jxr"));
        fs::write(&refused, b"previous output").unwrap();
        let outcome = paintcraft_platform::save::save_with_preparation(
            &refused,
            true,
            wire::ENCODED_BYTES_MAX,
            |sink| {
                sink.write_all(prepared.bytes())
                    .map_err(|e| Error::new(ErrorCode::StorageFailed, e.to_string()))
            },
            |_| {
                current.set(owner());
                prepared.check_current(&|| Ok(()), &|| Ok(current.get()))
            },
        );
        assert_eq!(
            outcome.replacement,
            paintcraft_platform::save::Replacement::NotReplaced
        );
        assert!(outcome.staged_sha256.is_some());
        assert_eq!(fs::read(&refused).unwrap(), b"previous output");
        drop(prepared);
        assert_eq!(budget.usage().reserved, 0);
    }
    for during in 0..5 {
        let expected = owner();
        let current = Cell::new(expected);
        let cancelled = Cell::new(false);
        let layout = wire::Layout::new(4, 3, if during == 0 { 1 } else { 1 << 20 }).unwrap();
        let quote = layout.reservation_bytes().unwrap();
        let budget = SharedBudget::new(quote);
        let error = JxrEncodeJob {
            layout,
            owner: expected,
            deadline: Duration::from_secs(20),
        }
        .prepare(
            Arc::new(rgb(4, 3)),
            ResourceLease::new(budget.reserve(quote).unwrap()),
            &|| {
                if cancelled.get() {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "retired cancellation",
                    ))
                } else {
                    Ok(())
                }
            },
            &|| Ok(current.get()),
            |_, _| Ok(()),
            |context, check| {
                let mut bytes = launch(context, check)?;
                match during {
                    1 => cancelled.set(true),
                    2 => current.set(owner()),
                    3 => bytes[125] ^= 1,
                    4 => *bytes.last_mut().unwrap() ^= 1,
                    _ => {}
                }
                Ok(bytes)
            },
        )
        .unwrap_err();
        // The approved private child maps the selected API's untyped Io error
        // (cap refusal or callback failure) to CodecFailed. A typed sink-error
        // API remains separate followup; do not infer ResourceLimit from text.
        assert_eq!(
            error.code,
            match during {
                0 | 4 => ErrorCode::CodecFailed,
                1 => ErrorCode::OperationCancelled,
                2 | 3 => ErrorCode::ConflictStaleBase,
                _ => unreachable!(),
            }
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    // A schema3 candidate is never silently accepted by ordinary decoder discovery.
    assert!(
        super::super::installed_helper(Codec::JxlRgba8, env!("CARGO_PKG_VERSION"), &|| Ok(()))
            .is_err()
    );
    let expected = owner();
    let layout = wire::Layout::new(4, 3, 1 << 20).unwrap();
    let quote = layout.reservation_bytes().unwrap();
    let budget = SharedBudget::new(quote);
    let prepared = JxrEncodeJob {
        layout,
        owner: expected,
        deadline: Duration::from_secs(20),
    }
    .prepare(
        Arc::new(rgb(4, 3)),
        ResourceLease::new(budget.reserve(quote).unwrap()),
        &|| Ok(()),
        &|| Ok(expected),
        |_, _| Ok(()),
        launch,
    )
    .unwrap();
    let manifest = root.join(codec_helper::MANIFEST_NAME);
    let original = fs::read(&manifest).unwrap();
    let mut changed = original.clone();
    changed.push(b' ');
    fs::write(&manifest, changed).unwrap();
    assert!(
        prepared
            .check_current(&|| Ok(()), &|| Ok(expected))
            .is_err(),
        "retained manifest pins are rechecked before publication"
    );
    drop(prepared);
    assert_eq!(budget.usage().reserved, 0);
    let mut forged = json::parse(&original).unwrap();
    let Value::Object(fields) = &mut forged else {
        unreachable!()
    };
    fields.insert("profile".into(), Value::str("unknown-private-encoder"));
    fs::write(&manifest, json::canonical(&forged)).unwrap();
    let error = JxrEncodeJob {
        layout,
        owner: expected,
        deadline: Duration::from_secs(20),
    }
    .prepare(
        Arc::new(rgb(4, 3)),
        ResourceLease::new(budget.reserve(quote).unwrap()),
        &|| Ok(()),
        &|| Ok(expected),
        |_, _| Ok(()),
        |_, _| panic!("unknown contract must not launch or retry"),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert_eq!(budget.usage().reserved, 0);
    fs::write(&manifest, original).unwrap();
}
