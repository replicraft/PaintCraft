use super::*;
use crate::{WebApp, importing::Owner};

#[test]
fn browser_download_encoder_keeps_selected_palette_policy_and_classic_admission() {
    use paintcraft_ui::pcx_export::Encoding;
    let rgba: Vec<u8> = (0..400u32)
        .flat_map(|i| [i as u8, (i >> 8) as u8, i.wrapping_mul(53) as u8, 255])
        .collect();
    let doc = paintcraft_document::import_image(40, 10, &rgba, "web-pcx-options").unwrap();
    assert!(
        encode_options_for_profile(
            &doc,
            Some(ImageExport::Pcx),
            90,
            Frontend::Studio,
            Encoding::IndexedExact
        )
        .is_err()
    );
    let bytes = encode_options_for_profile(
        &doc,
        Some(ImageExport::Pcx),
        90,
        Frontend::Studio,
        Encoding::Indexed256,
    )
    .unwrap();
    assert_eq!(bytes[65], 1);
    let expected = paintcraft_formats::pcx::prepare_rgba(
        40,
        10,
        &rgba,
        paintcraft_formats::pcx::Mode::Quantized256,
        &|| Ok(()),
    )
    .unwrap()
    .preview(40, &|| Ok(()))
    .unwrap();
    assert_eq!(image::decode(&bytes).unwrap().rgba, expected.rgba);
    for frontend in [
        Frontend::ClassicV7,
        Frontend::ClassicV10,
        Frontend::ClassicV11,
    ] {
        assert!(
            encode_options_for_profile(
                &doc,
                Some(ImageExport::Pcx),
                90,
                frontend,
                Encoding::Indexed256
            )
            .is_err()
        );
    }
}

#[test]
fn openraster_explicit_conversion_requires_consent_before_publishing_layers() {
    let bytes = include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/ora-fixtures/deflated.ora"
    );
    assert!(decode_input(bytes, "layers.ora".into(), Frontend::Studio, false).is_err());
    assert!(decode_input(bytes, "layers.ora".into(), Frontend::ClassicV11, true).is_err());
    let ctx = eframe::egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = paintcraft_ui::actions::Frontend::Studio;
    let before = app.editor.session.snapshot();
    for accept in [false, true] {
        let loaded = decode_input(bytes, "layers.ora".into(), Frontend::Studio, true).unwrap();
        app.import_owner = Some(Owner::capture(&app));
        app.accept_loaded(loaded, &ctx);
        assert!(app.pending_import.is_some());
        assert!(app.editor.session.document().same_semantics(&before));
        app.resolve_import(&ctx, accept);
    }
    let doc = app.editor.session.document();
    assert_eq!(doc.roots.len(), 3);
    assert_eq!(doc.layers[&doc.roots[1]].origin, (1, 1));
    assert!(!doc.layers[&doc.roots[2]].visible);
    assert!(app.editor.session.is_dirty());
}
const PAINT: &[u8] = include_bytes!(
    "../../../../../crates/adapters/paint-lib/tests/fixtures/mspaintlib-sample.paint"
);
#[test]
fn layered_paint_conversion_preserves_source_until_consent_and_remains_unbound() {
    assert!(decode_input(PAINT, "sample.paint".into(), Frontend::Studio, false).is_err());
    assert!(decode_input(PAINT, "sample.paint".into(), Frontend::ClassicV7, true).is_err());
    let ctx = eframe::egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = paintcraft_ui::actions::Frontend::Studio;
    let before = app.editor.session.snapshot();
    for accept in [false, true] {
        let loaded = decode_input(PAINT, "sample.paint".into(), Frontend::Studio, true).unwrap();
        assert_eq!(loaded.document.roots.len(), 5);
        assert_eq!(
            loaded.losses,
            paintcraft_formats::project_import::PAINT_LOSSES
        );
        app.import_owner = Some(Owner::capture(&app));
        app.accept_loaded(loaded, &ctx);
        assert!(app.pending_import.is_some());
        assert!(app.editor.session.document().same_semantics(&before));
        app.resolve_import(&ctx, accept);
        if !accept {
            assert!(app.editor.session.document().same_semantics(&before));
        }
    }
    assert_eq!(app.editor.session.document().roots.len(), 5);
    assert!(app.editor.session.is_dirty());
    assert_eq!(app.editor.session.document().canvas, (333, 271));
}
