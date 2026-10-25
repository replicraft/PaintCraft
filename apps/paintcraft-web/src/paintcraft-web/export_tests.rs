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
            for format in ImageExport::ALL
                .into_iter()
                .filter(|format| format.available(paintcraft_ui::actions::Frontend::Classic))
            {
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

#[test]
fn studio_export_dialog_offers_stock_raster_additions_and_classic_keeps_its_own_inventory() {
    for studio in [false, true] {
        let mut h = Harness::builder()
            .with_size(Vec2::new(1360.0, 900.0))
            .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
        if studio {
            h.state_mut().frontend = paintcraft_ui::actions::Frontend::Studio;
        }
        h.run();
        let choices = h.state().enabled_export_formats();
        for format in [ImageExport::Gif, ImageExport::Tiff] {
            assert!(choices.contains(&format));
        }
        for format in [
            ImageExport::Tga,
            ImageExport::Webp,
            ImageExport::Jxl,
            ImageExport::Dds,
            ImageExport::Avif,
        ] {
            assert_eq!(choices.contains(&format), studio);
        }
        let source = h.state().editor.session.snapshot();
        h.get_by_role_and_label(Role::Button, "File").click();
        h.run();
        if studio {
            h.get_by_label("Export…").click();
        } else {
            h.get_by_label("Save as").hover();
            h.run();
            h.get_by_label("Picture…").click();
        }
        h.run();
        h.get_by_label("File type").click();
        h.run();
        assert_eq!(h.query_by_label("WebP").is_some(), studio);
        h.get_by_label("GIF").click();
        h.run();
        assert!(
            h.query_by_label("Save a palette picture. Colors and transparency may be reduced.")
                .is_some()
        );
        if studio {
            h.get_by_label("File type").click();
            h.run();
            h.get_by_label("AVIF").scroll_to_me();
            h.run();
            h.get_by_label("AVIF").click();
            h.run();
            assert!(
                h.query_by_role_and_label(Role::Slider, "AVIF quality")
                    .is_some()
            );
            assert!(
                h.query_by_label(
                    "AVIF uses lossy compression for colors and transparency, even at quality 100."
                )
                .is_some()
            );
        }
        h.get_by_label("Cancel").click();
        h.run();
        assert!(h.state().editor.session.document().same_semantics(&source));
    }
}
