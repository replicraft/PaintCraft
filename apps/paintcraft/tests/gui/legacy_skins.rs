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
