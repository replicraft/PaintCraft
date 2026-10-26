//! Opt-in copied installation: real current_exe discovery, Executor and helper.
use super::*;
use paintcraft_core::types::{
    json::{self, Value},
    policy::hex,
};
use paintcraft_editor::session::Session;
use paintcraft_runtime::{Executor, Limits, process};
use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};
const INNER: &str =
    "codec_worker::native::import::sdr_installed_tests::installed_v6_sdr_job_fixture";
const COLOR: &str = "avif-color-reconstruction";
const NAMES: &str = "avif-item-names-not-retained";
fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/avif-sdr-fixtures")
            .join(name),
    )
    .unwrap()
}
fn target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => panic!("native release target required"),
    }
}
fn metadata(helper: &Path, profile: &str, variants: &[&str]) -> Value {
    Value::object([
        ("schema_version", Value::Int(2)),
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
        ("protocol_version", Value::Int(i64::from(PROTOCOL_VERSION))),
        ("profile", Value::str(profile)),
        (
            "requested_heap_ceiling_bytes",
            Value::Int(HEAP_BYTES as i64),
        ),
        (
            "qualified_import_variants",
            Value::Array(variants.iter().map(|value| Value::str(*value)).collect()),
        ),
    ])
}
#[test]
#[ignore = "requires an actual opt-in V6 helper built beside the test target"]
fn copied_installed_v6_sdr_job_executes_and_adopts_real_source_with_consent() {
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
    let bytes = fs::read(&helper).expect("build real helper with --features avif-sdr-codec first");
    let record = native_avif_sdr_build_record(env!("CARGO_PKG_VERSION")).unwrap();
    assert_eq!(
        bytes
            .windows(record.len())
            .filter(|value| *value == record)
            .count(),
        1,
        "actual V6 build required"
    );
    let root = std::env::temp_dir().join(format!("paintcraft-installed-sdr-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let application = root.join(current.file_name().unwrap());
    fs::copy(&current, &application).unwrap();
    let installed_helper = root.join(helper_name);
    fs::copy(&helper, &installed_helper).unwrap();
    fs::write(
        root.join(codec_helper::MANIFEST_NAME),
        json::canonical(&metadata(
            &installed_helper,
            AVIF_SDR_NATIVE_PROFILE,
            QUALIFIED_AVIF_SDR_IMPORT_VARIANTS,
        )),
    )
    .unwrap();
    let output = Command::new(&application)
        .args(["--ignored", "--exact", INNER, "--nocapture"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "actual installed fixture failed: {} / {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
    fs::remove_dir_all(root).unwrap();
}
#[derive(Clone, Copy)]
enum During {
    None,
    CancelSupervision,
    SourceAfterChild,
    OwnerAfterChild,
}
fn prepare_actual(
    input: &Path,
    codec: Codec,
    owner: Owner,
    epoch: Arc<AtomicU64>,
    resident: SharedBudget,
    transient: SharedBudget,
    during: During,
) -> Result<PreparedImport> {
    let identity = SourceIdentity::from_file(fs::File::open(input).unwrap())?;
    let length = identity.file().metadata().unwrap().len();
    let reservation = reservation_bytes_for_input(length)? + (82 << 20);
    let mut executor = Executor::with_budget(
        Limits {
            workers: 1,
            pending_jobs: 1,
            transient_bytes: 512 << 20,
        },
        transient,
    )?;
    let path = input.to_owned();
    let handle = executor.submit(
        reservation,
        move |cancel| {
            let lease = cancel.resource_lease().unwrap();
            let check = || cancel.check();
            let current = || {
                Ok(Owner {
                    epoch: epoch.load(Ordering::SeqCst),
                    ..owner
                })
            };
            Job {
                input: path.clone(),
                identity,
                length,
                codec,
                owner,
                pixels: PIXELS_MAX,
                deadline: Duration::from_secs(10),
                application_version: env!("CARGO_PKG_VERSION").into(),
                expected_sha256: None,
            }
            .prepare_import(resident, lease, &check, &current, |launch, check| {
                let checks = AtomicUsize::new(0);
                let live_check = || {
                    check()?;
                    if matches!(during, During::CancelSupervision)
                        && checks.fetch_add(1, Ordering::SeqCst) > 0
                    {
                        return Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "cancel during actual supervision",
                        ));
                    }
                    Ok(())
                };
                let output = process::run_parent_lease(
                    launch.executable,
                    launch.arguments,
                    launch.request.segments(),
                    process::Limits {
                        input_bytes: INPUT_BYTES_MAX + REQUEST_HEADER,
                        output_bytes: OUTPUT_BYTES_MAX,
                        deadline: launch.remaining,
                        resource_exit_code: Some(RESOURCE_EXIT),
                    },
                    &live_check,
                )?;
                assert!(output.status.success());
                match during {
                    During::SourceAfterChild => fs::write(&path, b"changed source").unwrap(),
                    During::OwnerAfterChild => {
                        epoch.fetch_add(1, Ordering::SeqCst);
                    }
                    _ => {}
                }
                Ok(output.bytes)
            })
        },
        || {},
    )?;
    let start = Instant::now();
    let result = loop {
        if let Some(result) = executor.poll_for(handle) {
            break result;
        }
        assert!(start.elapsed() < Duration::from_secs(15));
        std::thread::sleep(Duration::from_millis(2));
    };
    drop(executor);
    result
}
fn pixels(document: &Document, expected: &[u8]) {
    let (width, height) = document.canvas;
    assert_eq!(expected.len(), width as usize * height as usize * 4);
    let layer = document.layers_in_order().next().unwrap();
    for (index, value) in expected.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            layer
                .raster
                .get(
                    (index % width as usize) as i64,
                    (index / width as usize) as i64
                )
                .0,
            *value
        );
    }
}
#[test]
#[ignore = "only invoked from the copied installed V6 qualifier"]
fn installed_v6_sdr_job_fixture() {
    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    let source = root.join("source.avif");
    let bytes = fixture("named-both.avif");
    fs::write(&source, &bytes).unwrap();
    let resident = SharedBudget::new(8 << 20);
    let transient = SharedBudget::new(512 << 20);
    let mut admission = Admission::with_residency(PIXELS_MAX, resident.clone());
    let previous = paintcraft_document::import_image_with_admission_checked(
        2,
        2,
        &[42; 16],
        "previous",
        &mut admission,
        &|| Ok(()),
    )
    .unwrap();
    drop(admission);
    let mut session = Session::new(previous);
    session.mark_unsaved();
    let prior = session.ticket();
    let history = session.history_ticket();
    let owner = Owner {
        session: history.owner,
        state: history.base.state_id,
        epoch: history.base.publication_epoch,
        frontend: 3,
    };
    let epoch = Arc::new(AtomicU64::new(owner.epoch));
    let baseline = resident.usage().reserved;
    let prepare = |during| {
        prepare_actual(
            &source,
            Codec::AvifSdr8,
            owner,
            epoch.clone(),
            resident.clone(),
            transient.clone(),
            during,
        )
    };
    for allowed in [
        vec![],
        vec!["unknown".to_owned()],
        vec![COLOR.to_owned()],
        vec![NAMES.to_owned()],
    ] {
        let pending = prepare(During::None).unwrap();
        assert_eq!(pending.decoder_profile(), NATIVE_AVIF_SDR_IMPORT_VARIANT);
        assert!(pending.avif_assessment().is_none());
        let assessment = pending.avif_sdr_assessment().unwrap();
        assert_eq!(assessment.sha256(), Sha256::digest(&bytes).as_slice());
        assert_eq!(assessment.primary_name(), "Color image");
        assert_eq!(assessment.alpha_name(), Some("Alpha mask"));
        assert_eq!(assessment.primary_sequence().chroma_position, 1);
        assert!(!assessment.primary_sequence().full_range);
        assert_eq!(
            transient.usage().reserved,
            reservation_bytes_for_input(bytes.len() as u64).unwrap() + (82 << 20)
        );
        assert!(resident.usage().reserved > baseline);
        let error = pending
            .adopt_with_losses(&allowed, &|| Ok(()), &|| Ok(owner))
            .unwrap_err();
        assert_eq!(
            error.code,
            if allowed.first().is_some_and(|value| value == "unknown") {
                ErrorCode::InputSchema
            } else {
                ErrorCode::CapabilityUnsupported
            }
        );
        assert_eq!(session.ticket(), prior);
        assert!(session.is_dirty());
        assert_eq!(resident.usage().reserved, baseline);
        assert_eq!(transient.usage().reserved, 0);
    }
    let allowed = vec![COLOR.to_owned(), NAMES.to_owned()];
    for failure in 0..3 {
        let pending = prepare(During::None).unwrap();
        match failure {
            0 => fs::write(&source, b"changed before adoption").unwrap(),
            1 => {
                epoch.fetch_add(1, Ordering::SeqCst);
            }
            _ => {}
        }
        let result = pending.adopt_with_losses(
            &allowed,
            &|| {
                if failure == 2 {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel adoption"))
                } else {
                    Ok(())
                }
            },
            &|| {
                Ok(Owner {
                    epoch: epoch.load(Ordering::SeqCst),
                    ..owner
                })
            },
        );
        assert_eq!(
            result.unwrap_err().code,
            if failure == 2 {
                ErrorCode::OperationCancelled
            } else {
                ErrorCode::ConflictStaleBase
            }
        );
        fs::write(&source, &bytes).unwrap();
        epoch.store(owner.epoch, Ordering::SeqCst);
        assert_eq!(session.ticket(), prior);
        assert_eq!(resident.usage().reserved, baseline);
        assert_eq!(transient.usage().reserved, 0);
    }
    for (during, code) in [
        (During::CancelSupervision, ErrorCode::OperationCancelled),
        (During::SourceAfterChild, ErrorCode::ConflictStaleBase),
        (During::OwnerAfterChild, ErrorCode::ConflictStaleBase),
    ] {
        assert_eq!(prepare(during).unwrap_err().code, code);
        fs::write(&source, &bytes).unwrap();
        epoch.store(owner.epoch, Ordering::SeqCst);
        assert_eq!(session.ticket(), prior);
        assert_eq!(resident.usage().reserved, baseline);
        assert_eq!(transient.usage().reserved, 0);
    }
    for name in ["420-colocated-full-odd", "444-fullchroma-limited-odd-alpha"] {
        let path = root.join(format!("{name}.avif"));
        fs::write(&path, fixture(&format!("{name}.avif"))).unwrap();
        let pending = prepare_actual(
            &path,
            Codec::AvifSdr8,
            owner,
            epoch.clone(),
            resident.clone(),
            transient.clone(),
            During::None,
        )
        .unwrap();
        assert_eq!(pending.losses(), [COLOR]);
        let adopted = pending
            .adopt_with_losses(&[COLOR.to_owned()], &|| Ok(()), &|| Ok(owner))
            .unwrap();
        pixels(
            &adopted.document,
            &fixture(&format!("{name}.rational.rgba")),
        );
        drop(adopted);
        assert_eq!(resident.usage().reserved, baseline);
        assert_eq!(transient.usage().reserved, 0);
    }
    for (name, codec, original, expected) in [
        (
            "identity.avif",
            Codec::AvifSrgb8,
            "avif-fixtures/alpha.avif",
            "avif-fixtures/alpha.libaom.rgba",
        ),
        ("prior.jxl", Codec::JxlRgba8, "jxl-fixtures/rgba8.jxl", ""),
    ] {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
        let path = root.join(name);
        fs::write(&path, fs::read(fixtures.join(original)).unwrap()).unwrap();
        let expected = if expected.is_empty() {
            vec![
                10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150,
                254, 160, 170, 180, 255,
            ]
        } else {
            fs::read(fixtures.join(expected)).unwrap()
        };
        let pending = prepare_actual(
            &path,
            codec,
            owner,
            epoch.clone(),
            resident.clone(),
            transient.clone(),
            During::None,
        )
        .unwrap();
        assert!(pending.losses().is_empty());
        assert!(pending.avif_sdr_assessment().is_none());
        let adopted = pending.adopt(&|| Ok(()), &|| Ok(owner)).unwrap();
        pixels(&adopted.document, &expected);
        drop(adopted);
        assert_eq!(resident.usage().reserved, baseline);
        assert_eq!(transient.usage().reserved, 0);
    }
    let helper = root.join(if cfg!(windows) {
        "paintcraft-codec-worker.exe"
    } else {
        "paintcraft-codec-worker"
    });
    let helper_bytes = fs::read(&helper).unwrap();
    let manifest_path = root.join(codec_helper::MANIFEST_NAME);
    let manifest_bytes = fs::read(&manifest_path).unwrap();
    let mut changed = helper_bytes.clone();
    changed.extend_from_slice(b"changed helper");
    fs::write(&helper, changed).unwrap();
    assert!(
        prepare(During::None)
            .unwrap_err()
            .message
            .contains("SHA-256")
    );
    fs::write(&helper, &helper_bytes).unwrap();
    fs::write(
        &manifest_path,
        json::canonical(&metadata(
            &helper,
            AVIF_NATIVE_PROFILE,
            QUALIFIED_AVIF_IMPORT_VARIANTS,
        )),
    )
    .unwrap();
    assert!(prepare(During::None).is_err());
    fs::write(&manifest_path, manifest_bytes).unwrap();
    let hidden = root.join("hidden-helper");
    fs::rename(&helper, &hidden).unwrap();
    assert!(prepare(During::None).is_err());
    fs::rename(&hidden, &helper).unwrap();
    let denied = SharedBudget::new(1);
    assert_eq!(
        prepare_actual(
            &source,
            Codec::AvifSdr8,
            owner,
            epoch.clone(),
            resident.clone(),
            denied.clone(),
            During::None
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(denied.usage().reserved, 0);
    assert_eq!(session.ticket(), prior);
    assert!(session.is_dirty());
    assert_eq!(resident.usage().reserved, baseline);
    assert_eq!(transient.usage().reserved, 0);
    let pending = prepare(During::None).unwrap();
    let adopted = pending
        .adopt_with_losses(&allowed, &|| Ok(()), &|| Ok(owner))
        .unwrap();
    assert!(adopted.avif_assessment.is_none());
    assert_eq!(
        adopted.avif_sdr_assessment.as_ref().unwrap().losses(),
        [COLOR, NAMES]
    );
    let expected = fixture("420-vertical-limited-odd-alpha.rational.rgba");
    pixels(&adopted.document, &expected);
    assert_eq!(transient.usage().reserved, 0);
    let identity = adopted.identity;
    session = Session::new(adopted.document);
    session.mark_unsaved();
    assert!(session.is_dirty());
    let project = root.join("converted.pcnd");
    let outcome = crate::save::save_with_preparation(
        &project,
        false,
        crate::save::STAGED_FILE_BYTES_MAX,
        |sink| crate::pcnd::write_compact_to_checked(session.document(), sink, &|| Ok(())),
        |_| Ok(()),
    );
    assert_eq!(outcome.replacement, crate::save::Replacement::Replaced);
    session.mark_saved();
    assert!(!session.is_dirty());
    pixels(
        &crate::pcnd::read(&fs::read(&project).unwrap()).unwrap(),
        &expected,
    );
    let renamed = root.join("renamed.avif");
    fs::rename(&source, &renamed).unwrap();
    let alias = root.join("source-alias.pcnd");
    fs::hard_link(&renamed, &alias).unwrap();
    let outcome = crate::save::save_with_preparation(
        &alias,
        true,
        crate::save::STAGED_FILE_BYTES_MAX,
        |sink| crate::pcnd::write_compact_to_checked(session.document(), sink, &|| Ok(())),
        |_| {
            if identity.matches_path(&alias)? {
                Err(Error::new(
                    ErrorCode::InputSchema,
                    "cannot overwrite captured source",
                ))
            } else {
                Ok(())
            }
        },
    );
    assert_ne!(outcome.replacement, crate::save::Replacement::Replaced);
    assert_eq!(fs::read(&alias).unwrap(), bytes);
    assert!(!session.is_dirty());
    let snapshot = session.snapshot();
    drop(session);
    assert!(resident.usage().reserved > 0);
    drop(snapshot);
    assert_eq!(resident.usage().reserved, 0);
    println!(
        "actual V6 Job: consent, source/owner/cancel, retained leases, exact PCND, dirty/save and renamed source guards passed"
    );
}
