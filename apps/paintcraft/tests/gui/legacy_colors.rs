use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Appearance, ClassicVariant, Settings};
use paintcraft_ui::i18n::{Language, translate};

fn open(h: &mut Harness<App>, language: Language) {
    h.get_by_role_and_label(Role::Button, &translate(language, "Edit colors…"))
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(Role::Button, &translate(language, "Edit colors…"))
        .click();
    h.run();
}

#[test]
fn historical_color_dialog_uses_native_hsl_ranges_and_stages_all_edits() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
                let dir = scratch("historical-colors-hsl");
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
                h.set_size(Vec2::new(1360.0, 900.0));
                h.run();
                let label = |s| translate(language, s);
                let snapshot = h.state().editor().document_snapshot();
                let ticket = h.state().editor().history_view().ticket;
                open(&mut h, language);
                for (name, value) in [("Hue", 0.0), ("Saturation", 240.0), ("Luminosity", 120.0)] {
                    h.get_by_role_and_label(Role::SpinButton, &label(name))
                        .set_value_accesskit(value);
                    h.run();
                }
                assert_eq!(
                    h.get_by_role_and_label(Role::SpinButton, &label("Red"))
                        .accesskit_node()
                        .numeric_value(),
                    Some(255.0)
                );
                for (value, rgb) in [
                    (240.0, [255.0; 3]),
                    (0.0, [0.0; 3]),
                    (120.0, [255.0, 0.0, 0.0]),
                ] {
                    h.get_by_role_and_label(Role::Slider, &label("Luminosity"))
                        .set_value_accesskit(value);
                    h.run();
                    for (name, value) in ["Red", "Green", "Blue"].into_iter().zip(rgb) {
                        assert_eq!(
                            h.get_by_role_and_label(Role::SpinButton, &label(name))
                                .accesskit_node()
                                .numeric_value(),
                            Some(value)
                        );
                    }
                }
                h.get_by_role_and_label(Role::SpinButton, &label("Hue"))
                    .set_value_accesskit(80.0);
                h.run();
                assert_eq!(
                    h.get_by_role_and_label(Role::SpinButton, &label("Green"))
                        .accesskit_node()
                        .numeric_value(),
                    Some(255.0)
                );
                assert_eq!(h.state().editor().color, Rgba([0, 0, 0, 255]));
                h.get_by_role_and_label(Role::TextInput, &label("Hex RGB"))
                    .click();
                h.event(key(Key::A, Modifiers::COMMAND));
                h.event(Event::Text("#0".into()));
                h.run();
                assert!(h.get_by_label(&label("OK")).accesskit_node().is_disabled());
                h.get_by_label(&label("Cancel")).click();
                h.run();
                assert_eq!(h.state().editor().color, Rgba([0, 0, 0, 255]));
                open(&mut h, language);
                for (name, value) in [("Hue", 160.0), ("Saturation", 240.0), ("Luminosity", 120.0)]
                {
                    h.get_by_role_and_label(Role::SpinButton, &label(name))
                        .set_value_accesskit(value);
                    h.run();
                }
                h.get_by_label(&label("Add custom color")).click();
                h.run();
                assert!(Settings::load(&dir).0.custom_colors[0].is_none());
                h.get_by_label(&label("OK")).click();
                h.run();
                assert_eq!(h.state().editor().color, Rgba([0, 0, 255, 255]));
                assert_eq!(
                    Settings::load(&dir).0.custom_colors[0],
                    Some(Rgba([0, 0, 255, 255]))
                );
                assert!(h.state().editor().document().same_semantics(&snapshot));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}

#[test]
fn historical_color_dialog_keeps_footer_reachable_and_all_custom_slots_in_short_windows() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        let dir = scratch("historical-color-compact");
        Settings {
            classic_variant: Some(variant),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(480.0, 300.0));
        h.run();
        open(&mut h, Language::English);
        for name in ["OK", "Cancel"] {
            let rect = h.get_by_label(name).rect();
            assert!(
                rect.min.x >= 0.0
                    && rect.min.y >= 0.0
                    && rect.max.x <= 480.0
                    && rect.max.y <= 300.0,
                "{name}: {rect:?}"
            );
        }
        h.get_by_label("Custom color 28").scroll_to_me();
        h.run();
        h.get_by_label("Custom color 28").click();
        h.run();
        h.get_by_role_and_label(Role::SpinButton, "Hue")
            .scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::SpinButton, "Hue")
            .set_value_accesskit(160.0);
        h.run();
        h.get_by_label("Add custom color").scroll_to_me();
        h.run();
        h.get_by_label("Add custom color").click();
        h.run();
        h.get_by_label("OK").click();
        h.run();
        assert!(Settings::load(&dir).0.custom_colors[27].is_some());
        assert_eq!(h.state().editor().undo_depth(), 0);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit offscreen historical Edit Colors captures"]
fn export_historical_edit_colors() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("paintcraft-historical-edit-colors"));
    std::fs::create_dir_all(&output).unwrap();
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            let dir = scratch("historical-edit-color-capture");
            Settings {
                classic_variant: Some(variant),
                language,
                appearance: if language == Language::Japanese {
                    Appearance::Dark
                } else {
                    Appearance::Light
                },
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
            h.set_size(Vec2::new(1100.0, 720.0));
            h.run();
            open(&mut h, language);
            h.get_by_role_and_label(Role::TextInput, &translate(language, "Hex RGB"))
                .click();
            h.event(key(Key::A, Modifiers::COMMAND));
            h.event(Event::Text("#3ED745".into()));
            h.run();
            h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(output.join(format!(
                    "classic-{}-edit-colors-{}.png",
                    variant.id(),
                    language.id()
                )))
                .unwrap();
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
