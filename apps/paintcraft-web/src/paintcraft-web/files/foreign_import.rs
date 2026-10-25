use crate::{app::WebApp, importing::Owner};
use eframe::egui;
use std::sync::Arc;

#[test]
fn pdn_browser_open_requires_translated_consent_and_download_keeps_real_layers() {
    use egui_kittest::{Harness, kittest::Queryable};
    use paintcraft_formats::{pcnd, registry::Frontend};
    use paintcraft_ui::i18n::{Language, set_language, translate};
    use sha2::{Digest, Sha256};
    const BYTES: &[u8] = include_bytes!(
        "../../../../../crates/adapters/pdn-lib/tests/fixtures/paintdotjs-2-layer.pdn"
    );
    for language in [Language::English, Language::Japanese] {
        let mut h = Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .build_eframe(move |cc| {
                let mut app = WebApp::new(&cc.egui_ctx);
                app.frontend = paintcraft_ui::actions::Frontend::Studio;
                set_language(&cc.egui_ctx, language);
                app.import_owner = Some(Owner::capture(&app));
                app.accept_loaded(
                    crate::files::decode_for_profile(BYTES, "layers.pdn".into(), Frontend::Studio)
                        .unwrap(),
                    &cc.egui_ctx,
                );
                app
            });
        h.run();
        let before = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.ticket();
        assert!(h.state().pending_import.is_some());
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
            .click();
        h.run();
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert_eq!(ticket, h.state().editor.session.ticket());
        let ctx = h.ctx.clone();
        let app = h.state_mut();
        app.import_owner = Some(Owner::capture(app));
        app.accept_loaded(
            crate::files::decode_for_profile(BYTES, "layers.pdn".into(), Frontend::Studio).unwrap(),
            &ctx,
        );
        h.run();
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .click();
        h.run();
        let doc = h.state().editor.session.document();
        assert_eq!(doc.canvas, (3, 2));
        assert_eq!(doc.roots.len(), 2);
        assert!(h.state().editor.session.is_dirty());
        for (id, name, visible, opacity, expected) in [
            (
                doc.roots[0],
                "Base 日本語",
                true,
                201,
                "d6931877bd07347276a7149f0ff06c97a16de2b64c416385f72e649facad7c56",
            ),
            (
                doc.roots[1],
                "Overlay é",
                false,
                73,
                "4f570b69e8ee688b9e255dab9f8842d18f632314066f17580b70460852dc50c8",
            ),
        ] {
            let layer = &doc.layers[&id];
            assert_eq!(
                (&*layer.name, layer.visible, layer.opacity),
                (name, visible, opacity)
            );
            let mut hash = Sha256::new();
            for y in 0..2 {
                for x in 0..3 {
                    hash.update(layer.raster.get(x, y).0);
                }
            }
            assert_eq!(paintcraft_document::hex(&hash.finalize()), expected);
        }
        let output =
            crate::files::encode_quality_for_profile(doc, None, 90, Frontend::Studio).unwrap();
        assert!(pcnd::read(&output).unwrap().same_semantics(doc));
    }
    assert!(
        crate::files::decode_for_profile(BYTES, "layers.pdn".into(), Frontend::ClassicV11).is_err()
    );
}
