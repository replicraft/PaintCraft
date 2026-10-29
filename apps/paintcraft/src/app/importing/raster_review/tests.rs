use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, set_language, translate};

fn raw_pixels(document: &paintcraft_document::Document) -> Vec<u8> {
    let layer = &document.layers[&document.roots[0]];
    let mut rgba = Vec::new();
    for y in 0..document.canvas.1 {
        for x in 0..document.canvas.0 {
            rgba.extend(layer.raster.get(i64::from(x), i64::from(y)).0);
        }
    }
    rgba
}

/// Real generic-raster dialog, rendered frames and checkbox input only. This
/// setup cannot authorize layered or specialized imports.
pub(crate) fn authorize(app: &mut App) -> egui::Context {
    assert!(matches!(
        app.pending_import.as_ref().unwrap().candidate,
        Candidate::Raster(_)
    ));
    let mut h = Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui_state(|ui, app: &mut &mut App| app.import_dialog(ui.ctx()), app);
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
fn decoded(name: &str) -> image::Decoded {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/services/paintcraft-formats/tests/raster-metadata-fixtures");
    image::decode(&std::fs::read(root.join(name)).unwrap()).unwrap()
}
fn oracle(name: &str) -> ((u32, u32), Vec<u8>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../crates/services/paintcraft-formats/tests/raster-metadata-fixtures/oracle.json",
    );
    let data = paintcraft_core::types::json::parse(&std::fs::read(root).unwrap()).unwrap();
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
#[test]
fn preview_requires_hidden_clip_and_previous_frame_guards_and_releases_charge() {
    for (width, height) in [(1, 1), (3, 2)] {
        let ctx = egui::Context::default();
        let image = image::Decoded {
            format: ImageFormat::Png,
            width,
            height,
            rgba: [13, 71, 201, 0].repeat((width * height) as usize),
            losses: vec!["depth-16-reduced-to-8".into()],
        };
        let budget = SharedBudget::new(SIDE * SIDE * 8);
        let mut review = Review::new(&ctx, &image, budget.clone()).unwrap();
        assert!(budget.usage().reserved > 0);
        review.consent = true;
        let mut rectangle = egui::Rect::NOTHING;
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
                rectangle = review.preview(ui);
                assert_eq!(review.ready(ui.ctx()), mode == 3);
            });
            output.textures_delta.clear();
            if mode < 2 {
                assert!(review.shown.is_none());
            }
        }
        assert!((rectangle.width() - 200.0).abs() < 0.01);
        assert!((rectangle.height() - 200.0 * height as f32 / width as f32).abs() <= 0.5);
        assert_eq!(
            image.rgba,
            [13, 71, 201, 0].repeat((width * height) as usize)
        );
        drop(review);
        assert_eq!(budget.usage().reserved, 0);
        assert!(Review::new(&ctx, &image, SharedBudget::new(0)).is_err());
    }
}
#[test]
fn real_png_bmp_pixels_and_translated_consent_work_in_all_frontends() {
    // A separate live process-realm charge must not affect this review's exact
    // release assertion, including when other import tests run concurrently.
    let global = paintcraft_raster::Admission::new(0).paint_metadata_budget();
    let unrelated = global.reserve(280).unwrap();
    for (experience, variant) in [
        (Experience::Classic, ClassicVariant::V7),
        (Experience::Classic, ClassicVariant::V10),
        (Experience::Classic, ClassicVariant::V11),
        (Experience::Studio, ClassicVariant::V11),
    ] {
        for language in [Language::English, Language::Japanese] {
            for name in [
                "rgba16-high-byte.png",
                "indexed2-adam7.png",
                "rgba8-exif-orientation.png",
                "rgb24-v5-icc.bmp",
            ] {
                let image = decoded(name);
                let (canvas, pixels) = oracle(name);
                assert_eq!((image.width, image.height), canvas);
                assert_eq!(image.rgba, pixels);
                let mut app = App::new(None);
                app.source_metadata_budget = SharedBudget::new(SIDE * SIDE * 8);
                app.settings.experience = experience;
                app.settings.classic_variant = Some(variant);
                app.stage_import(image, PathBuf::from(name), "ab".repeat(32), true)
                    .unwrap();
                let mut h = Harness::builder()
                    .with_size(egui::vec2(1360.0, 1000.0))
                    .with_context_setup(move |ctx| set_language(ctx, language))
                    .build_ui_state(|ui, app: &mut App| app.import_dialog(ui.ctx()), app);
                h.run();
                let before = h.state().editor.document_snapshot();
                let ticket = h.state().editor.render_key();
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
                assert_eq!(budget.usage().limit, SIDE * SIDE * 8);
                assert!(budget.usage().reserved > 0);
                assert!(
                    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                        .accesskit_node()
                        .is_disabled()
                );
                let ctx = h.ctx.clone();
                h.state_mut().resolve_import(&ctx, true);
                assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
                assert_eq!(ticket, h.state().editor.render_key());
                h.get_by_label(&translate(language, "Accept import limitations"))
                    .click();
                h.run();
                h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                    .click();
                h.run();
                assert!(h.state().pending_import.is_none());
                assert_eq!(h.state().editor.document().canvas, canvas);
                assert!(h.state().bound.is_none());
                assert_eq!(raw_pixels(h.state().editor.document()), pixels);
                assert_eq!(budget.usage().reserved, 0);
            }
        }
    }
    assert!(global.usage().reserved >= 280);
    drop(unrelated);
}
#[test]
fn cancel_and_stale_owner_drop_only_private_review_charges() {
    let global = paintcraft_raster::Admission::new(0).paint_metadata_budget();
    let unrelated = global.reserve(280).unwrap();
    for cancel in [false, true] {
        let mut app = App::new(None);
        app.source_metadata_budget = SharedBudget::new(SIDE * SIDE * 8);
        let before = app.editor.document_snapshot();
        let ticket = app.editor.render_key();
        let bound = app.bound.clone();
        let source = app.imported_source.clone();
        app.stage_import(
            decoded("rgba16-high-byte.png"),
            PathBuf::from("source.png"),
            "ab".repeat(32),
            true,
        )
        .unwrap();
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
        assert_eq!(budget.usage().limit, SIDE * SIDE * 8);
        if cancel {
            app.resolve_import(&ctx, false);
        } else {
            app.settings.experience = if app.settings.experience == Experience::Studio {
                Experience::Classic
            } else {
                Experience::Studio
            };
            app.resolve_import(&ctx, true);
        }
        assert!(app.pending_import.is_none());
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert_eq!(ticket, app.editor.render_key());
        assert_eq!(app.bound, bound);
        assert_eq!(app.imported_source, source);
        assert_eq!(budget.usage().reserved, 0);
    }
    assert!(global.usage().reserved >= 280);
    drop(unrelated);
}

#[test]
#[ignore = "requires an explicit headless GPU screenshot directory"]
fn actual_headless_renderer_shows_tiny_import_pixels_before_consent() {
    let output = PathBuf::from(
        std::env::var_os("PAINTCRAFT_STOCK_PREVIEW_SCREENSHOT_DIRECTORY")
            .expect("explicit preview screenshot directory"),
    );
    std::fs::create_dir_all(&output).unwrap();
    for name in ["rgba16-high-byte.png", "indexed2-adam7.png"] {
        let mut app = App::new(None);
        app.settings.language = Language::English;
        app.stage_import(decoded(name), PathBuf::from(name), "ab".repeat(32), true)
            .unwrap();
        let mut h = Harness::builder()
            .with_size(egui::vec2(900.0, 700.0))
            .with_render_cursor(false)
            .build_ui_state(|ui, app: &mut App| app.import_dialog(ui.ctx()), app);
        h.run();
        assert!(
            h.get_by_role_and_label(egui::Role::Button, "Open image")
                .accesskit_node()
                .is_disabled()
        );
        let pixels = h.render().expect("headless egui renderer");
        let path = output.join(format!("{name}-preview-before-consent.png"));
        pixels.save(&path).unwrap();
        println!("Headless native preview: {}", path.display());
        assert!(h.state().pending_import.is_some());
    }
}
