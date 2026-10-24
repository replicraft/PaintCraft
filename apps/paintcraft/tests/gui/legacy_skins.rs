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

#[test]
fn historical_shape_arrows_and_expanded_gallery_reach_last_shapes_without_edits() {
    use eframe::egui::Role;
    use paintcraft::settings::Appearance;
    use paintcraft_tools::{ShapePreset, Tool};
    use paintcraft_ui::i18n::{Language, translate};
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
                let dir = scratch("historical-shape-controls");
                Settings {
                    classic_variant: Some(variant),
                    language,
                    appearance,
                    ..Default::default()
                }
                .save(&dir)
                .unwrap();
                let mut h =
                    harness_with(App::new(None).with_settings(dir.clone()), ppp, ppp == 2.0);
                h.set_size(Vec2::new(1480.0, 960.0));
                h.run();
                let snapshot = h.state().editor().document_snapshot();
                let ticket = h.state().editor().history_view().ticket;
                let label = |s| translate(language, s);
                assert!(
                    h.get_by_label(&label("Previous shapes"))
                        .accesskit_node()
                        .is_disabled()
                );
                h.get_by_label(&label("Next shapes")).click();
                h.run();
                assert!(
                    !h.get_by_label(&label("Previous shapes"))
                        .accesskit_node()
                        .is_disabled()
                );
                h.get_by_role_and_label(Role::Button, &label("Lightning"))
                    .click();
                h.run();
                assert_eq!(
                    h.state().editor().tool,
                    Tool::Preset(ShapePreset::Lightning)
                );
                h.get_by_label(&label("Previous shapes")).click();
                h.run();
                assert!(
                    h.get_by_label(&label("Previous shapes"))
                        .accesskit_node()
                        .is_disabled()
                );
                h.get_by_label(&label("Shapes…")).click();
                h.run();
                h.query_all_by_label(&label("Heart"))
                    .last()
                    .unwrap()
                    .click();
                h.run();
                assert_eq!(h.state().editor().tool, Tool::Preset(ShapePreset::Heart));
                assert_eq!(h.query_all_by_label(&label("Heart")).count(), 1);
                assert!(h.state().editor().document().same_semantics(&snapshot));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}

#[test]
#[ignore = "Explicit offscreen historical shape chooser capture"]
fn export_historical_shape_chooser() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("paintcraft-historical-shapes"));
    std::fs::create_dir_all(&output).unwrap();
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        let dir = scratch("historical-shapes-capture");
        Settings {
            classic_variant: Some(variant),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(1100.0, 720.0));
        h.run();
        h.get_by_label("Shapes…").click();
        h.run();
        h.run_steps(4);
        assert!(
            h.output()
                .platform_output
                .request_discard_reasons
                .is_empty()
        );
        assert!(!h.output().shapes.iter().any(|shape| matches!(&shape.shape,
            eframe::egui::epaint::Shape::Text(text) if text.galley.job.text.contains("PERF WARNING"))));
        h.render()
            .unwrap()
            .save(output.join(format!("classic-{}-shapes.png", variant.id())))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
