//! Headless controls against an immutable copied V7 installation, no GUI windows.
use super::super::tests::{app as make_app, retire};
use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, translate};
use std::{fs, time::Instant};
const PRIOR: [u8; 16] = [
    19, 88, 201, 255, 99, 3, 17, 0, 34, 55, 89, 127, 8, 13, 21, 254,
];
fn fixture(group: &str, file: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/services/paintcraft-formats/tests")
            .join(group)
            .join(file),
    )
    .unwrap()
}
fn app(language: Language) -> App {
    let mut app = make_app();
    app.settings.language = language;
    app.install(Editor::with_document(
        paintcraft_document::import_image(2, 2, &PRIOR, &"0".repeat(64)).unwrap(),
        None,
    ))
    .unwrap();
    app
}
fn pixels(doc: &paintcraft_document::Document, expected: &[u8]) {
    let (width, height) = doc.canvas;
    assert_eq!(expected.len(), width as usize * height as usize * 4);
    assert_eq!(doc.roots.len(), 1);
    let raster = &doc.layers[&doc.roots[0]].raster;
    for (i, p) in expected.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            raster
                .get((i % width as usize) as i64, (i / width as usize) as i64)
                .0,
            *p,
            "pixel{i}"
        );
    }
}
fn source<'a>(h: &'a Harness<'_, App>) -> &'a Source {
    let Candidate::NativeCodec(source) = &h
        .state()
        .pending_import
        .as_ref()
        .expect("pending import")
        .candidate
    else {
        panic!()
    };
    source
}
fn button(h: &mut Harness<App>, language: Language, label: &str) {
    h.get_by_role_and_label(egui::Role::Button, &translate(language, label))
        .click();
    h.step();
    h.step();
}
fn wait(h: &mut Harness<App>, expected_error: bool) {
    let end = Instant::now() + Duration::from_secs(35);
    loop {
        h.step();
        let value = source(h);
        if value.job.is_none() {
            assert_eq!(
                value.float.as_ref().unwrap().error.is_some(),
                expected_error,
                "{}",
                h.state().editor.status()
            );
            assert_eq!(value.review.is_some(), !expected_error);
            break;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    h.run();
    h.run();
}
fn consent(h: &mut Harness<App>, language: Language) {
    assert!(
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .accesskit_node()
            .is_disabled()
    );
    for label in ["Accept import limitations", "Open image", "Cancel"] {
        let text = translate(language, label);
        let node = h.get_by_label(&text);
        println!("consent-target {label}: {:?}", node.rect());
    }
    h.get_by_role_and_label(
        egui::Role::CheckBox,
        &translate(language, "Accept import limitations"),
    )
    .click();
    for frame in 0..3 {
        h.step();
        println!(
            "consent-frame {frame}: pending={}, status={}",
            h.state().pending_import.is_some(),
            h.state().editor.status()
        );
        for label in ["Accept import limitations", "Open image", "Cancel"] {
            if let Some(node) = h.query_by_label(&translate(language, label)) {
                println!("consent-frame {frame} {label}: {:?}", node.rect());
            }
        }
    }
    assert!(
        h.state().pending_import.is_some(),
        "consent click dismissed import: {}",
        h.state().editor.status()
    );
    assert!(
        !h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .accesskit_node()
            .is_disabled()
    );
}
#[test]
#[ignore = "requires copied immutable V7 helper and matching manifest beside this test binary"]
fn fixture_real_installed_float_settings_preview_and_adoption() {
    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    for language in [Language::English, Language::Japanese] {
        for (case, group, name, codec, golden) in [
            (
                0,
                "radiance-fixtures",
                "literal-2x2.hdr",
                wire::Codec::HdrSdr8,
                "hdr-clip.rgba",
            ),
            (
                1,
                "exr-fixtures",
                "unlabelled.exr",
                wire::Codec::ExrSdr8,
                "exr-explicit-source.rgba",
            ),
            (
                2,
                "exr-fixtures",
                "half-rgba-rle.exr",
                wire::Codec::ExrSdr8,
                "exr-alpha-explicit-flatten.rgba",
            ),
        ] {
            let input = root.join(format!(
                "float-{case}-{language:?}.{}",
                if codec == wire::Codec::HdrSdr8 {
                    "hdr"
                } else {
                    "exr"
                }
            ));
            let original = fixture(group, name);
            fs::write(&input, &original).unwrap();
            let mut app = app(language);
            let before = app.editor.document_snapshot();
            app.import_selected(input.clone());
            assert!(app.pending_import.is_some(), "{}", app.editor.status());
            let mut h = Harness::builder()
                .with_size(egui::vec2(1400., 1400.))
                .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
            h.run();
            if case == 1 {
                button(&mut h, language, "Prepare preview");
                wait(&mut h, true);
                assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
                h.get_by_label(&translate(
                    language,
                    "Choose the image's source color space before conversion",
                ));
                h.get_by_role_and_label(
                    egui::Role::ComboBox,
                    &translate(language, "Source color space"),
                )
                .click();
                h.run();
                h.get_by_label(&translate(language, "Linear Rec.709 / D65"))
                    .click();
                h.run();
                button(&mut h, language, "Prepare preview");
                wait(&mut h, true);
                h.get_by_label(&translate(language, "This image needs an explicit negative-value policy. Choose clipping to continue."));
            }
            // Change the actual controls; the source/settings binding must follow.
            h.get_by_role_and_label(
                egui::Role::ComboBox,
                &translate(language, "Negative samples"),
            )
            .click();
            h.run();
            h.get_by_label(&translate(language, "Clip negative samples to black"))
                .click();
            h.run();
            if case == 2 {
                button(&mut h, language, "Prepare preview");
                wait(&mut h, true);
                h.get_by_label(&translate(language, "This image contains additive transparent color. Choose a background to flatten it."));
                h.get_by_role_and_label(
                    egui::Role::CheckBox,
                    &translate(language, "Flatten transparency onto a color"),
                )
                .click();
                h.run();
            }
            button(&mut h, language, "Prepare preview");
            wait(&mut h, false);
            assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
            pixels(h.state().editor.document(), &PRIOR);
            consent(&mut h, language);
            if case == 0 {
                // A settings edit retires both preview and already granted consent.
                h.get_by_role_and_label(egui::Role::ComboBox, &translate(language, "Tone mapping"))
                    .click();
                h.run();
                h.get_by_label(&translate(language, "Reinhard per channel"))
                    .click();
                h.run();
                assert!(source(&h).prepared.is_none() && source(&h).review.is_none());
                button(&mut h, language, "Prepare preview");
                wait(&mut h, false);
                consent(&mut h, language);
            }
            button(&mut h, language, "Open image");
            assert!(h.state().pending_import.is_none());
            let golden = if case == 0 {
                "hdr-reinhard.rgba"
            } else {
                golden
            };
            let expected = fixture("float-import-fixtures", golden);
            pixels(h.state().editor.document(), &expected);
            assert!(
                h.state().editor.is_dirty()
                    && h.state().editor.path().is_none()
                    && h.state().bound.is_none()
            );
            let project = root.join(format!("float-result-{case}-{language:?}.pcnd"));
            let ctx = h.state().context.clone();
            h.state_mut().save_selected(&ctx, project.clone());
            let end = Instant::now() + Duration::from_secs(15);
            while !project.is_file()
                || h.state().any_pending_edits()
                || h.state().any_deferred_files()
            {
                h.state_mut().poll_edits(&ctx);
                h.state_mut().finish_save();
                assert!(Instant::now() < end);
                std::thread::sleep(Duration::from_millis(1));
            }
            pixels(
                &paintcraft_formats::pcnd::read_with_limits(
                    &fs::read(project).unwrap(),
                    256,
                    wire::float::PIXELS_MAX,
                )
                .unwrap(),
                &expected,
            );
            let moved = input.with_file_name(format!("renamed-{case}-{language:?}.source"));
            fs::rename(&input, &moved).unwrap();
            let alias = input.with_file_name(format!("alias-{case}-{language:?}.source"));
            fs::hard_link(&moved, &alias).unwrap();
            h.state_mut().save_selected(&ctx, alias.clone());
            assert_eq!(fs::read(alias).unwrap(), original);
            retire(h.state());
            println!(
                "Float {language:?} case{case}: actual controls, exact independent pixels, PCND/source identity passed"
            );
        }
    }
}

#[test]
#[ignore = "requires copied immutable V7 helper and matching manifest beside this test binary"]
fn fixture_real_installed_float_preserves_nonempty_project_on_cancel_stale_and_source_change() {
    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    for language in [Language::English, Language::Japanese] {
        for case in 0..6 {
            let input = root.join(format!("float-guard-{case}-{language:?}.hdr"));
            fs::write(&input, fixture("radiance-fixtures", "literal-2x2.hdr")).unwrap();
            let mut app = app(language);
            let before = app.editor.document_snapshot();
            let metadata = app.editing.as_ref().unwrap().metadata_budget();
            let residency = app.editing.as_ref().unwrap().import_residency();
            let baseline = (metadata.usage().reserved, residency.usage().reserved);
            app.import_selected(input.clone());
            assert!(app.pending_import.is_some(), "{}", app.editor.status());
            let mut h = Harness::builder()
                .with_size(egui::vec2(1400., 1400.))
                .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
            h.run();
            h.get_by_role_and_label(
                egui::Role::ComboBox,
                &translate(language, "Negative samples"),
            )
            .click();
            h.run();
            h.get_by_label(&translate(language, "Clip negative samples to black"))
                .click();
            h.run();
            button(&mut h, language, "Prepare preview");
            if case == 5 {
                // Logical cancellation while preparation is pending; no exact decode-phase claim.
                button(&mut h, language, "Cancel");
            } else {
                wait(&mut h, false);
                if case == 0 {
                    button(&mut h, language, "Cancel");
                } else {
                    consent(&mut h, language);
                    match case {
                        1 => {
                            fs::write(&input, b"changed after preview").unwrap();
                            button(&mut h, language, "Open image");
                        }
                        2 => {
                            h.state_mut().editor.begin_text(Uuid::nil(), (0, 0));
                            h.run();
                        }
                        3 => {
                            h.state_mut().settings.experience = Experience::Classic;
                            h.run();
                        }
                        4 => {
                            let ctx = h.state().context.clone();
                            h.state_mut()
                                .add_document_tab(Editor::new_document(3, 3).unwrap(), &ctx)
                                .unwrap();
                            h.run();
                        }
                        _ => unreachable!(),
                    }
                }
            }
            assert!(
                h.state().pending_import.is_none(),
                "{}",
                h.state().editor.status()
            );
            if case == 4 {
                let prior = h
                    .state()
                    .tabs
                    .iter()
                    .filter_map(|tab| tab.state.as_ref())
                    .find(|state| Arc::ptr_eq(&before, &state.editor.document_snapshot()))
                    .expect("prior project remains in its tab");
                pixels(prior.editor.document(), &PRIOR);
            } else {
                assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
                pixels(h.state().editor.document(), &PRIOR);
            }
            retire(h.state());
            assert_eq!(
                (metadata.usage().reserved, residency.usage().reserved),
                baseline
            );
            println!(
                "Float {language:?} guard{case}: unchanged nonempty project and retired import accounts"
            );
        }
    }
}

/// Test-only artifact injection stages a real installation. Production discovers
/// exclusively from its running executable, without an environment/PATH override.
#[test]
#[ignore = "requires PAINTCRAFT_V7_HELPER pointing to the final qualified V7 artifact"]
fn copied_final_v7_helper_qualifies_float_and_existing_native_imports() {
    use paintcraft_core::types::json;
    use std::process::Command;
    let helper =
        PathBuf::from(std::env::var_os("PAINTCRAFT_V7_HELPER").expect("final V7 artifact path"));
    let base = std::env::var_os("PAINTCRAFT_FLOAT_UI_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let evidence = base.join(format!("final-descriptor-{}", Uuid::new_v4()));
    let root = evidence.join("installation");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let current = std::env::current_exe().unwrap();
    let application = root.join(current.file_name().unwrap());
    fs::copy(current, &application).unwrap();
    let helper_name = if cfg!(windows) {
        "paintcraft-codec-worker.exe"
    } else {
        "paintcraft-codec-worker"
    };
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
    let manifest = super::super::fixture_manifest::manifest(
        &installed,
        target,
        Some(wire::NATIVE_HDR_SDR_IMPORT_VARIANT),
    );
    assert_eq!(
        manifest.get("profile").and_then(|v| v.as_str()),
        Some(wire::FLOAT_NATIVE_PROFILE)
    );
    assert_eq!(
        manifest
            .get("qualified_import_variants")
            .and_then(|v| v.as_array())
            .unwrap()
            .len(),
        7
    );
    fs::write(
        root.join("paintcraft-codec-worker.json"),
        json::canonical(&manifest),
    )
    .unwrap();
    println!("Final V7 App installation: {}", root.display());
    for test in [
        "importing::native_codec::float::installed_tests::fixture_real_installed_float_settings_preview_and_adoption",
        "importing::native_codec::float::installed_tests::fixture_real_installed_float_preserves_nonempty_project_on_cancel_stale_and_source_change",
        "importing::native_codec::installed_tests::fixture_real_installed_jxl",
        "importing::native_codec::installed_tests::fixture_running_helper_cancel",
        "importing::native_codec::jxr_installed_tests::fixture_real_installed_jxr_review",
        "importing::native_codec::heif_installed_tests::fixture_real_installed_heif_review",
        "importing::native_codec::avif_installed_tests::fixture_real_installed_avif_review",
        "importing::native_codec::avif_sdr_installed_tests::fixture_real_installed_avif_sdr_review",
    ] {
        let output = Command::new(&application)
            .args([
                "--exact",
                test,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .output()
            .unwrap();
        let name = test.rsplit("::").next().unwrap();
        let mut log = output.stdout.clone();
        log.extend(&output.stderr);
        fs::write(evidence.join(format!("{name}.log")), &log).unwrap();
        assert!(
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).contains("test result: ok. 1 passed"),
            "{test}: {}",
            String::from_utf8_lossy(&log)
        );
        println!("Final V7 App fixture passed: {name}");
    }
    // Keep the complete installation, inputs, generated PCND files and logs for
    // inspection, including any failed child rather than deleting its evidence.
}
