//! Empty document names remain valid and must still expose an actionable row.
use super::*;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn unnamed_layer_rows_remain_accessible_in_classic_and_studio() {
    for experience in [Experience::Classic, Experience::Studio] {
        for name in ["", "   "] {
            let mut app = App::new(None);
            app.settings.experience = experience;
            let layer = app.editor.target().unwrap();
            app.editor.request(
                9001,
                Action::Rename {
                    layer,
                    name: name.to_owned(),
                },
            );
            assert_eq!(app.editor.run_pending(), 1);
            let before = app.editor.document_snapshot();
            let depth = app.editor.undo_depth();
            let mut harness = Harness::builder()
                .with_size(Vec2::new(700.0, 800.0))
                .build_ui_state(
                    |ui, app: &mut App| {
                        if ui.ctx().missing_glyph_policy()
                            != egui::epaint::text::MissingGlyphPolicy::Tofu
                        {
                            paintcraft_ui::fonts::install(ui.ctx());
                            ui.ctx().set_missing_glyph_policy(
                                egui::epaint::text::MissingGlyphPolicy::Tofu,
                            );
                        }
                        paintcraft_ui::i18n::set_language(
                            ui.ctx(),
                            paintcraft_ui::i18n::Language::English,
                        );
                        app.layer_panel(ui);
                    },
                    app,
                );
            harness.run();
            harness.get_by_role_and_label(egui::Role::Button, "Layer");
            harness.get_by_role_and_label(egui::Role::CheckBox, "Show Layer");
            assert!(harness.state().editor.document().same_semantics(&before));
            assert_eq!(harness.state().editor.undo_depth(), depth);
        }
    }
}
