use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_core::types::{Rgba, filters::Descriptor};
use paintcraft_document::{FilterInstance, FilterStack};
const CHECK: &str = "Bake editable filters into this exported copy";
#[test]
fn actual_options_require_bake_even_for_disabled_filters_and_revoke_on_uncheck() {
    for enabled in [false, true] {
        let mut app = WebApp::new(&egui::Context::default());
        let mut doc = paintcraft_document::new_document(
            8,
            4,
            Some(Rgba([17, 31, 47, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                uuid::Uuid::from_u128(1),
                enabled,
                Descriptor::InvertRgbV1,
            )])
            .unwrap();
        app.editor = crate::editor::Editor::from_document(doc).unwrap();
        app.frontend = Frontend::Studio;
        app.export_confirm = Some(ImageExport::Png);
        let original = app.editor.session.snapshot();
        let ticket = app.editor.session.history_ticket();
        let mut h = Harness::builder()
            .with_size(egui::vec2(1200., 1000.))
            .build_ui_state(|ui, app: &mut WebApp| app.export_dialog(ui.ctx()), app);
        h.run();
        assert!(
            h.get_by_label("Download PNG")
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(CHECK).click();
        h.run();
        assert!(
            !h.get_by_label("Download PNG")
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(CHECK).click();
        h.run();
        assert!(
            h.get_by_label("Download PNG")
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label("Cancel").click();
        h.run();
        assert!(!h.state().export_bake_filters);
        assert!(h.state().export_frozen.is_none());
        assert!(std::sync::Arc::ptr_eq(
            &original,
            &h.state().editor.session.snapshot()
        ));
        assert_eq!(ticket, h.state().editor.session.history_ticket());
    }
}
