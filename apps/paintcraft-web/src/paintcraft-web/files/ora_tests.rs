use super::*;
use crate::{app::WebApp, importing::Owner};
use eframe::egui;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{Language, import_loss_description, set_language, translate};
use std::sync::Arc;
const SOURCE: &[u8] = include_bytes!(
    "../../../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/neutral.ora"
);
const MERGED: &[u8] = include_bytes!(
    "../../../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/merged.rgba"
);
fn check(doc: &Document) {
    assert_eq!(doc.canvas, (3, 2));
    assert_eq!(doc.roots.len(), 6);
    assert_eq!(paintcraft_render::render_flat(doc).unwrap(), MERGED);
    assert_eq!(doc.layers[&doc.roots[1]].raster.get(0, 1).0, [81, 8, 13, 0]);
    assert!(!doc.layers[&doc.roots[2]].visible);
}
#[test]
fn browser_en_ja_group_review_adopts_immutable_pixels_only_with_current_owner_and_consent() {
    for language in [Language::English, Language::Japanese] {
        for name in ["layers.ora", "extensionless"] {
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
                        let mut bytes = SOURCE.to_vec();
                        let loaded =
                            decode_input(&bytes, name.into(), Frontend::Studio, true).unwrap();
                        bytes.fill(0);
                        app.accept_loaded(loaded, &cc.egui_ctx);
                        app
                    });
                h.run();
                let before = h.state().editor.session.snapshot();
                let ticket = h.state().editor.session.ticket();
                assert!(h.state().pending_import.is_some());
                h.get_by_label(&translate(
                    language,
                    import_loss_description(paintcraft_formats::ora::GROUP_STRUCTURE_LOSS).unwrap(),
                ));
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
                    check(doc);
                    assert!(h.state().editor.session.is_dirty());
                    let output =
                        encode_quality_for_profile(doc, None, 90, Frontend::Studio).unwrap();
                    check(&pcnd::read(&output).unwrap());
                } else {
                    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
                    assert_eq!(ticket, h.state().editor.session.ticket());
                    assert!(h.state().editor.session.is_dirty());
                }
            }
        }
    }
    for frontend in [
        Frontend::ClassicV7,
        Frontend::ClassicV10,
        Frontend::ClassicV11,
    ] {
        assert!(decode_input(SOURCE, "layers.ora".into(), frontend, true).is_err());
    }
    assert!(decode_input(SOURCE, "layers.ora".into(), Frontend::Studio, false).is_err());
    for source in [include_bytes!("../../../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/hidden-isolate.ora").as_slice(),include_bytes!("../../../../../crates/services/paintcraft-formats/tests/ora-group-fixtures/different-projection.ora"),b"PK\x03\x04unrelatedZip"]{assert!(decode_input(source,"extensionless".into(),Frontend::Studio,true).is_err());}
}
