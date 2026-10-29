use super::*;
use crate::{app::WebApp, importing::Owner};
use eframe::egui;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{Language, set_language, translate};
use std::sync::Arc;
const SOURCE: &[u8] = include_bytes!(
    "../../../../../crates/services/paintcraft-formats/tests/dds-mip-fixtures/dx10-rgba.dds"
);
const TOP: &[u8] = include_bytes!(
    "../../../../../crates/services/paintcraft-formats/tests/dds-mip-fixtures/top.rgba"
);
fn pixels(doc: &Document) {
    assert_eq!(doc.canvas, (7, 5));
    for (i, p) in TOP.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            doc.layers[&doc.roots[0]]
                .raster
                .get((i % 7) as i64, (i / 7) as i64)
                .0,
            *p
        );
    }
}
#[test]
fn actual_browser_dds_open_import_review_preserves_nonempty_document_on_cancel_and_stale() {
    for language in [Language::English, Language::Japanese] {
        for convert in [false, true] {
            for case in 0..4 {
                let mut h = Harness::builder()
                    .with_size(egui::vec2(1360., 900.))
                    .build_eframe(move |cc| {
                        let mut app = WebApp::new(&cc.egui_ctx);
                        app.frontend = paintcraft_ui::actions::Frontend::Studio;
                        set_language(&cc.egui_ctx, language);
                        let doc = paintcraft_document::import_image(
                            2,
                            1,
                            &[9, 21, 33, 255, 88, 42, 13, 128],
                            &"0".repeat(64),
                        )
                        .unwrap();
                        app.editor = crate::editor::Editor::from_document(doc).unwrap();
                        app.editor.session.mark_unsaved();
                        app.import_owner = Some(Owner::capture(&app));
                        app.accept_loaded(
                            decode_input(SOURCE, "texture.dds".into(), Frontend::Studio, convert)
                                .unwrap(),
                            &cc.egui_ctx,
                        );
                        app
                    });
                h.run();
                let before = h.state().editor.session.snapshot();
                let ticket = h.state().editor.session.ticket();
                assert!(h.state().pending_import.is_some());
                h.get_by_label(&translate(language,"Only the full-size DDS image will be imported. Its smaller mip levels will stay in the source file."));
                if case != 1 {
                    h.get_by_label(&translate(language, "Accept import limitations"))
                        .click();
                    h.run();
                }
                match case {
                    2 => h.state_mut().frontend = paintcraft_ui::actions::Frontend::Classic,
                    3 => h.state_mut().text.preedit = "draft".into(),
                    _ => {}
                }
                h.get_by_role_and_label(
                    egui::Role::Button,
                    &translate(language, if case == 1 { "Cancel" } else { "Open image" }),
                )
                .click();
                h.run();
                assert!(h.state().pending_import.is_none());
                if case == 0 {
                    let doc = h.state().editor.session.document();
                    pixels(doc);
                    assert!(h.state().editor.session.is_dirty());
                    let bytes =
                        encode_quality_for_profile(doc, None, 90, Frontend::Studio).unwrap();
                    pixels(&pcnd::read(&bytes).unwrap());
                } else {
                    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
                    assert_eq!(ticket, h.state().editor.session.ticket());
                    assert!(h.state().editor.session.is_dirty());
                }
            }
        }
    }
    for convert in [false, true] {
        assert!(decode_input(SOURCE, "texture.dds".into(), Frontend::ClassicV11, convert).is_err());
        let loaded = decode_input(SOURCE, "texture".into(), Frontend::Studio, convert).unwrap();
        assert!(loaded.converted_copy);
        assert_eq!(loaded.losses, ["dds-other-mips-not-imported"]);
    }
}
