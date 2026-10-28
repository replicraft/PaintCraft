use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::pen::{PressureCurve, Settings as PenSettings};

#[test]
fn desktop_pen_settings_are_staged_until_apply_without_document_changes() {
    for experience in [Experience::Classic, Experience::Studio] {
        let mut app = App::new(None);
        app.settings.experience = experience;
        app.open_settings(&egui::Context::default());
        let mut h = Harness::builder()
            .with_size(egui::vec2(1100.0, 800.0))
            .build_ui_state(
                |ui, app: &mut App| {
                    if let Some(Confirm::Settings(draft)) = app.confirm.clone() {
                        app.settings_dialog(ui.ctx(), draft);
                    }
                },
                app,
            );
        let ticket = h.state().editor.history_view().ticket;
        let before = h.state().editor.document_snapshot();
        h.run();
        h.get_by_label("Pen and tablet").click();
        h.run();
        h.get_by_label("Firm").click();
        h.run();
        h.get_by_label("Use pressure to control size").click();
        h.run();
        assert_eq!(h.state().settings.pen, PenSettings::default());
        assert_eq!(h.state().editor.options.pen, PenSettings::default());
        h.get_by_label("Cancel").click();
        h.run();
        assert_eq!(h.state().settings.pen, PenSettings::default());
        let ctx = h.ctx.clone();
        h.state_mut().open_settings(&ctx);
        h.run();
        h.get_by_label("Pen and tablet").click();
        h.run();
        h.get_by_label("Firm").click();
        h.run();
        h.get_by_label("Use pressure to control size").click();
        h.run();
        h.get_by_label("Apply").click();
        h.run();
        let expected = h.state().settings.pen;
        assert!(!expected.size_pressure);
        assert_eq!(expected.curve, PressureCurve::Firm);
        assert_eq!(h.state().settings.pen, expected);
        assert_eq!(h.state().editor.options.pen, expected);
        assert_eq!(h.state().editor.history_view().ticket, ticket);
        assert!(std::sync::Arc::ptr_eq(
            &before,
            &h.state().editor.document_snapshot()
        ));
        assert!(!h.state().editor.is_dirty());
    }
}
