use super::*;
use paintcraft::settings::{ClassicVariant, Settings};
use paintcraft_core::types::brush::BrushMedia;
#[test]
fn historical_ribbons_expose_picker_every_brush_and_image_transforms() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        let dir = scratch("legacy-features");
        Settings {
            classic_variant: Some(variant),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, true);
        h.set_size(Vec2::new(1480.0, 960.0));
        h.run();
        assert!(h.query_by_label("Remove background").is_none());
        assert!(h.get_by_label("Crop").accesskit_node().is_disabled());
        assert!(h.query_by_label_contains("Resize and skew").is_some());
        h.get_by_label("Color picker").click();
        h.run();
        assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::ColorPicker);
        for media in BrushMedia::ALL {
            h.get_by_role_and_label(eframe::egui::Role::Button, "Brushes")
                .click();
            h.run();
            let label = if media == BrushMedia::Plain {
                "Round brush"
            } else {
                media.name()
            };
            h.get_by_label(label).scroll_to_me();
            h.run();
            assert!(!h.get_by_label(label).accesskit_node().is_disabled());
            h.get_by_label(label).click();
            h.run();
            assert_eq!(h.state().editor().options.media, media);
        }
        h.get_by_label("Rotate").click();
        h.run();
        for label in [
            "Rotate right 90°",
            "Rotate left 90°",
            "Rotate 180°",
            "Flip vertical",
            "Flip horizontal",
        ] {
            assert!(!h.get_by_label(label).accesskit_node().is_disabled());
        }
        h.key_press(Key::Escape);
        h.run();
        h.get_by_label("Palette custom color 1").click();
        h.run();
        assert!(h.query_by_label("Basic colors").is_some());
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn historical_home_view_tabs_keep_compact_layout_in_both_languages_and_themes() {
    use eframe::egui::Role;
    use paintcraft::settings::Appearance;
    use paintcraft_ui::i18n::{Language, translate};
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for appearance in [Appearance::Light, Appearance::Dark] {
            for language in Language::ALL {
                let dir = scratch("legacy-tabs");
                Settings {
                    classic_variant: Some(variant),
                    appearance,
                    language,
                    ..Default::default()
                }
                .save(&dir)
                .unwrap();
                let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
                h.set_size(Vec2::new(1360.0, 900.0));
                h.run();
                let source = h.state().editor().document_snapshot();
                let label = |source| translate(language, source);
                assert!(
                    h.get_by_role_and_label(Role::Button, &label("Pencil"))
                        .rect()
                        .max
                        .y
                        < 160.0
                );
                assert!(
                    h.query_by_role_and_label(Role::Button, &label("Edit"))
                        .is_none()
                );
                assert!(h.query_by_label(&label("Clipboard")).is_some());
                assert!(
                    !h.get_by_role_and_label(Role::Button, &label("Stroke size"))
                        .accesskit_node()
                        .is_disabled()
                );
                h.get_by_role_and_label(Role::Button, &label("View"))
                    .click();
                h.run();
                assert!(
                    h.query_by_role_and_label(Role::CheckBox, &label("Rulers"))
                        .is_some()
                );
                assert!(
                    h.query_by_role_and_label(Role::CheckBox, &label("Gridlines"))
                        .is_some()
                );
                h.get_by_role_and_label(Role::Button, &label("Home"))
                    .click();
                h.run();
                assert!(
                    h.query_by_role_and_label(Role::Button, &label("Pencil"))
                        .is_some()
                );
                assert!(h.state().editor().document().same_semantics(&source));
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}
