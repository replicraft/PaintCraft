use super::*;
use eframe::egui::{Role, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

fn leading_number(h: &Harness<App>, label: &str, text: &str) {
    let rect = h.get_by_role_and_label(Role::SpinButton, label).rect();
    let painted = h
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            Shape::Text(value)
                if value.galley.job.text.parse::<f64>().ok() == text.parse::<f64>().ok()
                    && rect.contains(value.pos) =>
            {
                Some(value)
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "{label}: value {text} is not painted inside {rect:?}; visible={:?}",
                h.output()
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        Shape::Text(value)
                            if rect.intersects(eframe::egui::Rect::from_min_size(
                                value.pos,
                                value.galley.size()
                            )) =>
                            Some((&value.galley.job.text, value.pos)),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            )
        });
    let inset = painted.pos.x - rect.left();
    assert!(
        (13.0..=16.0).contains(&inset),
        "{label}: leading inset {inset}, rect={rect:?}"
    );
    assert_eq!(painted.galley.job.sections[0].format.font_id.size, 16.0);
    assert_eq!(
        h.get_by_label(label).accesskit_node().numeric_value(),
        text.parse::<f64>().ok()
    );
}

#[test]
fn color_numeric_fields_align_in_resting_and_edit_modes_without_publishing() {
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("color-numeric-alignment");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.set_size(Vec2::new(1360.0, 900.0));
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        h.get_by_label("Edit colors…").click();
        h.run();
        for (label, value) in [("Red", 32.0), ("Green", 128.0), ("Blue", 255.0)] {
            h.get_by_label(label).set_value_accesskit(value);
            h.run();
        }
        for (label, value) in [("Red", "32"), ("Green", "128"), ("Blue", "255")] {
            leading_number(&h, label, value);
            h.get_by_label(label).focus();
            h.run();
            leading_number(&h, label, value);
            h.get_by_label("Hex RGB").focus();
            h.run();
        }
        h.get_by_label("Color model").click();
        h.run();
        h.get_by_label("HSL").click();
        h.run();
        for (label, value) in [("Hue", 210.0), ("Saturation", 100.0), ("Lightness", 50.0)] {
            h.get_by_label(label).set_value_accesskit(value);
            h.run();
        }
        for (label, value) in [("Hue", "210"), ("Saturation", "100"), ("Lightness", "50")] {
            leading_number(&h, label, value);
        }
        h.get_by_label("Cancel").click();
        h.run();
        assert_eq!(h.state().editor().color, Rgba([0, 0, 0, 255]));
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn font_size_alignment_preserves_joined_geometry_and_cancelled_drafts() {
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("font-number-alignment");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        h.get_by_label("Text").click();
        h.run();
        h.get_by_label("Font size").set_value_accesskit(37.0);
        h.run();
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        leading_number(&h, "Font size", "37");
        let field = h.get_by_label("Font size").rect();
        let arrow = h.get_by_label("Font size presets").rect();
        assert_eq!(field.right(), arrow.left());
        assert_eq!(field.width() + arrow.width(), 90.0);
        h.get_by_label("Font size").focus();
        h.run();
        leading_number(&h, "Font size", "37");
        h.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
        h.run();
        h.get_by_label("Font size").type_text("40");
        h.run();
        h.key_press(Key::Escape);
        h.run();
        leading_number(&h, "Font size", "37");
        assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        assert!(!h.state().editor().is_dirty());
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit GPU numeric field alignment review"]
fn export_numeric_field_surfaces() {
    let output = std::env::temp_dir().join("paintcraft-numeric-fields");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        let dir = scratch("numeric-control-review");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(1360.0, 900.0));
        h.run();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        h.get_by_label("Edit colors…").click();
        h.run();
        for (label, value) in [("Red", 32.0), ("Green", 128.0), ("Blue", 255.0)] {
            h.get_by_label(label).set_value_accesskit(value);
            h.run();
        }
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-rgb.png")))
            .unwrap();
        h.get_by_label("Red").focus();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-red-focused.png")))
            .unwrap();
        h.get_by_label("Color model").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-model-popup.png")))
            .unwrap();
        h.get_by_label("HSL").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-hsl.png")))
            .unwrap();
        h.get_by_label("Cancel").click();
        h.run();
        h.get_by_label("Text").click();
        h.run();
        h.get_by_label("Font size").set_value_accesskit(37.0);
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-font-size.png")))
            .unwrap();
        h.get_by_label("Font size").focus();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-font-size-focused.png")))
            .unwrap();
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
