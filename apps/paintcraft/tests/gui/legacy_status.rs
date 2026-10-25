use super::*;
use paintcraft::settings::{ClassicVariant, Settings};
use paintcraft_ui::i18n::{Language, translate};

#[test]
fn historical_status_zoom_controls_work_at_both_scales_without_document_edits() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            for ppp in [1.0, 2.0] {
                let dir = scratch("legacy-status");
                Settings {
                    classic_variant: Some(variant),
                    language,
                    ..Default::default()
                }
                .save(&dir)
                .unwrap();
                let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, false);
                h.set_size(Vec2::new(1360.0, 900.0));
                h.run();
                let source = h.state().editor().document_snapshot();
                let ticket = h.state().editor().history_view().ticket;
                let label = |source| translate(language, source);
                assert!(h.get_by_label(&label("Canvas zoom")).rect().height() <= 22.0);
                h.get_by_label(&label("Canvas zoom"))
                    .set_value_accesskit(200.0);
                h.run();
                assert!((h.state().canvas_rect().unwrap().width() - 2000.0).abs() < 0.1);
                h.get_by_label(&label("Zoom percentage")).click();
                h.run();
                h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
                h.run();
                h.get_by_label(&label("Zoom percentage")).type_text("73%");
                h.run();
                h.key_press(Key::Enter);
                h.run();
                assert!((h.state().canvas_rect().unwrap().width() - 730.0).abs() < 0.1);
                assert!(h.state().editor().document().same_semantics(&source));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}
