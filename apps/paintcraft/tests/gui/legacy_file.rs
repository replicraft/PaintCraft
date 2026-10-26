use super::*;
use paintcraft::settings::{ClassicVariant, Settings};
use paintcraft_ui::{
    actions::ImageExport,
    i18n::{Language, translate},
};

fn choose_format(h: &mut Harness<App>, language: Language, format: ImageExport) {
    h.get_by_role_and_label(eframe::egui::Role::Button, &translate(language, "File"))
        .click();
    h.run();
    h.get_by_role_and_label(eframe::egui::Role::Button, &translate(language, "Save as"))
        .click();
    h.run();

    h.get_by_role_and_label(
        eframe::egui::Role::Button,
        &translate(language, format.label()),
    )
    .scroll_to_me();
    h.run();
    h.get_by_role_and_label(
        eframe::egui::Role::Button,
        &translate(language, format.label()),
    )
    .click();
    h.run();
}

#[test]
fn historical_save_as_choices_export_real_formats_only_after_confirmation() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            for format in ImageExport::ALL
                .into_iter()
                .filter(|format| format.available(paintcraft_ui::actions::Frontend::Classic))
            {
                let dir = scratch("historical-file-export");
                let source = dir.join("original.pcnd");
                let document = paintcraft_document::new_document(
                    24,
                    16,
                    Some(Rgba([20, 80, 140, 255])),
                    0,
                    paintcraft_document::POLICY,
                )
                .unwrap();
                std::fs::write(&source, paintcraft_formats::pcnd::write(&document).unwrap())
                    .unwrap();
                Settings {
                    classic_variant: Some(variant),
                    language,
                    ..Default::default()
                }
                .save(&dir)
                .unwrap();
                let mut h = harness_with(
                    App::new(Some(source.clone())).with_settings(dir.clone()),
                    1.0,
                    true,
                );
                let before = h.state().editor().document_snapshot();
                let ticket = h.state().editor().history_view().ticket;
                let out = dir.join("picture").with_extension(format.extension());
                std::fs::write(&out, b"existing picture").unwrap();
                h.state_mut()
                    .set_path_text(&dir.join("picture.pcnd").to_string_lossy());
                choose_format(&mut h, language, format);
                assert_eq!(std::fs::read(&out).unwrap(), b"existing picture");
                h.get_by_label(&translate(language, "Cancel")).click();
                h.run();
                assert_eq!(std::fs::read(&out).unwrap(), b"existing picture");
                choose_format(&mut h, language, format);
                h.get_by_label(&translate(language, "Choose destination…"))
                    .click();
                h.run();
                assert_eq!(std::fs::read(&out).unwrap(), b"existing picture");
                h.get_by_label(&translate(language, "Cancel")).click();
                h.run();
                assert_eq!(std::fs::read(&out).unwrap(), b"existing picture");
                choose_format(&mut h, language, format);
                h.get_by_label(&translate(language, "Choose destination…"))
                    .click();
                h.run();
                assert_eq!(std::fs::read(&out).unwrap(), b"existing picture");
                h.get_by_label(&translate(language, "Export now")).click();
                h.run();
                let bytes = std::fs::read(&out).unwrap();
                assert_eq!(
                    paintcraft_formats::image::ImageFormat::sniff(&bytes)
                        .unwrap()
                        .name(),
                    format.name()
                );
                let decoded = paintcraft_formats::image::decode(&bytes).unwrap();
                assert_eq!((decoded.width, decoded.height), (24, 16));
                assert!(decoded.rgba.as_chunks::<4>().0.iter().all(|p| {
                    p[3] == 255
                        && p[..3]
                            .iter()
                            .zip([20_u8, 80, 140])
                            .all(|(actual, expected)| {
                                actual.abs_diff(expected)
                                    <= if format == ImageExport::Jpeg { 4 } else { 0 }
                            })
                }));
                assert!(h.state().editor().document().same_semantics(&before));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
                assert_eq!(h.state().editor().path(), Some(source.as_path()));
                assert!(!h.state().editor().is_dirty());
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}

#[test]
fn historical_file_panel_scrolls_reopens_dispatches_gif_and_refuses_studio_webp() {
    for (variant, frontend) in [
        (
            ClassicVariant::V7,
            paintcraft_formats::registry::Frontend::ClassicV7,
        ),
        (
            ClassicVariant::V10,
            paintcraft_formats::registry::Frontend::ClassicV10,
        ),
    ] {
        let dir = scratch("historical-file-layout");
        Settings {
            classic_variant: Some(variant),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(640.0, 300.0));
        h.run();
        h.get_by_role_and_label(eframe::egui::Role::Button, "File")
            .click();
        h.run();
        h.get_by_role_and_label(eframe::egui::Role::Button, "Save as")
            .click();
        h.run();
        h.get_by_label("GIF picture").scroll_to_me();
        h.run();
        assert!(!h.get_by_label("GIF picture").accesskit_node().is_disabled());
        assert!(h.query_by_label("WebP picture").is_none());
        assert!(
            !paintcraft_formats::registry::admission(paintcraft_formats::registry::Request::new(
                frontend,
                paintcraft_formats::registry::Operation::ExportFlat,
                paintcraft_formats::registry::Family::Webp,
                paintcraft_formats::registry::Platform::Native,
            ),)
            .available()
        );
        let before = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        h.get_by_role_and_label(eframe::egui::Role::Button, "GIF picture")
            .click();
        h.run();
        h.get_by_label("Choose destination…");
        h.get_by_role_and_label(eframe::egui::Role::Button, "Cancel")
            .click();
        h.run();
        assert!(h.state().editor().document().same_semantics(&before));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        h.get_by_role_and_label(eframe::egui::Role::Button, "File")
            .click();
        h.run();
        h.get_by_role_and_label(eframe::egui::Role::Button, "Save as")
            .click();
        h.run();
        h.get_by_label("PaintCraft project").scroll_to_me();
        h.run();
        let rect = h.get_by_label("PaintCraft project").rect();
        assert!(rect.top() >= 0.0 && rect.bottom() <= 300.0);
        h.key_press(Key::Escape);
        h.run();
        h.get_by_role_and_label(eframe::egui::Role::Button, "File")
            .click();
        h.run();
        assert!(h.query_by_label("Recent pictures").is_some());
        h.get_by_label("Exit").scroll_to_me();
        h.run();
        let exit = h.get_by_label("Exit").rect();
        assert!(exit.top() >= 0.0 && exit.bottom() <= 300.0);

        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit offscreen historical File panel capture"]
fn export_historical_file_panels() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("paintcraft-file-panels"));
    std::fs::create_dir_all(&output).unwrap();
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            let dir = scratch("historical-file-capture");
            Settings {
                classic_variant: Some(variant),
                language,
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
            h.set_size(Vec2::new(1060.0, 720.0));
            h.run();
            h.get_by_role_and_label(eframe::egui::Role::Button, &translate(language, "File"))
                .click();
            h.run();
            h.get_by_role_and_label(eframe::egui::Role::Button, &translate(language, "Save as"))
                .hover();
            h.run();
            assert!(
                h.query_by_role_and_label(
                    eframe::egui::Role::Button,
                    &translate(language, "PNG picture")
                )
                .is_some()
            );
            h.render()
                .unwrap()
                .save(output.join(format!(
                    "classic-{}-file-{}.png",
                    variant.id(),
                    language.id()
                )))
                .unwrap();
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
