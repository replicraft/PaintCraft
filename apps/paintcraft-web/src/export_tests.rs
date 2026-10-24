use crate::WebApp;
use eframe::egui::{Role, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_classic::ClassicVariant;
use paintcraft_ui::{
    actions::ImageExport,
    i18n::{Language, format_message, set_language, translate},
};

#[test]
fn historical_browser_file_panel_keeps_format_quality_and_cancellation_in_every_language() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            for format in ImageExport::ALL {
                let mut h = Harness::builder()
                    .with_size(Vec2::new(1360.0, 900.0))
                    .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
                h.state_mut().variant = variant;
                h.state_mut().export_quality = 37;
                set_language(&h.ctx, language);
                h.run();
                let before = h.state().editor.session.snapshot();
                let ticket = h.state().editor.session.ticket();
                let label = |text| translate(language, text);
                h.get_by_role_and_label(Role::Button, &label("File"))
                    .click();
                h.run();
                h.get_by_role_and_label(Role::Button, &label("Save as"))
                    .click();
                h.run();
                h.get_by_role_and_label(Role::Button, &label(format.label()))
                    .click();
                h.run();
                let download = format_message(
                    language,
                    "Download { $format }",
                    &[("format", format.name().into())],
                );
                assert!(h.query_by_role_and_label(Role::Button, &download).is_some());
                if format == ImageExport::Jpeg {
                    assert!(
                        h.query_by_role_and_label(Role::Slider, &label("JPEG quality"))
                            .is_some()
                    );
                    assert!(
                        h.query_by_label(&label(
                            "JPEG uses lossy compression and cannot preserve transparency."
                        ))
                        .is_some()
                    );
                }
                h.get_by_label(&label("Cancel")).click();
                h.run();
                assert!(h.query_by_role_and_label(Role::Button, &download).is_none());
                assert!(h.state().editor.session.document().same_semantics(&before));
                assert_eq!(h.state().editor.session.ticket(), ticket);
                assert_eq!(h.state().export_quality, 37);
            }
        }
    }
}
