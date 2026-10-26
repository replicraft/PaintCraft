//! Explicit installed-helper qualification. These tests never create an OS window.
use super::tests::{INPUT, app as make_app, draw, retire};
use super::*;
use sha2::{Digest, Sha256};
use std::{fs, process::Command, time::Instant};
#[test]
#[ignore = "requires a built real native helper; stages a copied headless test executable"]
fn copied_installed_helper_adopts_exact_pixels_and_refuses_stale_owners() {
    use paintcraft_core::types::json::{self, Value};
    let root = std::env::temp_dir().join(format!("paintcraft-jxl-installed-{}", Uuid::new_v4()));
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
    assert!(helper.exists(), "Build paintcraft-codec-worker first");
    let child = root.join(current.file_name().unwrap());
    fs::copy(current, &child).unwrap();
    let installed = root.join(helper_name);
    fs::copy(helper, &installed).unwrap();
    let target = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => panic!("release target required"),
    };
    let manifest = Value::object([
        ("schema_version", Value::Int(2)),
        ("embedded_build_record_verified", Value::Bool(true)),
        ("application_version", Value::str(env!("CARGO_PKG_VERSION"))),
        ("target", Value::str(target)),
        ("executable", Value::str(helper_name)),
        (
            "sha256",
            Value::str(hex(&Sha256::digest(fs::read(installed).unwrap()))),
        ),
        (
            "protocol_version",
            Value::Int(wire::PROTOCOL_VERSION.into()),
        ),
        ("profile", Value::str(wire::NATIVE_PROFILE)),
        (
            "requested_heap_ceiling_bytes",
            Value::Int(wire::HEAP_BYTES as i64),
        ),
        (
            "qualified_import_variants",
            Value::Array(
                wire::QUALIFIED_NATIVE_IMPORT_VARIANTS
                    .iter()
                    .copied()
                    .map(Value::str)
                    .collect(),
            ),
        ),
    ]);
    fs::write(
        root.join("paintcraft-codec-worker.json"),
        json::canonical(&manifest),
    )
    .unwrap();
    for test in [
        "importing::native_codec::installed_tests::fixture_real_installed_jxl",
        "importing::native_codec::installed_tests::fixture_running_helper_cancel",
        "importing::native_codec::jxr_installed_tests::fixture_real_installed_jxr_review",
        "importing::native_codec::heif_installed_tests::fixture_real_installed_heif_review",
    ] {
        let output = Command::new(&child)
            .args(["--exact", test, "--ignored", "--nocapture"])
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("test result: ok. 1 passed"),
            "Expected one actual fixture for {test}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::remove_dir_all(root).unwrap();
}
fn prepared(app: &mut App) {
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        let pending = app.pending_import.as_mut().expect("pending import");
        let Candidate::NativeCodec(source) = &mut pending.candidate else {
            panic!("JXL candidate")
        };
        if let Some(result) = source.job.as_ref().and_then(|job| job.poll()) {
            source.job = None;
            source.prepared = Some(result.unwrap());
            return;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
#[ignore = "private copied installation fixture"]
fn fixture_real_installed_jxl() {
    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let input = root.join("source.jxl");
    fs::write(&input, INPUT).unwrap();
    let mut app = make_app();
    let before = app.editor.document_snapshot();
    let resident = app.editing.as_ref().unwrap().import_residency();
    let baseline = resident.usage().reserved;
    app.import_selected(input.clone());
    assert!(app.pending_import.is_some(), "{}", app.editor.status());
    prepared(&mut app);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert!(app.worker_budget.usage().reserved > 0);
    assert!(resident.usage().reserved > baseline);
    let pending_alias = root.join("pending-source.pcnd");
    fs::hard_link(&input, &pending_alias).unwrap();
    app.save_selected(&app.context.clone(), pending_alias.clone());
    assert_eq!(fs::read(&pending_alias).unwrap(), INPUT);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    draw(&mut app);
    assert!(app.pending_import.is_none(), "{}", app.editor.status());
    assert_eq!(
        app.editor.document().canvas,
        (3, 2),
        "{}",
        app.editor.status()
    );
    assert_eq!(app.worker_budget.usage().reserved, 0);
    assert!(app.editor.is_dirty());
    assert!(app.editor.path().is_none());
    assert!(app.bound.is_none());
    assert_eq!(app.auxiliary_sources.len(), 1);
    let expected = [
        [10, 20, 30, 0],
        [40, 50, 60, 1],
        [70, 80, 90, 127],
        [100, 110, 120, 128],
        [130, 140, 150, 254],
        [160, 170, 180, 255],
    ];
    let doc = app.editor.document();
    let layer = &doc.layers[&doc.roots[0]];
    for (i, rgba) in expected.iter().enumerate() {
        assert_eq!(layer.raster.get((i % 3) as i64, (i / 3) as i64).0, *rgba);
    }
    let ctx = app.context.clone();
    let project = root.join("painting.pcnd");
    app.save_selected(&ctx, project.clone());
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        app.poll_edits(&ctx);
        app.finish_save();
        if project.is_file() && !app.any_pending_edits() && !app.any_deferred_files() {
            break;
        }
        assert!(Instant::now() < until, "{}", app.editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(project.is_file());
    let moved = root.join("renamed.jxl");
    fs::rename(&input, &moved).unwrap();
    let retained = app.editor.document_snapshot();
    for location in 0..3 {
        match location {
            1 => app
                .add_document_tab(Editor::new_document(2, 2).unwrap(), &ctx)
                .unwrap(),
            2 => {
                let mut parked = tabs::WorkspaceDocuments::new((2, 2), false);
                parked.swap(&mut app);
                app.other_workspace = Some(Box::new(parked));
                app.settings.experience = Experience::Classic;
            }
            _ => {}
        }
        let alias = root.join(format!("source-{location}.png"));
        fs::hard_link(&moved, &alias).unwrap();
        app.do_export(&alias, ImageFormat::Png, &KNOWN_LOSSES.map(str::to_owned));
        assert_eq!(fs::read(alias).unwrap(), INPUT);
        let alias = root.join(format!("source-{location}.pcnd"));
        fs::hard_link(&moved, &alias).unwrap();
        app.save_selected(&ctx, alias.clone());
        assert_eq!(fs::read(alias).unwrap(), INPUT);
    }
    drop(app);
    assert!(resident.usage().reserved > baseline);
    drop(retained);
    assert_eq!(resident.usage().reserved, baseline);
    for change in 0..8 {
        let path = root.join(format!("stale-{change}.jxl"));
        fs::write(&path, INPUT).unwrap();
        let mut app = make_app();
        app.import_selected(path.clone());
        prepared(&mut app);
        let resident = app.editing.as_ref().unwrap().import_residency();
        assert!(app.worker_budget.usage().reserved > 0);
        match change {
            0 => {
                app.editor = Editor::with_document(app.editor.document().clone(), None);
            }
            1 => app.settings.experience = Experience::Classic,
            2 => app.editor.invalidate_saved_state(),
            3 => app.editor.begin_text(Uuid::nil(), (0, 0)),
            4 => {
                let mut changed = INPUT.to_vec();
                let last = changed.len() - 1;
                changed[last] ^= 1;
                fs::write(&path, changed).unwrap();
            }
            5 => app
                .add_document_tab(Editor::new_document(4, 4).unwrap(), &app.context.clone())
                .unwrap(),
            6 => {
                let mut parked = tabs::WorkspaceDocuments::new((4, 4), false);
                parked.swap(&mut app);
                app.other_workspace = Some(Box::new(parked));
                app.settings.experience = Experience::Classic;
            }
            _ => {
                app.pending_import = None;
                app.confirm = None;
            }
        }
        let current = app.editor.document_snapshot();
        if change != 7 {
            draw(&mut app);
        }
        assert!(app.pending_import.is_none());
        assert!(Arc::ptr_eq(&current, &app.editor.document_snapshot()));
        retire(&app);
        assert_eq!(resident.usage().reserved, baseline);
        assert!(app.auxiliary_sources.is_empty());
    }
}
#[test]
#[ignore = "private copied installation fixture"]
fn fixture_running_helper_cancel() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };
    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let input = root.join("cancel.jxl");
    fs::write(&input, INPUT).unwrap();
    let mut app = make_app();
    let before = app.editor.document_snapshot();
    let (started, wait) = mpsc::sync_channel(1);
    let probes = Arc::new(AtomicUsize::new(0));
    let counts = probes.clone();
    app.stage_native_jxl_with_launch(
        input,
        Box::new(move |launch, check| {
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
                    if counts.fetch_add(1, Ordering::AcqRel) == 1 {
                        started.send(()).unwrap();
                        while check().is_ok() {
                            std::thread::sleep(Duration::from_millis(1));
                        }
                    }
                    check()
                },
            )
            .map(|output| output.bytes)
        }),
    )
    .unwrap();
    wait.recv_timeout(Duration::from_secs(10)).unwrap();
    app.resolve_import(&app.context.clone(), false);
    retire(&app);
    assert!(probes.load(Ordering::Acquire) >= 2);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert!(app.pending_import.is_none());
    assert!(app.auxiliary_sources.is_empty());
}
