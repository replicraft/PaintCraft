//! Actual stock acquisition, review and saved JPEG normalization consumers.
use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_core::types::json;
use paintcraft_ui::i18n::{Language, set_language, translate};
use std::{
    fs,
    time::{Duration, Instant},
};
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/jpeg-consumers/runs")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/services/paintcraft-formats/tests/jpeg-fixtures")
}
fn names() -> Vec<String> {
    let mut names: Vec<_> = (1..=8)
        .map(|i| format!("rgb-orientation-{i}-II.jpg"))
        .collect();
    names.extend(
        [
            "adobe-cmyk.jpg",
            "adobe-ycck-color.jpg",
            "unmarked-cmyk.jpg",
            "adobe-ycck-color-orientation-6-II.jpg",
            "unmarked-cmyk-orientation-6-II.jpg",
        ]
        .map(str::to_owned),
    );
    names
}
struct Case {
    name: String,
    bytes: Vec<u8>,
    pixels: Vec<u8>,
    size: (u32, u32),
    expected: paintcraft_document::Document,
}
impl Case {
    fn new(name: &str) -> Self {
        let oracle = json::parse(&fs::read(fixtures().join("oracle.json")).unwrap()).unwrap();
        let row = oracle
            .get("cases")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r.get("file").unwrap().as_str() == Some(name))
            .unwrap();
        let bytes = fs::read(fixtures().join(name)).unwrap();
        let pixels =
            fs::read(fixtures().join(row.get("rgba_file").unwrap().as_str().unwrap())).unwrap();
        assert_eq!(
            hex(&Sha256::digest(&bytes)),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        assert_eq!(
            hex(&Sha256::digest(&pixels)),
            row.get("rgba_sha256").unwrap().as_str().unwrap()
        );
        let size = (
            row.get("expected_width").unwrap().as_i64().unwrap() as u32,
            row.get("expected_height").unwrap().as_i64().unwrap() as u32,
        );
        let decoded = image::Decoded {
            format: ImageFormat::Jpeg,
            width: size.0,
            height: size.1,
            rgba: pixels.clone(),
            losses: Vec::new(),
        };
        let key = image::document_conversion_key(&bytes, &decoded, &|| Ok(()))
            .unwrap()
            .unwrap();
        let expected = import_image(size.0, size.1, &pixels, &key.hex()).unwrap();
        let old = import_image(size.0, size.1, &pixels, &hex(&Sha256::digest(&bytes))).unwrap();
        assert_ne!(expected.document_id, old.document_id);
        assert_ne!(expected.state_id, old.state_id);
        assert_ne!(expected.roots, old.roots);
        Self {
            name: name.into(),
            bytes,
            pixels,
            size,
            expected,
        }
    }
    fn assert_doc(&self, doc: &paintcraft_document::Document) {
        assert_eq!(
            doc.canvas, self.size,
            "{} normalized exactly once",
            self.name
        );
        assert_eq!(
            doc.document_id, self.expected.document_id,
            "{} document key",
            self.name
        );
        assert_eq!(doc.state_id, self.expected.state_id);
        assert_eq!(doc.roots, self.expected.roots);
        let layer = &doc.layers[&doc.roots[0]];
        let mut pixels = Vec::new();
        for y in 0..self.size.1 {
            for x in 0..self.size.0 {
                pixels.extend(layer.raster.get(i64::from(x), i64::from(y)).0);
            }
        }
        assert_eq!(
            pixels, self.pixels,
            "{} independent decoded samples",
            self.name
        );
    }
}
fn save(app: &mut App, ctx: &egui::Context, path: &Path) {
    app.save_selected(ctx, path.into());
    app.editor.run_pending();
    let until = Instant::now() + Duration::from_secs(15);
    while app.bound.as_deref() != Some(path) {
        app.poll_edits(ctx);
        app.finish_save();
        assert!(Instant::now() < until, "{}", app.editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn adopt(
    case: &Case,
    path: &Path,
    experience: Experience,
    variant: ClassicVariant,
    language: Language,
) -> (App, egui::Context) {
    let mut app = App::new(None);
    app.settings.experience = experience;
    app.settings.classic_variant = Some(variant);
    app.open_path(path).unwrap();
    let before = app.editor.document_snapshot();
    let ticket = app.editor.render_key();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1100.0, 850.0))
        .with_context_setup(move |ctx| set_language(ctx, language))
        .build_ui_state(|ui, app: &mut App| app.import_dialog(ui.ctx()), app);
    h.run();
    assert!(h.state().pending_import.is_some());
    assert!(
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .accesskit_node()
            .is_disabled()
    );
    let ctx = h.ctx.clone();
    h.state_mut().resolve_import(&ctx, true);
    assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
    assert_eq!(ticket, h.state().editor.render_key());
    if case.name.contains("cmyk") || case.name.contains("ycck") {
        h.get_by_label(&translate(
            language,
            import_loss_description("jpeg-cmyk-to-rgb").unwrap(),
        ));
    }
    if case.name.starts_with("unmarked") {
        h.get_by_label(&translate(
            language,
            import_loss_description("jpeg-color-model-assumed").unwrap(),
        ));
    }
    h.get_by_label(&translate(language, "Accept import limitations"))
        .click();
    h.run();
    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
        .click();
    h.run();
    assert!(h.state().pending_import.is_none());
    case.assert_doc(h.state().editor.document());
    let ctx = h.ctx.clone();
    (h.into_state(), ctx)
}
#[test]
fn actual_jpeg_reviews_save_exact_oriented_device_color_pixels_and_stable_normalized_ids() {
    let folder = Folder::new();
    for name in names() {
        let case = Case::new(&name);
        let source = folder.0.join(&name);
        fs::write(&source, &case.bytes).unwrap();
        for repeat in 0..2 {
            let (mut app, ctx) = adopt(
                &case,
                &source,
                Experience::Studio,
                ClassicVariant::V11,
                Language::English,
            );
            let copy = folder.0.join(format!("{name}-{repeat}.pcnd"));
            save(&mut app, &ctx, &copy);
            case.assert_doc(&paintcraft_formats::pcnd::read(&fs::read(&copy).unwrap()).unwrap());
            app.open_path(&copy).unwrap();
            case.assert_doc(app.editor.document());
            if repeat == 0
                && let Some(root) = std::env::var_os("PAINTCRAFT_JPEG_CONSUMER_EVIDENCE")
            {
                let root = PathBuf::from(root).join("native");
                fs::create_dir_all(&root).unwrap();
                fs::copy(&copy, root.join(format!("{name}.pcnd"))).unwrap();
            }
            assert_eq!(fs::read(&source).unwrap(), case.bytes);
        }
    }
    for name in [
        "rgb-orientation-6-II.jpg",
        "adobe-ycck-color.jpg",
        "unmarked-cmyk.jpg",
    ] {
        let case = Case::new(name);
        let source = folder.0.join(name);
        for (experience, variant) in [
            (Experience::Classic, ClassicVariant::V7),
            (Experience::Classic, ClassicVariant::V10),
            (Experience::Classic, ClassicVariant::V11),
            (Experience::Studio, ClassicVariant::V11),
        ] {
            for language in [Language::English, Language::Japanese] {
                let _ = adopt(&case, &source, experience, variant, language);
            }
        }
    }
}
#[test]
fn jpeg_cancel_source_replacement_and_stale_review_leave_saved_work_and_guards_intact() {
    let folder = Folder::new();
    let case = Case::new("unmarked-cmyk-orientation-6-II.jpg");
    for mode in 0..5 {
        let source = folder.0.join(format!("input-{mode}.jpg"));
        fs::write(&source, &case.bytes).unwrap();
        let prior = folder.0.join(format!("prior-{mode}.pcnd"));
        let doc = paintcraft_document::new_document(
            3,
            2,
            Some(Rgba([9, 21, 33, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        fs::write(&prior, paintcraft_formats::pcnd::write(&doc).unwrap()).unwrap();
        let mut app = App::new(None);
        app.settings.experience = Experience::Classic;
        app.settings.classic_variant = Some(ClassicVariant::V11);
        app.open_path(&prior).unwrap();
        app.open_path(&source).unwrap();
        let ctx = super::raster_review::tests::authorize(&mut app);
        match mode {
            1 => {
                let mut changed = case.bytes.clone();
                *changed.last_mut().unwrap() ^= 1;
                fs::write(&source, changed).unwrap();
            }
            2 => {
                fs::rename(&source, folder.0.join("moved.jpg")).unwrap();
                fs::write(&source, &case.bytes).unwrap();
            }
            3 => app.settings.experience = Experience::Studio,
            4 => app.editor = Editor::with_document(app.editor.document().clone(), None),
            _ => {}
        }
        let before = app.editor.document_snapshot();
        let ticket = app.editor.render_key();
        let dirty = app.editor.is_dirty();
        let bound = app.bound.clone();
        let imported = app.imported_source.clone();
        let guards: Vec<_> = app
            .auxiliary_sources
            .iter()
            .map(|g| g.path.clone())
            .collect();
        app.resolve_import(&ctx, mode != 0);
        assert!(app.pending_import.is_none());
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert_eq!(ticket, app.editor.render_key());
        assert_eq!(dirty, app.editor.is_dirty());
        assert_eq!(app.bound, bound);
        assert_eq!(app.imported_source, imported);
        assert_eq!(
            app.auxiliary_sources
                .iter()
                .map(|g| g.path.clone())
                .collect::<Vec<_>>(),
            guards
        );
    }
    let source = folder.0.join("alias-source.jpg");
    fs::write(&source, &case.bytes).unwrap();
    let (mut app, ctx) = adopt(
        &case,
        &source,
        Experience::Studio,
        ClassicVariant::V11,
        Language::English,
    );
    save(&mut app, &ctx, &folder.0.join("saved.pcnd"));
    let moved = folder.0.join("retained.jpg");
    fs::rename(&source, &moved).unwrap();
    let alias = folder.0.join("alias.pcnd");
    fs::hard_link(&moved, &alias).unwrap();
    assert!(app.guard_save_destination(&alias).is_err());
    assert!(app.guard_export_destination(&alias).is_err());
    assert_eq!(fs::read(&moved).unwrap(), case.bytes);
}
