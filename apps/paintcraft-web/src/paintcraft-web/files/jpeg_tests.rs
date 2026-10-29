//! Actual browser frontend review and PCND serialization of normalized JPEGs.
use super::*;
use crate::{WebApp, importing::Owner};
use eframe::egui;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_core::types::json;
use paintcraft_ui::i18n::{Language, set_language, translate};
use std::{path::PathBuf, sync::Arc};
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/services/paintcraft-formats/tests/jpeg-fixtures")
}
fn names() -> Vec<String> {
    let mut v: Vec<_> = (1..=8)
        .map(|i| format!("rgb-orientation-{i}-II.jpg"))
        .collect();
    v.extend(
        [
            "adobe-cmyk.jpg",
            "adobe-ycck-color.jpg",
            "unmarked-cmyk.jpg",
            "adobe-ycck-color-orientation-6-II.jpg",
            "unmarked-cmyk-orientation-6-II.jpg",
        ]
        .map(str::to_owned),
    );
    v
}
struct Case {
    name: String,
    bytes: Vec<u8>,
    pixels: Vec<u8>,
    size: (u32, u32),
    expected: Document,
}
impl Case {
    fn new(name: &str) -> Self {
        let data = json::parse(&std::fs::read(fixtures().join("oracle.json")).unwrap()).unwrap();
        let row = data
            .get("cases")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r.get("file").unwrap().as_str() == Some(name))
            .unwrap();
        let bytes = std::fs::read(fixtures().join(name)).unwrap();
        let pixels =
            std::fs::read(fixtures().join(row.get("rgba_file").unwrap().as_str().unwrap()))
                .unwrap();
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&bytes)),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&pixels)),
            row.get("rgba_sha256").unwrap().as_str().unwrap()
        );
        let size = (
            row.get("expected_width").unwrap().as_i64().unwrap() as u32,
            row.get("expected_height").unwrap().as_i64().unwrap() as u32,
        );
        let decoded = image::Decoded {
            format: image::ImageFormat::Jpeg,
            width: size.0,
            height: size.1,
            rgba: pixels.clone(),
            losses: Vec::new(),
        };
        let key = image::document_conversion_key(&bytes, &decoded, &|| Ok(()))
            .unwrap()
            .unwrap();
        let expected =
            paintcraft_document::import_image(size.0, size.1, &pixels, &key.hex()).unwrap();
        let old = paintcraft_document::import_image(
            size.0,
            size.1,
            &pixels,
            &paintcraft_document::hex(&Sha256::digest(&bytes)),
        )
        .unwrap();
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
    fn assert_doc(&self, doc: &Document) {
        assert_eq!(doc.canvas, self.size, "{} normalized once", self.name);
        assert_eq!(doc.document_id, self.expected.document_id);
        assert_eq!(doc.state_id, self.expected.state_id);
        assert_eq!(doc.roots, self.expected.roots);
        let layer = &doc.layers[&doc.roots[0]];
        let mut rgba = Vec::new();
        for y in 0..self.size.1 {
            for x in 0..self.size.0 {
                rgba.extend(layer.raster.get(i64::from(x), i64::from(y)).0);
            }
        }
        assert_eq!(rgba, self.pixels, "{} independent raw oracle", self.name);
    }
}
fn adopt(case: &Case, frontend: Frontend, language: Language) -> WebApp {
    use paintcraft_classic::ClassicVariant;
    let loaded = decode_for_profile(&case.bytes, case.name.clone(), frontend).unwrap();
    assert!(loaded.raster_preview);
    assert!(!loaded.losses.is_empty());
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200.0, 900.0))
        .build_eframe(move |cc| {
            let mut app = WebApp::new(&cc.egui_ctx);
            app.variant = match frontend {
                Frontend::ClassicV7 => ClassicVariant::V7,
                Frontend::ClassicV10 => ClassicVariant::V10,
                _ => ClassicVariant::V11,
            };
            if frontend == Frontend::Studio {
                app.frontend = paintcraft_ui::actions::Frontend::Studio;
            }
            set_language(&cc.egui_ctx, language);
            app.import_owner = Some(Owner::capture(&app));
            app.accept_loaded(loaded, &cc.egui_ctx);
            app
        });
    h.run();
    let before = h.state().editor.session.snapshot();
    let ticket = h.state().editor.session.ticket();
    assert!(h.state().pending_import.is_some());
    assert!(
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .accesskit_node()
            .is_disabled()
    );
    let ctx = h.ctx.clone();
    h.state_mut().resolve_import(&ctx, true);
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    assert_eq!(ticket, h.state().editor.session.ticket());
    if case.name.contains("cmyk") || case.name.contains("ycck") {
        h.get_by_label(&translate(
            language,
            paintcraft_ui::i18n::import_loss_description("jpeg-cmyk-to-rgb").unwrap(),
        ));
    }
    if case.name.starts_with("unmarked") {
        h.get_by_label(&translate(
            language,
            paintcraft_ui::i18n::import_loss_description("jpeg-color-model-assumed").unwrap(),
        ));
    }
    h.get_by_label(&translate(language, "Accept import limitations"))
        .click();
    h.run();
    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
        .click();
    h.run();
    assert!(h.state().pending_import.is_none());
    case.assert_doc(h.state().editor.session.document());
    h.into_state()
}
#[test]
fn actual_jpeg_web_reviews_and_pcnd_reopen_keep_independent_pixels_and_shared_normalized_ids() {
    for name in names() {
        let case = Case::new(&name);
        for repeat in 0..2 {
            let mut app = adopt(&case, Frontend::Studio, Language::English);
            let bytes = encode_quality_for_profile(
                app.editor.session.document(),
                None,
                90,
                Frontend::Studio,
            )
            .unwrap();
            case.assert_doc(&pcnd::read(&bytes).unwrap());
            let ctx = egui::Context::default();
            app.import_owner = Some(Owner::capture(&app));
            app.accept_loaded(
                decode_for_profile(&bytes, "reopened.pcnd".into(), Frontend::Studio).unwrap(),
                &ctx,
            );
            assert!(app.pending_import.is_none());
            case.assert_doc(app.editor.session.document());
            if repeat == 0
                && let Some(root) = std::env::var_os("PAINTCRAFT_JPEG_CONSUMER_EVIDENCE")
            {
                let root = PathBuf::from(root).join("web");
                std::fs::create_dir_all(&root).unwrap();
                std::fs::write(root.join(format!("{name}.pcnd")), &bytes).unwrap();
            }
        }
    }
    for name in [
        "rgb-orientation-6-II.jpg",
        "adobe-ycck-color.jpg",
        "unmarked-cmyk.jpg",
    ] {
        let case = Case::new(name);
        for frontend in [
            Frontend::ClassicV7,
            Frontend::ClassicV10,
            Frontend::ClassicV11,
            Frontend::Studio,
        ] {
            for language in [Language::English, Language::Japanese] {
                let _ = adopt(&case, frontend, language);
            }
        }
    }
}
#[test]
fn jpeg_web_cancel_stale_sessions_profiles_and_drafts_preserve_prior_work() {
    let case = Case::new("unmarked-cmyk-orientation-6-II.jpg");
    for mode in 0..5 {
        let ctx = egui::Context::default();
        let mut app = WebApp::new(&ctx);
        let prior = paintcraft_document::new_document(
            3,
            2,
            Some(paintcraft_core::types::Rgba([9, 21, 33, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        app.editor = crate::editor::Editor::from_document(prior).unwrap();
        app.editor.session.mark_unsaved();
        app.import_owner = Some(Owner::capture(&app));
        app.accept_loaded(
            decode_for_profile(&case.bytes, case.name.clone(), Frontend::ClassicV11).unwrap(),
            &ctx,
        );
        let mut h = Harness::builder()
            .with_size(egui::vec2(1000.0, 800.0))
            .build_ui_state(
                |ui, app: &mut WebApp| app.raster_import_dialog(ui.ctx()),
                app,
            );
        h.run();
        h.get_by_label("Accept import limitations").click();
        h.run();
        let ctx = h.ctx.clone();
        let mut app = h.into_state();
        match mode {
            1 => app.variant = paintcraft_classic::ClassicVariant::V7,
            2 => app.frontend = paintcraft_ui::actions::Frontend::Studio,
            3 => app.text.preedit = "composition".into(),
            4 => {
                app.editor =
                    crate::editor::Editor::from_document(app.editor.session.document().clone())
                        .unwrap()
            }
            _ => {}
        }
        let before = app.editor.session.snapshot();
        let ticket = app.editor.session.ticket();
        let dirty = app.editor.session.is_dirty();
        app.resolve_import(&ctx, mode != 0);
        assert!(app.pending_import.is_none());
        assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
        assert_eq!(ticket, app.editor.session.ticket());
        assert_eq!(dirty, app.editor.session.is_dirty());
    }
}
