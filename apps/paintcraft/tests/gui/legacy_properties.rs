use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Appearance, ClassicVariant, Settings};
use paintcraft_ui::i18n::{Language, translate};

fn open(h: &mut Harness<App>) {
    h.key_combination_modifiers(Modifiers::COMMAND, &[Key::E]);
    h.run();
}

#[test]
fn historical_properties_are_compact_staged_and_undoable() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
                let dir = scratch("historical-properties");
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
                let before = publications(&h);
                open(&mut h);
                let width_label = label("Width");
                let width = h.get_by_role_and_label(Role::SpinButton, &width_label);
                assert!(width.rect().height() <= 26.0);
                width.set_value_accesskit(640.0);
                h.run();
                h.get_by_role_and_label(Role::Button, &label("Cancel"))
                    .click();
                h.run();
                assert!(h.state().editor().document().same_semantics(&snapshot));
                assert_eq!(publications(&h), before);
                open(&mut h);
                for (name, value) in [("Width", 640.0), ("Height", 480.0)] {
                    h.get_by_role_and_label(Role::SpinButton, &label(name))
                        .set_value_accesskit(value);
                    h.run();
                }
                for name in ["Inches", "Centimeters", "Black and white"] {
                    assert!(
                        h.get_by_role_and_label(Role::RadioButton, &label(name))
                            .accesskit_node()
                            .is_disabled()
                    );
                }
                h.get_by_role_and_label(Role::Button, &label("OK")).click();
                h.run();
                assert_eq!(h.state().editor().document().canvas, (640, 480));
                assert_eq!(publications(&h), before + 1);
                h.key_combination_modifiers(Modifiers::COMMAND, &[Key::Z]);
                h.run();
                assert!(h.state().editor().document().same_semantics(&snapshot));
                open(&mut h);
                h.set_size(Vec2::new(480.0, 300.0));
                h.run();
                let cancel_label = label("Cancel");
                let cancel = h.get_by_role_and_label(Role::Button, &cancel_label);
                assert!(cancel.rect().bottom() <= 300.0);
                cancel.click();
                h.run();
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}

#[test]
#[ignore = "Explicit offscreen historical properties captures"]
fn export_historical_properties() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap();
    std::fs::create_dir_all(&output).unwrap();
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            let dir = scratch("historical-properties-capture");
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
            open(&mut h);
            h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(output.join(format!(
                    "classic-{}-properties-{}.png",
                    variant.id(),
                    language.id()
                )))
                .unwrap();
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
