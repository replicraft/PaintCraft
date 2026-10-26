//! Actual installed V5 helper and headless Studio controls, without OS windows.
use super::tests::{app as make_app, draw, retire};
use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, translate};
use std::{fs, time::Instant};
#[test]
#[ignore = "requires a built AVIF-enabled native helper; stages a copied headless test executable"]
fn copied_avif_helper_runs_english_japanese_review_and_failure_guards() {
    use paintcraft_core::types::json;
    let root = std::env::temp_dir().join(format!("paintcraft-avif-installed-{}", Uuid::new_v4()));
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
        helper.is_file(),
        "Build paintcraft-codec-worker --features avif-codec first"
    );
    let executable = root.join(current.file_name().unwrap());
    fs::copy(&current, &executable).unwrap();
    let installed = root.join(helper_name);
    fs::copy(helper, &installed).unwrap();
    let target = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "aarch64") if cfg!(target_env = "gnu") => "aarch64-unknown-linux-gnu",
        ("linux", "x86_64") if cfg!(target_env = "gnu") => "x86_64-unknown-linux-gnu",
        ("windows", "aarch64") if cfg!(target_env = "msvc") => "aarch64-pc-windows-msvc",
        ("windows", "x86_64") if cfg!(target_env = "msvc") => "x86_64-pc-windows-msvc",
        _ => panic!("supported release target required"),
    };
    let manifest = super::fixture_manifest::manifest(
        &installed,
        target,
        Some(wire::NATIVE_AVIF_IMPORT_VARIANT),
    );
    fs::write(
        root.join("paintcraft-codec-worker.json"),
        json::canonical(&manifest),
    )
    .unwrap();
    let output = std::process::Command::new(&executable)
        .args([
            "--exact",
            "importing::native_codec::avif_installed_tests::fixture_real_installed_avif_review",
            "--ignored",
            "--nocapture",
        ])
        .output()
        .unwrap();
    if let Some(base) = std::env::var_os("PAINTCRAFT_AVIF_UI_EVIDENCE") {
        let directory = PathBuf::from(base).join(format!("attempt-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("stdout.log"), &output.stdout).unwrap();
        fs::write(directory.join("stderr.log"), &output.stderr).unwrap();
        fs::write(directory.join("manifest.json"), json::canonical(&manifest)).unwrap();
        println!("AVIF installed UI evidence: {}", directory.display());
    }
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("test result: ok. 1 passed"),
        "Expected one real installed fixture: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../../../../crates/services/paintcraft-formats/tests/avif-fixtures/",
            $name
        ))
    };
}
const PRIOR: [u8; 16] = [
    19, 88, 201, 255, 99, 3, 17, 0, 34, 55, 89, 127, 8, 13, 21, 254,
];
fn prior_app(language: Language) -> App {
    let mut app = make_app();
    app.settings.language = language;
    app.install(Editor::with_document(
        paintcraft_document::import_image(2, 2, &PRIOR, &"0".repeat(64)).unwrap(),
        None,
    ))
    .unwrap();
    app
}
fn pixels(app: &App, width: u32, height: u32, expected: &[u8]) {
    document_pixels(app.editor.document(), width, height, expected);
}
fn document_pixels(doc: &paintcraft_document::Document, width: u32, height: u32, expected: &[u8]) {
    assert_eq!(doc.canvas, (width, height));
    assert_eq!(doc.roots.len(), 1);
    assert_eq!(expected.len(), width as usize * height as usize * 4);
    let layer = &doc.layers[&doc.roots[0]];
    for (i, rgba) in expected.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            layer
                .raster
                .get((i % width as usize) as i64, (i / width as usize) as i64)
                .0,
            *rgba
        );
    }
}
#[test]
fn avif_generic_open_classic_drafts_and_admission_cannot_publish() {
    let directory = std::env::temp_dir().join(format!("paintcraft-avif-guards-{}", Uuid::new_v4()));
    fs::create_dir(&directory).unwrap();
    let input = directory.join("source.avif");
    fs::write(&input, fixture!("named-alpha.avif")).unwrap();
    let mut app = prior_app(Language::English);
    let before = app.editor.document_snapshot();
    assert!(app.open_path(&input).is_err());
    app.settings.experience = Experience::Classic;
    assert!(app.stage_native_avif(input.clone()).is_err());
    assert!(app.open_path(&input).is_err());
    assert!(
        !app.format_filters(paintcraft_platform::dialogs::Kind::Import)
            .iter()
            .any(|filter| filter
                .extensions
                .iter()
                .any(|extension| extension == "avif"))
    );
    app.settings.experience = Experience::Studio;
    assert!(
        app.format_filters(paintcraft_platform::dialogs::Kind::Import)
            .iter()
            .any(|filter| filter
                .extensions
                .iter()
                .any(|extension| extension == "avif"))
    );
    assert!(
        !app.format_filters(paintcraft_platform::dialogs::Kind::Open)
            .iter()
            .any(|filter| filter
                .extensions
                .iter()
                .any(|extension| extension == "avif"))
    );
    app.editor.begin_text(Uuid::nil(), (0, 0));
    assert_eq!(
        app.stage_native_avif(directory.join("absent.avif"))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    app.editor.cancel_text();
    let lease = app
        .worker_budget
        .reserve(app.worker_budget.usage().limit)
        .unwrap();
    assert_eq!(
        app.stage_native_avif(input).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    drop(lease);
    assert!(app.pending_import.is_none());
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    pixels(&app, 2, 2, &PRIOR);
    assert_eq!(app.worker_budget.usage().reserved, 0);
    fs::remove_dir_all(directory).unwrap();
}
#[test]
#[ignore = "private copied AVIF-enabled installation fixture"]
fn fixture_real_installed_avif_review() {
    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    for (case, language) in (0..4)
        .flat_map(|case| [Language::English, Language::Japanese].map(|language| (case, language)))
    {
        let input = root.join(format!("avif-review-{case}-{language:?}.avif"));
        fs::write(&input, fixture!("named-alpha.avif")).unwrap();
        let mut app = prior_app(language);
        let before = app.editor.document_snapshot();
        let metadata = app.editing.as_ref().unwrap().metadata_budget();
        let resident = app.editing.as_ref().unwrap().import_residency();
        let resident_before = resident.usage().reserved;
        let baseline = metadata.usage().reserved;
        app.import_selected(input.clone());
        assert!(app.pending_import.is_some(), "{}", app.editor.status());
        let mut h = Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
        let until = Instant::now() + Duration::from_secs(15);
        loop {
            h.step();
            let pending = h
                .state()
                .pending_import
                .as_ref()
                .unwrap_or_else(|| panic!("Review vanished: {}", h.state().editor.status()));
            let Candidate::NativeCodec(source) = &pending.candidate else {
                panic!("native codec")
            };
            if source.review.is_some() {
                break;
            }
            assert!(Instant::now() < until, "{}", h.state().editor.status());
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        pixels(h.state(), 2, 2, &PRIOR);
        assert!(metadata.usage().reserved > baseline);
        h.run();
        h.run();
        h.get_by_label(&translate(language, "Image name"));
        h.get_by_label("Color image");
        h.get_by_label(&translate(language, "Transparency name"));
        h.get_by_label("Alpha mask");
        h.get_by_label(&translate(
            language,
            "AVIF image item names are not retained in the new document.",
        ));
        assert!(
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                .accesskit_node()
                .is_disabled()
        );
        if case == 1 {
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
                .click();
            h.run();
        } else {
            h.get_by_role_and_label(
                egui::Role::CheckBox,
                &translate(language, "Accept import limitations"),
            )
            .click();
            h.run();
            h.run();
            assert!(
                !h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                    .accesskit_node()
                    .is_disabled()
            );
            if case == 2 {
                fs::write(&input, b"changed after preview").unwrap();
            }
            if case == 3 {
                h.state_mut().editor.begin_text(Uuid::nil(), (0, 0));
                h.run();
            } else {
                h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                    .click();
                h.run();
                h.run();
            }
        }
        assert!(
            h.state().pending_import.is_none(),
            "{}",
            h.state().editor.status()
        );
        assert_eq!(metadata.usage().reserved, baseline);
        if case != 0 {
            assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
            pixels(h.state(), 2, 2, &PRIOR);
            assert!(h.state().auxiliary_sources.is_empty());
            assert_eq!(resident.usage().reserved, resident_before);
        } else {
            pixels(h.state(), 16, 16, fixture!("alpha.libaom.rgba"));
            assert!(h.state().editor.is_dirty());
            assert!(h.state().editor.path().is_none());
            assert!(h.state().bound.is_none());
            assert_eq!(h.state().auxiliary_sources.len(), 1);
            let ctx = h.state().context.clone();
            let project = root.join(format!("avif-project-{language:?}.pcnd"));
            h.state_mut().save_selected(&ctx, project.clone());
            let until = Instant::now() + Duration::from_secs(15);
            loop {
                h.state_mut().poll_edits(&ctx);
                h.state_mut().finish_save();
                if project.is_file()
                    && !h.state().any_pending_edits()
                    && !h.state().any_deferred_files()
                {
                    break;
                }
                assert!(Instant::now() < until, "{}", h.state().editor.status());
                std::thread::sleep(Duration::from_millis(1));
            }
            let reopened = paintcraft_formats::pcnd::read_with_limits(
                &fs::read(project).unwrap(),
                256,
                paintcraft_core::types::policy::STAGED_PIXELS_MAX,
            )
            .unwrap();
            document_pixels(&reopened, 16, 16, fixture!("alpha.libaom.rgba"));
            let moved = root.join(format!("avif-moved-{language:?}.avif"));
            fs::rename(&input, &moved).unwrap();
            let alias = root.join(format!("avif-alias-{language:?}.pcnd"));
            fs::hard_link(&moved, &alias).unwrap();
            h.state_mut().save_selected(&ctx, alias.clone());
            assert_eq!(fs::read(alias).unwrap(), fixture!("named-alpha.avif"));
        }
        retire(h.state());
        println!(
            "AVIF {language:?} named review case{case}: exact pixels and publication guards passed"
        );
    }
    for language in [Language::English, Language::Japanese] {
        for (name, reference) in [
            ("opaque", fixture!("opaque.libaom.rgba").as_slice()),
            ("alpha", fixture!("alpha.libaom.rgba").as_slice()),
        ] {
            let input = root.join(format!("avif-anonymous-{name}-{language:?}.avif"));
            fs::write(
                &input,
                if name == "opaque" {
                    fixture!("opaque.avif").as_slice()
                } else {
                    fixture!("alpha.avif").as_slice()
                },
            )
            .unwrap();
            let mut app = prior_app(language);
            app.import_selected(input);
            let until = Instant::now() + Duration::from_secs(15);
            while app.pending_import.is_some() {
                draw(&mut app);
                assert!(Instant::now() < until, "{}", app.editor.status());
                std::thread::sleep(Duration::from_millis(1));
            }
            pixels(&app, 16, 16, reference);
            assert!(app.editor.is_dirty());
            assert_eq!(app.auxiliary_sources.len(), 1);
            retire(&app);
            println!(
                "AVIF {language:?} unnamed {name}: exact independent pixels, automatic no-loss adoption"
            );
        }
    }
}
