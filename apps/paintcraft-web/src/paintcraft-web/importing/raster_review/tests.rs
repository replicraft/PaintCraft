use super::*;
use crate::{WebApp, importing::Owner};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, set_language, translate};
use std::sync::Arc;

#[test]
fn browser_raster_review_shares_the_retained_profile_quota_before_allocating_preview_pixels() {
    let input = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd",
    ))
    .unwrap();
    let probe = paintcraft_formats::pcnd::read(&input).unwrap();
    let bytes = probe.source_metadata.as_ref().unwrap().storage_usage().1;
    drop(probe);
    let budget = SharedBudget::new(bytes + 7);
    let loaded = paintcraft_formats::pcnd::read_with_metadata_admission_checked(
        &input,
        16,
        65536,
        1,
        paintcraft_formats::pcnd::MetadataAdmission {
            bytes: paintcraft_document::source_metadata::BYTES_MAX,
            budget: &budget,
        },
        &|| Ok(()),
    )
    .unwrap();
    let raster = paintcraft_document::import_image(1, 1, &[1, 2, 3, 255], "edited-preview")
        .unwrap()
        .with_source_metadata_checked(loaded.source_metadata.clone(), &|| Ok(()))
        .unwrap();
    let ctx = eframe::egui::Context::default();
    let error = Review::new(&ctx, &raster, budget.clone()).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(budget.usage().reserved, bytes);
    drop(raster);
    drop(loaded);
    assert_eq!(budget.usage().reserved, 0);
}

fn raw_pixels(document: &Document) -> Vec<u8> {
    let layer = &document.layers[&document.roots[0]];
    let mut rgba = Vec::new();
    for y in 0..document.canvas.1 {
        for x in 0..document.canvas.0 {
            rgba.extend(layer.raster.get(i64::from(x), i64::from(y)).0);
        }
    }
    rgba
}

fn loaded(name: &str, profile: paintcraft_formats::registry::Frontend) -> crate::files::Loaded {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/services/paintcraft-formats/tests/raster-metadata-fixtures");
    crate::files::decode_for_profile(
        &std::fs::read(root.join(name)).unwrap(),
        name.into(),
        profile,
    )
    .unwrap()
}
fn oracle(name: &str) -> ((u32, u32), Vec<u8>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../crates/services/paintcraft-formats/tests/raster-metadata-fixtures/oracle.json",
    );
    let data = paintcraft_core::types::json::parse(&std::fs::read(path).unwrap()).unwrap();
    let row = data
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row.get("file").unwrap().as_str() == Some(name))
        .unwrap();
    let size = (
        row.get("width").unwrap().as_i64().unwrap() as u32,
        row.get("height").unwrap().as_i64().unwrap() as u32,
    );
    let pixels = row
        .get("expected_rgba8")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|sample| sample.as_i64().unwrap() as u8)
        .collect();
    (size, pixels)
}
/// Narrow regression setup: actual generic-raster dialog, rendered frames and
/// real checkbox input. It never supplies consent to layered/specialized imports.
pub(crate) fn authorize(app: &mut WebApp) -> egui::Context {
    assert!(app.pending_import.as_ref().unwrap().loaded.raster_preview);
    let mut h = Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui_state(
            |ui, app: &mut &mut WebApp| app.raster_import_dialog(ui.ctx()),
            app,
        );
    h.run();
    assert!(
        h.get_by_role_and_label(egui::Role::Button, "Open image")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Accept import limitations").click();
    h.run();
    assert!(
        !h.get_by_role_and_label(egui::Role::Button, "Open image")
            .accesskit_node()
            .is_disabled()
    );
    h.ctx.clone()
}
#[test]
fn hidden_clipped_and_first_visible_frames_cannot_authorize_and_copy_charge_drops() {
    for (width, height) in [(1, 1), (3, 2)] {
        let ctx = egui::Context::default();
        let pixels = [13, 71, 201, 0].repeat((width * height) as usize);
        let doc =
            paintcraft_document::import_image(width, height, &pixels, "preview-test").unwrap();
        let mut review = Review::new(&ctx, &doc, SharedBudget::new(SIDE * SIDE * 8)).unwrap();
        let budget = review._copy.budget();
        assert!(budget.usage().reserved > 0);
        review.consent = true;
        let mut rect = egui::Rect::NOTHING;
        for mode in 0..4 {
            let mut output = ctx.run_ui(Default::default(), |ui| {
                if mode == 0 {
                    ui.set_invisible();
                } else if mode == 1 {
                    ui.set_clip_rect(egui::Rect::from_min_size(
                        ui.cursor().min,
                        egui::vec2(10.0, 10.0),
                    ));
                }
                rect = review.preview(ui);
                assert_eq!(review.ready(ui.ctx()), mode == 3);
            });
            output.textures_delta.clear();
            if mode < 2 {
                assert!(review.shown.is_none());
            }
        }
        assert!((rect.width() - 200.0).abs() < 0.01);
        assert!((rect.height() - 200.0 * height as f32 / width as f32).abs() <= 0.5);
        assert_eq!(raw_pixels(&doc), pixels);
        drop(review);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn actual_png_bmp_imports_require_translated_checkbox_and_keep_exact_pixels_in_every_frontend() {
    use paintcraft_classic::ClassicVariant;
    use paintcraft_formats::registry::Frontend;
    for (variant, frontend) in [
        (ClassicVariant::V7, Frontend::ClassicV7),
        (ClassicVariant::V10, Frontend::ClassicV10),
        (ClassicVariant::V11, Frontend::ClassicV11),
        (ClassicVariant::V11, Frontend::Studio),
    ] {
        for language in [Language::English, Language::Japanese] {
            for name in [
                "rgba16-high-byte.png",
                "indexed2-adam7.png",
                "rgba8-exif-orientation.png",
                "rgb24-v5-icc.bmp",
            ] {
                let expected = loaded(name, frontend);
                let (canvas, rgba) = oracle(name);
                assert_eq!(expected.document.canvas, canvas);
                assert_eq!(raw_pixels(&expected.document), rgba);
                let bytes_name = name.to_owned();
                let mut h = Harness::builder()
                    .with_size(egui::vec2(1360.0, 1000.0))
                    .build_eframe(move |cc| {
                        let mut app = WebApp::new(&cc.egui_ctx);
                        app.variant = variant;
                        if frontend == Frontend::Studio {
                            app.frontend = paintcraft_ui::actions::Frontend::Studio;
                        }
                        set_language(&cc.egui_ctx, language);
                        app.import_owner = Some(Owner::capture(&app));
                        app.accept_loaded(loaded(&bytes_name, frontend), &cc.egui_ctx);
                        app
                    });
                h.run();
                let before = h.state().editor.session.snapshot();
                let ticket = h.state().editor.session.ticket();
                let budget = h
                    .state()
                    .pending_import
                    .as_ref()
                    .unwrap()
                    .review
                    .as_ref()
                    .unwrap()
                    ._copy
                    .budget();
                assert!(budget.usage().reserved > 0);
                assert!(
                    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                        .accesskit_node()
                        .is_disabled()
                );
                let ctx = h.ctx.clone();
                h.state_mut().resolve_import(&ctx, true);
                assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
                assert_eq!(ticket, h.state().editor.session.ticket());
                h.get_by_label(&translate(language, "Accept import limitations"))
                    .click();
                h.run();
                h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                    .click();
                h.run();
                assert!(h.state().pending_import.is_none());
                assert_eq!(h.state().editor.session.document().canvas, canvas);
                assert_eq!(raw_pixels(h.state().editor.session.document()), rgba);
                assert_eq!(budget.usage().reserved, 0);
            }
        }
    }
}
#[test]
fn cancel_and_stale_owner_release_preview_without_changing_the_existing_document() {
    use paintcraft_formats::registry::Frontend;
    for cancel in [false, true] {
        let ctx = egui::Context::default();
        let mut app = WebApp::new(&ctx);
        let before = app.editor.session.snapshot();
        let ticket = app.editor.session.ticket();
        app.import_owner = Some(Owner::capture(&app));
        app.accept_loaded(loaded("rgba16-high-byte.png", Frontend::ClassicV11), &ctx);
        let ctx = authorize(&mut app);
        let budget = app
            .pending_import
            .as_ref()
            .unwrap()
            .review
            .as_ref()
            .unwrap()
            ._copy
            .budget();
        if cancel {
            app.resolve_import(&ctx, false);
        } else {
            app.text.preedit = "composition".into();
            app.resolve_import(&ctx, true);
        }
        assert!(app.pending_import.is_none());
        assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
        assert_eq!(ticket, app.editor.session.ticket());
        assert_eq!(budget.usage().reserved, 0);
    }
}
