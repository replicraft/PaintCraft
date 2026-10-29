use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{Language, set_language, translate};

#[test]
fn localized_jpeg_matte_control_is_format_specific_and_cancel_preserves_work() {
    for language in Language::ALL {
        let mut h = Harness::builder()
            .with_size(egui::vec2(1360., 1000.))
            .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        assert_eq!(h.state().export_jpeg_matte, [255; 3]);
        let source = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.history_ticket();
        let dirty = h.state().editor.session.is_dirty();
        h.state_mut().export_confirm = Some(ImageExport::Jpeg);
        h.state_mut().export_jpeg_matte = [32, 160, 224];
        set_language(&h.ctx, language);
        h.run();
        let tr = |source| translate(language, source);
        h.get_by_label(&tr("JPEG background color"));
        h.get_by_label(&tr("Transparent pixels use the selected background color."));
        h.get_by_label(&tr("Cancel")).click();
        h.run();
        assert!(h.state().export_confirm.is_none());
        assert!(std::sync::Arc::ptr_eq(
            &source,
            &h.state().editor.session.snapshot()
        ));
        assert_eq!(ticket, h.state().editor.session.history_ticket());
        assert_eq!(dirty, h.state().editor.session.is_dirty());
        h.state_mut().export_confirm = Some(ImageExport::Png);
        h.run();
        assert!(h.query_by_label(&tr("JPEG background color")).is_none());
        h.get_by_label(&tr("Cancel")).click();
        h.run();
        assert!(std::sync::Arc::ptr_eq(
            &source,
            &h.state().editor.session.snapshot()
        ));
    }
}
