//! Real copied public installation and headless Studio controls, never OS windows.
use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, translate};
use std::{fs, time::Instant};
#[path = "installed_cases.rs"]
mod cases;
#[path = "portable.rs"]
mod portable;
#[derive(Clone, Copy)]
enum Oracle {
    C,
    Portable,
}
const ORA: &[u8] = include_bytes!(
    "../../../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/flat.ora"
);
const EXPECTED: &[u8] = include_bytes!(
    "../../../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/merged.rgba"
);

struct Before {
    doc: Arc<paintcraft_document::Document>,
    ticket: paintcraft_editor::session::HistoryTicket,
    dirty: bool,
    bound: Option<PathBuf>,
    editor_path: Option<PathBuf>,
    path_text: String,
    tab: Uuid,
    count: usize,
}
impl Before {
    fn capture(app: &App) -> Self {
        Self {
            doc: app.editor.document_snapshot(),
            ticket: app.editor.history_view().ticket,
            dirty: app.editor.is_dirty(),
            bound: app.bound.clone(),
            editor_path: app.editor.path().map(Path::to_path_buf),
            path_text: app.path_text.clone(),
            tab: app.active_tab_id(),
            count: app.document_count(),
        }
    }
    fn unchanged(&self, app: &App) {
        assert!(Arc::ptr_eq(&self.doc, &app.editor.document_snapshot()));
        assert_eq!(self.ticket, app.editor.history_view().ticket);
        assert_eq!(self.dirty, app.editor.is_dirty());
        assert_eq!(self.bound, app.bound);
        assert_eq!(self.editor_path.as_deref(), app.editor.path());
        assert_eq!(self.path_text, app.path_text);
        assert_eq!(self.tab, app.active_tab_id());
        assert_eq!(self.count, app.document_count());
    }
}
fn directory() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
// The driver may copy this test executable into a real Mac bundle layout.
// Select that shape explicitly; a missing Resources payload is a fixture error,
// never permission to retry adjacent metadata and mask production discovery.
fn resources_manifest_path() -> PathBuf {
    let directory = directory();
    let adjacent = directory.join("paintcraft-codec-worker.json");
    let contents = directory.parent();
    let bundle = contents.and_then(Path::parent);
    if cfg!(target_os = "macos")
        && directory.file_name().is_some_and(|n| n == "MacOS")
        && contents.is_some_and(|p| p.file_name().is_some_and(|n| n == "Contents"))
        && bundle.is_some_and(|p| p.extension().is_some_and(|n| n == "app"))
    {
        assert!(
            fs::symlink_metadata(&adjacent)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
            "mixed bundle fixture"
        );
        let resources = contents.unwrap().join("Resources");
        assert!(resources.is_dir() && !resources.is_symlink());
        let manifest = resources.join("paintcraft-codec-worker.json");
        assert!(manifest.is_file() && !manifest.is_symlink());
        manifest
    } else {
        assert!(adjacent.is_file() && !adjacent.is_symlink());
        adjacent
    }
}
fn app(language: Language, source: &Path, document: paintcraft_document::Document) -> App {
    fs::write(source, paintcraft_formats::pcnd::write(&document).unwrap()).unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.settings.language = language;
    app.initialize_editing(app.context.clone());
    // Use the production Open association: install(with_document(path)) alone
    // does not set App.bound and therefore is not an acquired document source.
    app.open_path(source).unwrap();
    assert_eq!(app.bound.as_deref(), Some(source));
    app
}
fn harness(app: App) -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_max_steps(40)
        .with_size(egui::vec2(1360.0, 1000.0))
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    h.run();
    h
}
fn retire(app: &App) {
    let until = Instant::now() + Duration::from_secs(15);
    while app.worker_budget.usage().reserved != 0 {
        app.editing.as_ref().unwrap().reap_abandoned();
        assert!(Instant::now() < until, "worker charge retained");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn wait_preview(h: &mut Harness<App>, language: Language) {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        h.step();
        let state = h
            .state()
            .jxr_export
            .as_ref()
            .unwrap_or_else(|| panic!("{}", h.state().editor.status()));
        if state.texture.is_some()
            && state.shown.is_some()
            && !h
                .get_by_role_and_label(
                    egui::Role::Button,
                    &translate(language, "Choose destination…"),
                )
                .accesskit_node()
                .is_disabled()
        {
            break;
        }
        assert!(Instant::now() < until, "{}", h.state().editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
    h.run();
    h.get_by_label(&translate(language, "Export preview"));
    assert!(!h.state().jxr_export.as_ref().unwrap().reviewed);
}
fn wait_review(h: &mut Harness<App>, language: Language) {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        h.step();
        let state = h
            .state()
            .jxr_export
            .as_ref()
            .unwrap_or_else(|| panic!("{}", h.state().editor.status()));
        if state.prepared.is_some() && state.destination.is_some() {
            break;
        }
        assert!(Instant::now() < until, "{}", h.state().editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
    h.run();
    assert!(h.state().worker_budget.usage().reserved > 0);
    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Export now"));
}
fn choose(h: &mut Harness<App>, language: Language, path: &Path) {
    h.state_mut().path_text = path.with_extension("pcnd").to_string_lossy().into_owned();
    h.state_mut()
        .begin_export_options(paintcraft_ui::actions::ImageExport::Png);
    h.run();
    h.get_by_role_and_label(egui::Role::ComboBox, &translate(language, "File format"))
        .click();
    h.run();
    h.get_by_label(&translate(language, "JPEG XR picture"))
        .scroll_to_me();
    h.run();
    h.get_by_label(&translate(language, "JPEG XR picture"))
        .click();
    h.run();
    h.get_by_label(&translate(
        language,
        "Save a lossless JPEG XR picture. The picture must be entirely opaque.",
    ));
    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Preview export"))
        .click();
    h.step();
    wait_preview(h, language);
    h.get_by_role_and_label(
        egui::Role::Button,
        &translate(language, "Choose destination…"),
    )
    .click();
    h.step();
    wait_review(h, language);
}
fn reference(path: &Path, width: u32, height: u32, expected: &[u8]) {
    let reference =
        PathBuf::from(std::env::var_os("PAINTCRAFT_JXR_REFERENCE").expect("pinned C reference"));
    use sha2::{Digest, Sha256};
    let reference_sha = Sha256::digest(fs::read(&reference).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(
        reference_sha, "8e3b71db2c53c5db5f988b8755946a30ac1070b7a6c0a8ffa6933d34d26b95ce",
        "pinned independent C oracle"
    );
    let output = path.with_extension("reference.rgb");
    let args = [
        "decode".into(),
        width.to_string().into(),
        height.to_string().into(),
        path.as_os_str().to_owned(),
        output.as_os_str().to_owned(),
    ];
    let result = paintcraft_runtime::process::run(
        &reference,
        &args,
        vec![],
        paintcraft_runtime::process::Limits {
            input_bytes: 1,
            output_bytes: 65536,
            deadline: Duration::from_secs(10),
            resource_exit_code: None,
        },
        &|| Ok(()),
    )
    .unwrap();
    assert!(
        result.status.success(),
        "C decoder failed: {:?}: {}",
        result.status,
        String::from_utf8_lossy(&result.bytes)
    );
    let rgb: Vec<_> = expected
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| p[..3].iter().copied())
        .collect();
    assert_eq!(fs::read(output).unwrap(), rgb, "independent C exact RGB");
}
fn pixels(doc: &paintcraft_document::Document, width: u32, height: u32, expected: &[u8]) {
    assert_eq!(doc.canvas, (width, height));
    assert_eq!(doc.roots.len(), 1);
    let layer = &doc.layers[&doc.roots[0]];
    for (index, pixel) in expected.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            layer
                .raster
                .get(
                    (index % width as usize) as i64,
                    (index / width as usize) as i64
                )
                .0,
            *pixel,
            "pixel{index}"
        );
    }
}
#[test]
#[ignore = "requires PAINTCRAFT_JXR_PUBLIC_HELPER and pinned PAINTCRAFT_JXR_REFERENCE"]
fn copied_public_v8_runs_native_studio_export_and_own_reader() {
    use paintcraft_core::types::json;
    use sha2::{Digest, Sha256};
    let helper = PathBuf::from(
        std::env::var_os("PAINTCRAFT_JXR_PUBLIC_HELPER").expect("qualified public V8 helper"),
    );
    let base = std::env::var_os("PAINTCRAFT_JXR_UI_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let evidence = base.join(format!("native-public-{}", Uuid::new_v4()));
    let root = evidence.join("installation");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let current = std::env::current_exe().unwrap();
    let executable = root.join(current.file_name().unwrap());
    fs::copy(&current, &executable).unwrap();
    let producer = helper.parent().unwrap();
    for name in [
        helper.file_name().unwrap().to_str().unwrap(),
        "paintcraft-codec-worker.json",
        "JPEGXR_LICENSE.txt",
        "DAV1D_LICENSE.txt",
        "HDR_EXR_LICENSES_V1.txt",
        "LICENSE",
    ] {
        fs::copy(producer.join(name), root.join(name)).unwrap();
    }
    let manifest = fs::read(root.join("paintcraft-codec-worker.json")).unwrap();
    let parsed = json::parse(&manifest).unwrap();
    assert_eq!(
        parsed.get("profile").and_then(|v| v.as_str()),
        Some(encode::qualified::NATIVE_PROFILE)
    );
    assert_eq!(
        parsed.get("schema_version").and_then(|v| v.as_i64()),
        Some(3)
    );
    assert_eq!(
        parsed
            .get("qualified_import_variants")
            .and_then(|v| v.as_array())
            .unwrap()
            .len(),
        8
    );
    assert_eq!(
        parsed
            .get("qualified_export_variants")
            .and_then(|v| v.as_array())
            .unwrap()
            .len(),
        1
    );
    println!(
        "Native public installation={} helperSHA={} appSHA={}",
        root.display(),
        Sha256::digest(fs::read(&helper).unwrap())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        Sha256::digest(fs::read(&current).unwrap())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    for name in [
        "jxr_export::installed_tests::cases::fixture_real_export_review_and_guards",
        "jxr_export::installed_tests::cases::fixture_real_fresh_long_output_import_and_source_guard",
        "importing::native_codec::jxr_installed_tests::fixture_real_installed_jxr_review",
    ] {
        let result = std::process::Command::new(&executable)
            .args([
                "--exact",
                name,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .output()
            .unwrap();
        let mut log = result.stdout.clone();
        log.extend(&result.stderr);
        fs::write(
            evidence.join(format!("{}.log", name.rsplit("::").next().unwrap())),
            &log,
        )
        .unwrap();
        assert!(
            result.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&log)
        );
    }
}

#[path = "metadata_installed.rs"]
mod metadata;

#[path = "preview_refusals.rs"]
mod preview_refusals;
