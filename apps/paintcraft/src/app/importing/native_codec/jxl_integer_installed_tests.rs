//! Actual copied V9/V10 installation and headless App controls, never OS windows.
use super::tests::{app as make_app, retire};
use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_core::types::json;
use paintcraft_ui::i18n::{Language, translate};
use sha2::{Digest, Sha256};
use std::{fs, process::Command, time::Instant};
macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../../../../crates/services/paintcraft-formats/tests/jxl-fixtures/",
            $name
        ))
    };
}
const INPUT8: &[u8] = fixture!("rgba8.jxl");
const INPUT16: &[u8] = fixture!("rgba16.jxl");
// Independent libjxl fixture samples, with scalar integer /257 oracle values.
const EXPECTED8: [u8; 24] = [
    10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127, 100, 110, 120, 128, 130, 140, 150, 254, 160,
    170, 180, 255,
];
const EXPECTED16: [u8; 16] = [
    0, 0, 255, 1, 1, 127, 128, 0, 255, 4, 8, 128, 16, 32, 64, 255,
];
const DEPTH: &str = "High-depth samples will be reduced from 16-bit to 8-bit.";

fn target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "aarch64") if cfg!(target_env = "gnu") => "aarch64-unknown-linux-gnu",
        ("linux", "x86_64") if cfg!(target_env = "gnu") => "x86_64-unknown-linux-gnu",
        ("windows", "aarch64") if cfg!(target_env = "msvc") => "aarch64-pc-windows-msvc",
        ("windows", "x86_64") if cfg!(target_env = "msvc") => "x86_64-pc-windows-msvc",
        _ => panic!("supported native target required"),
    }
}
#[test]
#[ignore = "requires PAINTCRAFT_JXL_INTEGER_HELPER selecting one immutable V9/V10 helper"]
fn copied_jxl_integer_helper_runs_actual_app_review_and_failure_guards() {
    let source = PathBuf::from(
        std::env::var_os("PAINTCRAFT_JXL_INTEGER_HELPER")
            .expect("select an immutable V9/V10 helper"),
    );
    assert!(source.is_file() && !source.is_symlink());
    let helper_bytes = fs::read(&source).unwrap();
    let source_sha = Sha256::digest(&helper_bytes);
    let root = std::env::temp_dir().join(format!("paintcraft-jxl-integer-app-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let current = std::env::current_exe().unwrap();
    let executable = root.join(current.file_name().unwrap());
    fs::copy(&current, &executable).unwrap();
    // Only private Linux debug test images lose DWARF. The explicitly selected
    // helper retains its exact separately qualified bytes on every platform.
    #[cfg(all(target_os = "linux", debug_assertions))]
    {
        let status = Command::new("strip")
            .arg("--strip-debug")
            .arg(&executable)
            .status()
            .unwrap();
        assert!(status.success());
    }
    let installed = root.join(if cfg!(windows) {
        "paintcraft-codec-worker.exe"
    } else {
        "paintcraft-codec-worker"
    });
    fs::write(&installed, &helper_bytes).unwrap();
    fs::set_permissions(&installed, fs::metadata(&source).unwrap().permissions()).unwrap();
    assert_eq!(Sha256::digest(fs::read(&installed).unwrap()), source_sha);
    let manifest = super::fixture_manifest::manifest(
        &installed,
        target(),
        Some(wire::jxl_integer::contracts::IMPORT_VARIANT),
    );
    fs::write(
        root.join("paintcraft-codec-worker.json"),
        json::canonical(&manifest),
    )
    .unwrap();
    println!(
        "JXL integer copied installation: {}; helper_sha256={}",
        root.display(),
        hex(&source_sha)
    );
    let output = Command::new(&executable).args([
        "--exact", "importing::native_codec::jxl_integer_installed_tests::fixture_real_installed_jxl_integer_app",
        "--ignored", "--nocapture", "--test-threads=1",
    ]).output().unwrap();
    if let Some(base) = std::env::var_os("PAINTCRAFT_JXL_INTEGER_GUI_EVIDENCE") {
        let evidence = PathBuf::from(base).join(format!("attempt-{}", Uuid::new_v4()));
        fs::create_dir_all(&evidence).unwrap();
        fs::write(evidence.join("stdout.log"), &output.stdout).unwrap();
        fs::write(evidence.join("stderr.log"), &output.stderr).unwrap();
        fs::write(evidence.join("manifest.json"), json::canonical(&manifest)).unwrap();
        fs::write(evidence.join("helper.sha256"), hex(&source_sha)).unwrap();
        println!("JXL integer GUI evidence: {}", evidence.display());
    }
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("test result: ok. 1 passed"));
    assert_eq!(
        Sha256::digest(fs::read(&source).unwrap()),
        source_sha,
        "immutable helper changed"
    );
    fs::remove_dir_all(root).unwrap();
}

struct Before {
    doc: Arc<paintcraft_document::Document>,
    ticket: paintcraft_editor::session::HistoryTicket,
    dirty: bool,
    bound: Option<PathBuf>,
    imported_source: Option<PathBuf>,
    editor_path: Option<PathBuf>,
    path_text: String,
    tab: Uuid,
    tabs: usize,
    auxiliary: Vec<crate::destination::SourceGuard>,
}
impl Before {
    fn capture(app: &App) -> Self {
        Self {
            doc: app.editor.document_snapshot(),
            ticket: app.editor.history_view().ticket,
            dirty: app.editor.is_dirty(),
            bound: app.bound.clone(),
            imported_source: app.imported_source.clone(),
            editor_path: app.editor.path().map(Path::to_path_buf),
            path_text: app.path_text.clone(),
            tab: app.active_tab_id(),
            tabs: app.document_count(),
            auxiliary: app.auxiliary_sources.clone(),
        }
    }
    fn unchanged(&self, app: &App) {
        assert!(Arc::ptr_eq(&self.doc, &app.editor.document_snapshot()));
        assert_eq!(self.ticket, app.editor.history_view().ticket);
        assert_eq!(self.dirty, app.editor.is_dirty());
        assert_eq!(self.bound, app.bound);
        assert_eq!(self.imported_source, app.imported_source);
        assert_eq!(self.editor_path.as_deref(), app.editor.path());
        assert_eq!(self.path_text, app.path_text);
        assert_eq!(self.tab, app.active_tab_id());
        assert_eq!(self.tabs, app.document_count());
        assert_eq!(self.auxiliary.len(), app.auxiliary_sources.len());
        for (left, right) in self.auxiliary.iter().zip(&app.auxiliary_sources) {
            assert_eq!(left.path, right.path);
            assert!(left.identity.matches(&right.identity));
        }
    }
}
fn pixels(doc: &paintcraft_document::Document, width: u32, height: u32, expected: &[u8]) {
    assert_eq!(doc.canvas, (width, height));
    assert_eq!(doc.roots.len(), 1);
    assert_eq!(expected.len(), width as usize * height as usize * 4);
    let raster = &doc.layers[&doc.roots[0]].raster;
    for (i, pixel) in expected.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            raster
                .get((i % width as usize) as i64, (i / width as usize) as i64)
                .0,
            *pixel,
            "pixel{i}"
        );
    }
}
fn source<'a>(h: &'a Harness<'_, App>) -> &'a Source {
    let Candidate::NativeCodec(source) = &h
        .state()
        .pending_import
        .as_ref()
        .expect("pending native import")
        .candidate
    else {
        panic!("native codec expected")
    };
    source
}
fn source_mut<'a>(h: &'a mut Harness<'_, App>) -> &'a mut Source {
    let Candidate::NativeCodec(source) = &mut h
        .state_mut()
        .pending_import
        .as_mut()
        .expect("pending native import")
        .candidate
    else {
        panic!("native codec expected")
    };
    source
}
fn pump_closed(h: &mut Harness<App>) {
    let until = Instant::now() + Duration::from_secs(20);
    while h.state().pending_import.is_some() {
        h.step();
        assert!(Instant::now() < until, "{}", h.state().editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn save(h: &mut Harness<App>, path: &Path) {
    let ctx = h.state().context.clone();
    h.state_mut().save_selected(&ctx, path.to_path_buf());
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        h.state_mut().poll_edits(&ctx);
        h.state_mut().finish_save();
        if path.is_file() && !h.state().any_pending_edits() && !h.state().any_deferred_files() {
            break;
        }
        assert!(Instant::now() < until, "{}", h.state().editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn harness(language: Language, root: &Path, label: &str) -> Harness<'static, App> {
    let mut app = make_app();
    app.settings.language = language;
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 1000.0))
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    // Import after Harness construction: its automatic setup frames must not
    // hide the actual first-preview-frame readiness checks below.
    let input = root.join(format!("{label}-prior-{language:?}.jxl"));
    fs::write(&input, INPUT8).unwrap();
    h.state_mut().import_selected(input.clone());
    let until = Instant::now() + Duration::from_secs(20);
    while h.state().pending_import.is_some() {
        h.step();
        if h.state().pending_import.is_some() {
            assert!(
                source(&h).review.is_none(),
                "8-bit import invented loss consent"
            );
        }
        assert!(Instant::now() < until, "{}", h.state().editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
    pixels(h.state().editor.document(), 3, 2, &EXPECTED8);
    assert!(h.state().editor.path().is_none() && h.state().bound.is_none());
    assert!(h.state().editor.is_dirty());
    assert_eq!(h.state().auxiliary_sources.len(), 1);
    assert_eq!(h.state().imported_source.as_deref(), Some(input.as_path()));
    let project = root.join(format!("{label}-prior-{language:?}.pcnd"));
    save(&mut h, &project);
    assert_eq!(h.state().bound.as_deref(), Some(project.as_path()));
    assert_eq!(h.state().imported_source.as_deref(), Some(input.as_path()));
    h.state_mut().editor.invalidate_saved_state();
    assert!(h.state().editor.is_dirty());
    retire(h.state());
    h
}
fn wait_review(h: &mut Harness<App>, language: Language) {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        h.step();
        if source(h).review.is_some() {
            break;
        }
        assert!(Instant::now() < until, "{}", h.state().editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
    let prepared = source(h).prepared.as_ref().unwrap();
    assert_eq!(
        prepared.decoder_profile(),
        wire::jxl_integer::contracts::IMPORT_VARIANT
    );
    assert_eq!(prepared.losses(), &[paintcraft_formats::jxl::DEPTH16_LOSS]);
    let report = prepared.jxl_integer_assessment().unwrap();
    assert_eq!(
        (report.source_depth, report.width, report.height),
        (16, 2, 2)
    );
    assert_eq!(report.changed_rgb_components, 8);
    assert_eq!(report.changed_alpha, 2);
    assert_eq!(
        report.source_sha256,
        <[u8; 32]>::from(Sha256::digest(INPUT16))
    );
    assert_eq!(
        report.output_sha256,
        <[u8; 32]>::from(Sha256::digest(EXPECTED16))
    );
    pixels(prepared.document(), 2, 2, &EXPECTED16);
    h.get_by_label(&translate(language, "Import preview"));
    h.get_by_label(&translate(language, DEPTH));
    if language == Language::Japanese {
        assert_ne!(translate(language, DEPTH), DEPTH);
    }
    assert!(
        h.get_by_role_and_label(
            egui::Role::CheckBox,
            &translate(language, "Accept import limitations")
        )
        .accesskit_node()
        .is_disabled(),
        "consent enabled in first preview frame"
    );
    assert!(
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .accesskit_node()
            .is_disabled()
    );
    h.step();
    assert!(
        !h.get_by_role_and_label(
            egui::Role::CheckBox,
            &translate(language, "Accept import limitations")
        )
        .accesskit_node()
        .is_disabled(),
        "visible previous frame did not enable consent"
    );
    assert!(
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .accesskit_node()
            .is_disabled()
    );
}
fn consent(h: &mut Harness<App>, language: Language) {
    h.get_by_role_and_label(
        egui::Role::CheckBox,
        &translate(language, "Accept import limitations"),
    )
    .click();
    h.step();
    h.step();
    assert!(
        !h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .accesskit_node()
            .is_disabled()
    );
}
fn click(h: &mut Harness<App>, language: Language, label: &str) {
    h.get_by_role_and_label(egui::Role::Button, &translate(language, label))
        .click();
    h.step();
    h.step();
}
fn assert_retired(
    h: &Harness<App>,
    metadata: &paintcraft_runtime::SharedBudget,
    metadata_before: u64,
    resident: &paintcraft_runtime::SharedBudget,
    resident_before: u64,
) {
    retire(h.state());
    assert_eq!(h.state().worker_budget.usage().reserved, 0);
    assert_eq!(metadata.usage().reserved, metadata_before);
    assert_eq!(resident.usage().reserved, resident_before);
}
fn accepted(language: Language, root: &Path) {
    let mut h = harness(language, root, "accepted");
    let before = Before::capture(h.state());
    let input = root.join(format!("accepted-{language:?}.jxl"));
    fs::write(&input, INPUT16).unwrap();
    let metadata = h.state().editing.as_ref().unwrap().metadata_budget();
    let baseline = metadata.usage().reserved;
    h.state_mut().import_selected(input.clone());
    wait_review(&mut h, language);
    before.unchanged(h.state());
    assert!(metadata.usage().reserved > baseline);
    // Pending source aliases are protected before any consent or adoption.
    let alias = root.join(format!("pending-{language:?}.pcnd"));
    fs::hard_link(&input, &alias).unwrap();
    let ctx = h.state().context.clone();
    h.state_mut().save_selected(&ctx, alias.clone());
    assert_eq!(fs::read(&alias).unwrap(), INPUT16);
    before.unchanged(h.state());
    consent(&mut h, language);
    click(&mut h, language, "Open image");
    assert!(
        h.state().pending_import.is_none(),
        "{}",
        h.state().editor.status()
    );
    pixels(h.state().editor.document(), 2, 2, &EXPECTED16);
    assert!(h.state().editor.is_dirty());
    assert!(h.state().editor.path().is_none() && h.state().bound.is_none());
    assert_eq!(h.state().auxiliary_sources.len(), 1);
    assert_eq!(h.state().imported_source.as_deref(), Some(input.as_path()));
    assert_eq!(metadata.usage().reserved, baseline);
    retire(h.state());
    let project = root.join(format!("integer-{language:?}.pcnd"));
    save(&mut h, &project);
    let reopened = paintcraft_formats::pcnd::read_with_limits(
        &fs::read(&project).unwrap(),
        256,
        paintcraft_core::types::policy::STAGED_PIXELS_MAX,
    )
    .unwrap();
    pixels(&reopened, 2, 2, &EXPECTED16);
    let moved = root.join(format!("renamed-{language:?}.jxl"));
    fs::rename(&input, &moved).unwrap();
    let alias = root.join(format!("source-guard-{language:?}.pcnd"));
    fs::hard_link(&moved, &alias).unwrap();
    let source_before = fs::read(&moved).unwrap();
    h.state_mut().save_selected(&ctx, alias.clone());
    assert_eq!(fs::read(&alias).unwrap(), source_before);
    assert_eq!(h.state().bound.as_deref(), Some(project.as_path()));
    println!(
        "JXL integer {language:?}: 8-bit automatic;16-bit visible review/consent; exact raw hidden RGB/alpha; PCND/source aliases"
    );
}
fn refused(language: Language, root: &Path) {
    for (name, bytes) in [
        ("icc", fixture!("icc.jxl").as_slice()),
        (
            "mixed8-16",
            fixture!("alpha-profiles/color8-alpha16.jxl").as_slice(),
        ),
        (
            "mixed16-8",
            fixture!("alpha-profiles/color16-alpha8.jxl").as_slice(),
        ),
    ] {
        let mut h = harness(language, root, name);
        let before = Before::capture(h.state());
        let metadata = h.state().editing.as_ref().unwrap().metadata_budget();
        let resident = h.state().editing.as_ref().unwrap().import_residency();
        let baseline = (metadata.usage().reserved, resident.usage().reserved);
        let input = root.join(format!("refuse-{name}-{language:?}.jxl"));
        fs::write(&input, bytes).unwrap();
        h.state_mut().import_selected(input.clone());
        pump_closed(&mut h);
        before.unchanged(h.state());
        assert!(h.state().editor.status().starts_with("Cannot open:"));
        assert_eq!(fs::read(input).unwrap(), bytes);
        assert_retired(&h, &metadata, baseline.0, &resident, baseline.1);
        println!(
            "JXL integer {language:?} {name}: strict refusal preserved existing document/binding/sources"
        );
    }
}
fn cancelled_or_stale(language: Language, root: &Path) {
    for case in 0..8 {
        let mut h = harness(language, root, &format!("stale{case}"));
        let input = root.join(format!("stale-{case}-{language:?}.jxl"));
        fs::write(&input, INPUT16).unwrap();
        let old = Before::capture(h.state());
        let metadata = h.state().editing.as_ref().unwrap().metadata_budget();
        let resident = h.state().editing.as_ref().unwrap().import_residency();
        let baseline = (metadata.usage().reserved, resident.usage().reserved);
        h.state_mut().import_selected(input.clone());
        wait_review(&mut h, language);
        old.unchanged(h.state());
        consent(&mut h, language);
        match case {
            0 => click(&mut h, language, "Cancel"),
            1 => {
                fs::write(&input, b"changed after preview").unwrap();
                click(&mut h, language, "Open image");
            }
            2 => {
                source_mut(&mut h)
                    .review
                    .as_mut()
                    .unwrap()
                    .jxl_digest
                    .as_mut()
                    .unwrap()[0] ^= 1;
                click(&mut h, language, "Open image");
            }
            3 => {
                h.state_mut().editor = Editor::with_document(
                    h.state().editor.document().clone(),
                    h.state().editor.path().map(Path::to_path_buf),
                );
            }
            4 => {
                let layer = h.state().editor.document().roots[0];
                h.state_mut().editor.begin_text(layer, (0, 0));
                h.state_mut().editor.type_text("keep draft");
            }
            5 => {
                let ctx = h.state().context.clone();
                h.state_mut()
                    .add_document_tab(Editor::new_document(3, 3).unwrap(), &ctx)
                    .unwrap();
            }
            6 => h.state_mut().settings.experience = Experience::Classic,
            _ => {
                let mut parked = tabs::WorkspaceDocuments::new((4, 4), false);
                parked.swap(h.state_mut());
                h.state_mut().other_workspace = Some(Box::new(parked));
                h.state_mut().settings.experience = Experience::Classic;
            }
        }
        let after_change = Before::capture(h.state());
        if h.state().pending_import.is_some() {
            pump_closed(&mut h);
        }
        assert!(h.state().pending_import.is_none());
        if case <= 2 {
            old.unchanged(h.state());
        } else {
            after_change.unchanged(h.state());
        }
        if case == 4 {
            assert_eq!(h.state().editor.canvas_text_content(), Some("keep draft"));
        }
        if case == 5 {
            let parked = h
                .state()
                .tabs
                .iter()
                .filter_map(|tab| tab.state.as_ref())
                .find(|state| Arc::ptr_eq(&old.doc, &state.editor.document_snapshot()))
                .expect("prior document parked");
            assert_eq!(parked.editor.history_view().ticket, old.ticket);
            pixels(parked.editor.document(), 3, 2, &EXPECTED8);
        }
        if case == 7 {
            let parked = &h.state().other_workspace.as_ref().unwrap().active.editor;
            assert!(Arc::ptr_eq(&old.doc, &parked.document_snapshot()));
            assert_eq!(parked.history_view().ticket, old.ticket);
        }
        assert_retired(&h, &metadata, baseline.0, &resident, baseline.1);
        println!(
            "JXL integer {language:?} cancellation/stale case{case}: no publication, unchanged target/bindings/source ownership and retired preparation"
        );
    }
}
#[test]
#[ignore = "private copied V9/V10 App installation; run through explicit parent"]
fn fixture_real_installed_jxl_integer_app() {
    let root = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    for language in [Language::English, Language::Japanese] {
        accepted(language, &root);
        cancelled_or_stale(language, &root);
        refused(language, &root);
    }
}
