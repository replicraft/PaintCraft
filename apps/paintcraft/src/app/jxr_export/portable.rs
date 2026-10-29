//! Actual installed App workflow against immutable separately C-qualified bytes.
//! Export uses the existing source view/text confirmation; imported output has
//! its actual decoded preview. This does not claim an encoded-export thumbnail.
use super::*;
use paintcraft_core::types::json::{self, Value};
use sha2::{Digest, Sha256};
const PROVENANCE_CANONICAL_SHA: &str =
    "905c12908ffbe4e21f47918f6b155e030655314288a47b02135ef39ff5ce8291";
const C_REFERENCE: &str = "8e3b71db2c53c5db5f988b8755946a30ac1070b7a6c0a8ffa6933d34d26b95ce";
const PROVENANCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../paintcraft-cli/tests/jxr-export-fixtures/provenance.json"
));
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../paintcraft-cli/tests/jxr-export-fixtures")
        .join(name)
}
fn field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap()
}
fn rgba(width: u32, height: u32) -> Vec<u8> {
    (0..height)
        .flat_map(|y| {
            (0..width).flat_map(move |x| {
                [
                    ((47 * x + 13 * y) % 256) as u8,
                    ((47 * x + 13 * y + 71) % 256) as u8,
                    ((47 * x + 13 * y + 142) % 256) as u8,
                    255,
                ]
            })
        })
        .collect()
}
fn golden(width: u32, height: u32) -> (Vec<u8>, Vec<u8>) {
    let manifest = json::parse(PROVENANCE).unwrap();
    // Canonical JSON keeps this pinned curation portable across checkout line endings.
    assert_eq!(
        hex(&Sha256::digest(json::canonical(&manifest))),
        PROVENANCE_CANONICAL_SHA
    );
    assert_eq!(manifest.get("schema").and_then(Value::as_i64), Some(1));
    assert_eq!(field(&manifest, "reference_sha256"), C_REFERENCE);
    let name = format!("{width}x{height}");
    let cases = manifest.get("cases").and_then(Value::as_array).unwrap();
    assert_eq!(cases.len(), 4);
    let case = cases
        .iter()
        .find(|case| field(case, "name") == name)
        .unwrap();
    assert_eq!(
        case.get("width").and_then(Value::as_i64),
        Some(i64::from(width))
    );
    assert_eq!(
        case.get("height").and_then(Value::as_i64),
        Some(i64::from(height))
    );
    let raw = fs::read(fixture(&format!("{name}.rgba"))).unwrap();
    assert_eq!(raw, rgba(width, height));
    assert_eq!(hex(&Sha256::digest(&raw)), field(case, "input_rgba_sha256"));
    let rgb = raw
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|pixel| pixel[..3].iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(
        hex(&Sha256::digest(&rgb)),
        field(case, "reference_rgb_sha256")
    );
    let encoded = fs::read(fixture(&format!("{name}.jxr"))).unwrap();
    assert_eq!(
        hex(&Sha256::digest(&encoded)),
        field(case, "encoded_sha256")
    );
    (raw, encoded)
}
pub(super) fn guard_document() -> paintcraft_document::Document {
    let (raw, _) = golden(4, 3);
    let mut document =
        paintcraft_document::import_image(4, 3, &raw, "portable-jxr-guard-hidden-layer-4x3")
            .unwrap();
    let id = paintcraft_core::types::ids::deterministic_id(&[
        "portable-jxr-guard-hidden-layer/v1",
        "4x3",
    ]);
    document.roots.push(id);
    document.layers.insert(
        id,
        paintcraft_document::Layer {
            id,
            name: "Invisible source colors".into(),
            origin: (0, 0),
            visible: false,
            opacity: 255,
            raster: paintcraft_raster::RasterLayer::new(4, 3, Rgba([19, 71, 201, 0])),
        },
    );
    assert_eq!(paintcraft_render::render_flat(&document).unwrap(), raw);
    document
}
pub(super) fn verify_output(path: &Path, width: u32, height: u32) {
    let (_, expected) = golden(width, height);
    assert_eq!(
        fs::read(path).unwrap(),
        expected,
        "complete bytes equal the independently C-qualified file"
    );
}
fn installed_writer() {
    let path = resources_manifest_path();
    let manifest = json::parse(&fs::read(path).unwrap()).unwrap();
    assert_eq!(
        field(&manifest, "application_version"),
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(
        manifest.get("schema_version").and_then(Value::as_i64),
        Some(3)
    );
    let profile = field(&manifest, "profile");
    let count = if profile == encode::qualified::NATIVE_PROFILE {
        8
    } else if profile == wire::jxl_integer::contracts::writer::NATIVE_PROFILE {
        9
    } else {
        panic!(
            "portable App fixture requires exact public V8 or V10 writer installation: {profile}"
        )
    };
    assert_eq!(
        manifest
            .get("qualified_import_variants")
            .and_then(Value::as_array)
            .unwrap()
            .len(),
        count
    );
    let exports = manifest
        .get("qualified_export_variants")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(exports.len(), 1);
    assert_eq!(exports[0].as_str(), Some(encode::qualified::EXPORT_VARIANT));
    println!(
        "Portable installed App writer profile={profile}, appSHA={}",
        hex(&Sha256::digest(
            fs::read(std::env::current_exe().unwrap()).unwrap()
        ))
    );
}
fn fresh_import(width: u32, height: u32, language: Language) {
    let root = directory().join(format!("jxr-portable-fresh-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let (raw, _) = golden(width, height);
    let source = root.join(format!(
        "portable-source-{width}x{height}-{language:?}.pcnd"
    ));
    let document = paintcraft_document::import_image(
        width,
        height,
        &raw,
        &format!("portable-jxr-{width}x{height}"),
    )
    .unwrap();
    let mut app = app(language, &source, document);
    let original = fs::read(&source).unwrap();
    let destination = root.join(format!("portable-copy-{width}x{height}-{language:?}.jxr"));
    app.path_text = destination
        .with_extension("pcnd")
        .to_string_lossy()
        .into_owned();
    let mut h = harness(app);
    let before = Before::capture(h.state());
    choose(&mut h, language, &destination);
    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Export now"))
        .click();
    h.run();
    retire(h.state());
    before.unchanged(h.state());
    verify_output(&destination, width, height);
    assert_eq!(fs::read(&source).unwrap(), original);
    h.state_mut().import_selected(destination.clone());
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        h.step();
        if h.query_all_by_role_and_label(
            egui::Role::CheckBox,
            &translate(language, "Accept import limitations"),
        )
        .count()
            > 0
        {
            break;
        }
        assert!(Instant::now() < until, "{}", h.state().editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
    before.unchanged(h.state());
    h.run();
    h.get_by_label(&translate(language, "Import preview"));
    assert!(
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_role_and_label(
        egui::Role::CheckBox,
        &translate(language, "Accept import limitations"),
    )
    .click();
    h.run();
    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
        .click();
    h.run();
    assert!(
        h.state().pending_import.is_none(),
        "{}",
        h.state().editor.status()
    );
    pixels(h.state().editor.document(), width, height, &raw);
    assert_eq!(h.state().document_count(), before.count + 1);
    let saved = root.join(format!(
        "portable-imported-{width}x{height}-{language:?}.pcnd"
    ));
    let ctx = h.state().context.clone();
    h.state_mut().save_selected(&ctx, saved.clone());
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        h.state_mut().poll_edits(&ctx);
        h.state_mut().finish_save();
        if saved.is_file() && !h.state().any_pending_edits() && !h.state().any_deferred_files() {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    let reopened = paintcraft_formats::pcnd::read(&fs::read(saved).unwrap()).unwrap();
    pixels(&reopened, width, height, &raw);
    let moved = destination.with_extension("renamed.jxr");
    fs::rename(&destination, &moved).unwrap();
    let alias = destination.with_extension("source-alias.pcnd");
    fs::hard_link(&moved, &alias).unwrap();
    let bytes = fs::read(&moved).unwrap();
    h.state_mut().save_selected(&ctx, alias.clone());
    assert_eq!(fs::read(&alias).unwrap(), bytes);
    retire(h.state());
    println!(
        "Portable App {language:?} {width}x{height}: export text consent, C-qualified whole-file identity, actual import preview/consent, PCND pixels and retained source guards"
    );
}
#[test]
fn pinned_curations_match_authored_pixels_without_executing_an_oracle() {
    for (width, height) in [(4, 3), (17, 19), (4096, 1), (1, 4096)] {
        golden(width, height);
    }
    let guard = guard_document();
    assert_eq!(paintcraft_render::render_flat(&guard).unwrap(), rgba(4, 3));
}

#[test]
#[ignore = "requires test executable copied beside a same-build qualified public V8/V10 writer installation; no C executable"]
fn fixture_installed_app_writer_matches_c_qualified_goldens() {
    installed_writer();
    cases::export_review_and_guards(Oracle::Portable);
    let mut fresh = 0;
    for language in [Language::English, Language::Japanese] {
        for (width, height) in [(4, 3), (17, 19), (4096, 1), (1, 4096)] {
            fresh_import(width, height, language);
            fresh += 1;
        }
    }
    assert_eq!(fresh, 8);
    // Reuse the established eight-case negative/positive import consumer in a
    // real same-installation child; an accidental zero-test invocation refuses.
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "importing::native_codec::jxr_installed_tests::fixture_real_installed_jxr_review",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .output()
        .unwrap();
    let mut log = result.stdout.clone();
    log.extend(&result.stderr);
    println!("{}", String::from_utf8_lossy(&log));
    assert!(result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stdout)
            .lines()
            .any(|line| line.starts_with("test result: ok. 1 passed;")),
        "missing executed import fixture"
    );
    println!(
        "Portable App gate completed14export guard+8fresh golden+8legacy import scenarios; encoded-export thumbnail is outside this fixture"
    );
}
