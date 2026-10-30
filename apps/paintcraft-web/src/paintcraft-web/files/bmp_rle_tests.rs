//! Actual browser frontend review and PCND serialization of imported BMP RLEs.
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
        .join("../../crates/services/paintcraft-formats/tests/bmp-rle-fixtures")
}
fn names() -> Vec<String> {
    [
        "rle4-combined.bmp",
        "rle8-combined.bmp",
        "rle4-size-zero.bmp",
        "rle8-size-zero.bmp",
        "rle4-icc-before.bmp",
        "rle8-icc-after-size-zero.bmp",
    ]
    .map(str::to_owned)
    .to_vec()
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
            .get("fixtures")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r.get("file").unwrap().as_str() == Some(name))
            .unwrap();
        let bytes = std::fs::read(fixtures().join(name)).unwrap();
        let pixels = std::fs::read(fixtures().join(format!("{name}.rgba"))).unwrap();
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&bytes)),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        assert_eq!(
            pixels,
            row.get("ffmpeg_rgba")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as u8)
                .collect::<Vec<_>>()
        );
        let size = (
            row.get("width").unwrap().as_i64().unwrap() as u32,
            row.get("height").unwrap().as_i64().unwrap() as u32,
        );
        let expected = retained_expected(&bytes, size, &pixels);
        Self {
            name: name.into(),
            bytes,
            pixels,
            size,
            expected,
        }
    }
    fn losses(&self) -> Vec<&'static str> {
        if !self.name.starts_with("rle") {
            return if self.name.contains("v40-") {
                vec![]
            } else {
                vec!["bmp-color-metadata-ignored"]
            };
        }
        let mut losses = vec!["palette-indices-expanded"];
        if self.name.contains("-icc-") {
            losses.extend(["icc-profile-ignored", "bmp-color-metadata-ignored"]);
        }
        if self.name.contains("size-zero") {
            losses.push("bmp-metadata-ignored");
        }
        losses
    }
    fn retained_losses(&self) -> Vec<&'static str> {
        self.losses()
            .into_iter()
            .map(|loss| match loss {
                "icc-profile-ignored" => "icc-source-retained-unapplied",
                other => other,
            })
            .collect()
    }
    fn assert_doc(&self, doc: &Document) {
        assert_eq!(doc.canvas, self.size, "{} imported once", self.name);
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

fn retained_expected(bytes: &[u8], size: (u32, u32), pixels: &[u8]) -> Document {
    use paintcraft_document::{RasterConversionContext, RasterConversionKey, RasterSelection};
    let source_hash: [u8; 32] = Sha256::digest(bytes).into();
    let budget = SharedBudget::new(1 << 20);
    let prepared = image::bmp_retention::decode(
        bytes,
        crate::editor::MAX_PIXELS,
        pcnd::MetadataAdmission {
            bytes: paintcraft_document::source_metadata::BYTES_MAX,
            budget: &budget,
        },
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!((prepared.image.width, prepared.image.height), size);
    assert_eq!(
        prepared.image.rgba, pixels,
        "independent source pixel oracle"
    );
    let dib = u32::from_le_bytes(bytes[14..18].try_into().unwrap());
    let profile = if dib == 124 {
        let at = 14 + u32::from_le_bytes(bytes[126..130].try_into().unwrap()) as usize;
        let len = u32::from_le_bytes(bytes[130..134].try_into().unwrap()) as usize;
        (len != 0).then(|| &bytes[at..at + len])
    } else {
        None
    };
    assert_eq!(
        prepared.attachment.is_some(),
        profile.is_some(),
        "zero-density fixture attachment"
    );
    if let Some(metadata) = &prepared.attachment {
        let facts = metadata.descriptor();
        assert_eq!(facts.source_sha256, source_hash);
        assert_eq!(facts.source_bytes, bytes.len() as u64);
        assert_eq!(facts.source_dimensions, size);
        assert_eq!(facts.output_dimensions, size);
        assert_eq!(
            facts.format,
            paintcraft_document::source_metadata::SourceFormat::Bmp
        );
        assert!(
            facts.resolution.is_none(),
            "these fixtures declare zero density"
        );
        assert_eq!(
            metadata.profile(),
            profile,
            "exact original V5 embedded profile"
        );
    }
    let key = RasterConversionKey::checked(
        RasterConversionContext {
            source_sha256: source_hash,
            profile: "bmp-source-profile-resolution-rgba8-v1",
            selection: RasterSelection::Image,
            assessment_sha256: prepared.attachment.as_ref().map(|m| m.digest()),
        },
        size.0,
        size.1,
        pixels,
        crate::editor::MAX_PIXELS,
        &|| Ok(()),
    )
    .unwrap();
    paintcraft_document::import_image(size.0, size.1, pixels, &key.hex())
        .unwrap()
        .with_source_metadata_checked(prepared.attachment, &|| Ok(()))
        .unwrap()
}

#[path = "bmp16_tests.rs"]
mod bmp16_tests;
fn adopt(case: &Case, frontend: Frontend, language: Language) -> WebApp {
    use paintcraft_classic::ClassicVariant;
    let loaded = decode_for_profile(&case.bytes, case.name.clone(), frontend).unwrap();
    assert!(loaded.raster_preview);
    assert_eq!(loaded.losses, case.retained_losses());
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
    for loss in case.retained_losses() {
        h.get_by_label(&translate(
            language,
            paintcraft_ui::i18n::import_loss_description(loss).unwrap(),
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
fn actual_bmp_rle_web_reviews_and_pcnd_reopen_keep_independent_pixels_and_shared_imported_ids() {
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
                && let Some(root) = std::env::var_os("PAINTCRAFT_BMP_RLE_CONSUMER_EVIDENCE")
            {
                let root = PathBuf::from(root).join("web");
                std::fs::create_dir_all(&root).unwrap();
                std::fs::write(root.join(format!("{name}.pcnd")), &bytes).unwrap();
            }
        }
    }
    for name in ["rle4-combined.bmp", "rle8-icc-after-size-zero.bmp"] {
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
fn bmp_rle_web_cancel_stale_sessions_profiles_and_drafts_preserve_prior_work() {
    let case = Case::new("rle8-icc-after-size-zero.bmp");
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
