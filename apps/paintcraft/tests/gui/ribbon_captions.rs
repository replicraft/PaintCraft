use super::*;
use eframe::egui::{Role, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

#[test]
fn classic_ribbon_has_legible_single_captions_and_icon_only_primary_tools() {
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("ribbon-caption-metrics");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for label in [
            "Pencil",
            "Fill",
            "Text",
            "Eraser",
            "Color picker",
            "Magnifier",
        ] {
            let rect = h.get_by_role_and_label(Role::Button, label).rect();
            assert_eq!(rect.size(), Vec2::splat(40.0), "{label}");
            assert!(
                h.output()
                    .shapes
                    .iter()
                    .any(|shape| matches!(&shape.shape, Shape::Rect(image)
                if image.brush.is_some() && image.rect.size() == Vec2::splat(24.0)
                    && rect.contains_rect(image.rect) && image.rect.center() == rect.center()
                    && image.fill.r() == image.fill.g() && image.fill.g() == image.fill.b()
                    && (matches!(label, "Color picker" | "Magnifier") || image.fill == eframe::egui::Color32::WHITE))),
                "{label} must use centered tool artwork"
            );
        }
        for (label, tool) in [
            ("Text", paintcraft_tools::Tool::Text),
            ("Fill", paintcraft_tools::Tool::Fill),
            ("Eraser", paintcraft_tools::Tool::Eraser),
            ("Pencil", paintcraft_tools::Tool::Pencil),
        ] {
            h.get_by_role_and_label(Role::Button, label).click();
            h.run();
            assert_eq!(h.state().editor().tool, tool);
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
        }
        for caption in [
            "Selection",
            "Image",
            "Tools",
            "Brushes",
            "Shapes",
            "Colors",
            "Layers",
        ] {
            let text: Vec<_> = h
                .output()
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    Shape::Text(text) if text.galley.job.text == caption => Some(text),
                    _ => None,
                })
                .collect();
            assert_eq!(text.len(), 1, "one painted {caption} caption");
            let format = &text[0].galley.job.sections[0].format;
            assert_eq!(format.font_id.size, 14.0, "{caption}");
            if appearance == Appearance::Dark {
                assert!(
                    format.color.r() >= 180,
                    "{caption} needs a readable dark-theme caption"
                );
            } else {
                assert!(
                    format.color.r() <= 90,
                    "{caption} needs a readable light-theme caption"
                );
            }
        }
        for (label, tool) in [
            ("Select", paintcraft_tools::Tool::Select),
            ("Brush", paintcraft_tools::Tool::Brush),
        ] {
            let rect = h.get_by_role_and_label(Role::Button, label).rect();
            assert_eq!(rect.size(), Vec2::splat(48.0));
            assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape, Shape::Rect(image)
                if image.brush.is_some() && image.rect.size() == Vec2::splat(32.0) && rect.contains_rect(image.rect) && image.rect.center() == rect.center()
                && image.fill.r() == image.fill.g() && image.fill.g() == image.fill.b()
                && (if appearance == Appearance::Dark {image.fill.r() >= 220} else {image.fill.r() <= 40}))));
            assert!(!h.output().shapes.iter().any(|shape| matches!(&shape.shape, Shape::Text(text)
                if text.galley.job.text == label && rect.contains_rect(text.visual_bounding_rect()))),
                "{label} must not repeat its name inside the primary button");
            h.get_by_role_and_label(Role::Button, label).click();
            h.run();
            assert_eq!(h.state().editor().tool, tool);
            assert_eq!(
                h.get_by_role_and_label(Role::Button, label)
                    .accesskit_node()
                    .toggled(),
                Some(eframe::egui::accesskit::Toggled::True)
            );
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
        }
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
