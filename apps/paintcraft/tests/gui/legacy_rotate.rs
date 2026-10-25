use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Appearance, ClassicVariant, Settings};
use paintcraft_ui::i18n::{Language, translate};

#[test]
fn historical_rotate_rows_transform_real_pixels_once_and_undo() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            let dir = scratch("historical-rotate");
            Settings {
                classic_variant: Some(variant),
                language,
                appearance: Appearance::Light,
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, true);
            h.set_size(Vec2::new(1360.0, 900.0));
            h.run();
            drag(&mut h, (40.0, 50.0), (40.0, 50.0));
            let label = |s| translate(language, s);
            for selected in [false, true] {
                if selected {
                    h.get_by_role_and_label(Role::Button, &label("Select"))
                        .click();
                    h.run();
                    drag(&mut h, (30.0, 40.0), (59.0, 79.0));
                }
                let source = h.state().editor().document_snapshot();
                let selection = h.state().editor().selection();
                for (name, whole, region, swap) in [
                    ("Rotate right 90°", (435, 40), (59, 50), true),
                    ("Rotate left 90°", (50, 959), (40, 59), true),
                    ("Rotate 180°", (959, 435), (49, 69), false),
                    ("Flip vertical", (40, 435), (40, 69), false),
                    ("Flip horizontal", (959, 50), (49, 50), false),
                ] {
                    let before = publications(&h);
                    h.get_by_role_and_label(Role::Button, &label("Rotate"))
                        .click();
                    h.run();
                    let caption = label(name);
                    let row = h.get_by_role_and_label(Role::Button, &caption);
                    assert!((28.0..=32.0).contains(&row.rect().height()));
                    row.click();
                    h.run();
                    assert!(h.query_by_role_and_label(Role::Button, &caption).is_none());
                    assert_eq!(publications(&h), before + 1);
                    let (x, y) = if selected { region } else { whole };
                    assert_eq!(
                        top_pixel(&h, x, y),
                        Rgba([0, 0, 0, 255]),
                        "{variant:?} {name} selected={selected}"
                    );
                    assert_eq!(
                        h.state().editor().document().canvas,
                        if swap && !selected {
                            (486, 1000)
                        } else {
                            (1000, 486)
                        }
                    );
                    h.get_by_role_and_label(Role::Button, &label("Undo"))
                        .click();
                    h.run();
                    assert!(h.state().editor().document().same_semantics(&source));
                    assert_eq!(h.state().editor().selection(), selection);
                }
            }
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
#[ignore = "Explicit offscreen historical Rotate menu captures"]
fn export_historical_rotate() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap();
    std::fs::create_dir_all(&output).unwrap();
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            let dir = scratch("historical-rotate-capture");
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
            h.get_by_role_and_label(Role::Button, &translate(language, "Rotate"))
                .click();
            h.run();
            h.get_by_role_and_label(Role::Button, &translate(language, "Rotate left 90°"))
                .hover();
            h.run();
            h.render()
                .unwrap()
                .save(output.join(format!(
                    "classic-{}-rotate-{}.png",
                    variant.id(),
                    language.id()
                )))
                .unwrap();
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
