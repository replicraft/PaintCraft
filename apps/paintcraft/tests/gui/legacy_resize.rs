use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Appearance, ClassicVariant, Settings};
use paintcraft_ui::i18n::{Language, translate};

fn open(h: &mut Harness<App>, language: Language) {
    h.get_by_role_and_label(Role::Button, &translate(language, "Resize and skew"))
        .click();
    h.run();
}

#[test]
fn historical_resize_preview_preserves_units_aspect_and_document() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
                let dir = scratch("historical-resize");
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
                let label = |s| translate(language, s);
                let snapshot = h.state().editor().document_snapshot();
                let ticket = h.state().editor().history_view().ticket;
                open(&mut h, language);
                let name = label("Resize horizontal");
                let field = h.get_by_role_and_label(Role::SpinButton, &name);
                assert!(field.rect().height() <= 26.0);
                field.set_value_accesskit(50.0);
                h.run();
                let value = |h: &Harness<App>, name| {
                    h.get_by_role_and_label(Role::SpinButton, &label(name))
                        .accesskit_node()
                        .numeric_value()
                        .unwrap()
                };
                assert_eq!(value(&h, "Resize vertical"), 50.0);
                h.get_by_role_and_label(Role::RadioButton, &label("Pixels"))
                    .click();
                h.run();
                assert_eq!(value(&h, "Resize horizontal"), 500.0);
                assert_eq!(value(&h, "Resize vertical"), 243.0);
                h.get_by_role_and_label(Role::CheckBox, &label("Maintain aspect ratio"))
                    .click();
                h.run();
                h.get_by_role_and_label(Role::SpinButton, &name)
                    .set_value_accesskit(400.0);
                h.run();
                assert_eq!(value(&h, "Resize vertical"), 243.0);
                h.get_by_role_and_label(Role::SpinButton, &label("Skew vertical"))
                    .set_value_accesskit(-20.0);
                h.run();
                assert_eq!(value(&h, "Skew vertical"), -20.0);
                assert!(
                    h.get_by_role_and_label(Role::Button, &label("OK"))
                        .accesskit_node()
                        .is_disabled()
                );
                assert!(h.state().editor().document().same_semantics(&snapshot));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
                h.set_size(Vec2::new(480.0, 300.0));
                h.run();
                assert!(
                    h.get_by_role_and_label(Role::Button, &label("Cancel"))
                        .rect()
                        .bottom()
                        <= 300.0
                );
                h.get_by_role_and_label(Role::Button, &label("Cancel"))
                    .click();
                h.run();
                h.set_size(Vec2::new(1100.0, 900.0));
                h.run();
                open(&mut h, language);
                assert_eq!(value(&h, "Resize horizontal"), 100.0);
                assert_eq!(value(&h, "Skew vertical"), 0.0);
                h.key_press(Key::Escape);
                h.run();
                assert!(h.state().editor().document().same_semantics(&snapshot));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}

#[test]
#[ignore = "Explicit offscreen historical Resize and Skew captures"]
fn export_historical_resize() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap();
    std::fs::create_dir_all(&output).unwrap();
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            let dir = scratch("historical-resize-capture");
            Settings {
                classic_variant: Some(variant),
                language,
                appearance: Appearance::Light,
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
            h.set_size(Vec2::new(1100.0, 720.0));
            h.run();
            open(&mut h, language);
            h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(output.join(format!(
                    "classic-{}-resize-{}.png",
                    variant.id(),
                    language.id()
                )))
                .unwrap();
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
