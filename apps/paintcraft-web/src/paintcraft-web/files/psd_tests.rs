use super::*;
use crate::{app::WebApp, importing::Owner};
use eframe::egui;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{Language, set_language, translate};
use std::sync::Arc;

#[test]
fn photoshop_browser_import_requires_translated_consent_and_download_keeps_layers() {
    for (extension, bytes) in [
        (
            "psd",
            include_bytes!(
                "../../../../../crates/services/paintcraft-formats/tests/psd-fixtures/layers-3.psd"
            )
            .as_slice(),
        ),
        (
            "psb",
            include_bytes!(
                "../../../../../crates/services/paintcraft-formats/tests/psd-fixtures/layers-1.psb"
            )
            .as_slice(),
        ),
    ] {
        let name = format!("layers.{extension}");
        assert!(decode_input(bytes, name.clone(), Frontend::Studio, false).is_err());
        for classic in [
            Frontend::ClassicV7,
            Frontend::ClassicV10,
            Frontend::ClassicV11,
        ] {
            assert!(decode_input(bytes, name.clone(), classic, true).is_err());
        }
        for language in [Language::English, Language::Japanese] {
            let pick_name = name.clone();
            let mut h = Harness::builder()
                .with_size(egui::vec2(1360.0, 900.0))
                .build_eframe(move |cc| {
                    let mut app = WebApp::new(&cc.egui_ctx);
                    app.frontend = paintcraft_ui::actions::Frontend::Studio;
                    set_language(&cc.egui_ctx, language);
                    app.import_owner = Some(Owner::capture(&app));
                    app.accept_loaded(
                        decode_input(bytes, pick_name, Frontend::Studio, true).unwrap(),
                        &cc.egui_ctx,
                    );
                    app
                });
            h.run();
            let before = h.state().editor.session.snapshot();
            let ticket = h.state().editor.session.ticket();
            let description =
                paintcraft_ui::i18n::import_loss_description("psd-metadata-ignored").unwrap();
            assert!(
                h.query_by_label(&translate(language, description))
                    .is_some()
            );
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
                .click();
            h.run();
            assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
            assert_eq!(h.state().editor.session.ticket(), ticket);
            let ctx = h.ctx.clone();
            let app = h.state_mut();
            app.import_owner = Some(Owner::capture(app));
            app.accept_loaded(
                decode_input(bytes, name.clone(), Frontend::Studio, true).unwrap(),
                &ctx,
            );
            h.run();
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                .click();
            h.run();
            assert!(h.state().pending_import.is_none());
            let doc = h.state().editor.session.document();
            assert_eq!((doc.canvas, doc.roots.len()), ((5, 3), 2));
            let layer = &doc.layers[&doc.roots[0]];
            assert_eq!(
                (
                    layer.name.as_str(),
                    layer.origin,
                    layer.opacity,
                    layer.visible
                ),
                ("日本語 duplicate", (-1, 1), 127, false)
            );
            assert_eq!(layer.raster.get(0, 0).0, [9, 38, 67, 96]);
            assert_ne!(doc.roots[0], doc.roots[1]);
            assert!(h.state().editor.session.is_dirty());
            let output = encode_quality_for_profile(doc, None, 90, Frontend::Studio).unwrap();
            assert!(pcnd::read(&output).unwrap().same_semantics(doc));
        }
    }
}

#[test]
fn changed_browser_owner_and_unsupported_photoshop_layers_preserve_the_session() {
    let bytes = include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/psd-fixtures/layers-0.psd"
    );
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = paintcraft_ui::actions::Frontend::Studio;
    app.import_owner = Some(Owner::capture(&app));
    app.accept_loaded(
        decode_input(bytes, "layers.psd".into(), Frontend::Studio, true).unwrap(),
        &ctx,
    );
    app.frontend = paintcraft_ui::actions::Frontend::Classic;
    let before = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    app.resolve_import(&ctx, true);
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.ticket(), ticket);
    let mut unsupported = bytes.to_vec();
    let at = unsupported
        .windows(4)
        .position(|part| part == b"luni")
        .unwrap();
    unsupported[at..at + 4].copy_from_slice(b"TySh");
    assert!(decode_input(&unsupported, "text.psd".into(), Frontend::Studio, true).is_err());
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
}
