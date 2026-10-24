use super::*;
use eframe::egui::{Role, accesskit::Toggled};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};
use paintcraft_tools::ShapeStyle;
use paintcraft_ui::i18n::{Language, translate};

#[test]
fn historical_shape_paint_rows_show_selection_and_change_real_shape_pixels() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
                let dir = scratch("historical-shape-paint");
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
                assert!(
                    h.get_by_role_and_label(Role::Button, &label("Shape fill"))
                        .accesskit_node()
                        .is_disabled()
                );
                h.get_by_role_and_label(Role::Button, &label("Rectangle"))
                    .click();
                h.run();
                let source = h.state().editor().document_snapshot();
                let ticket = h.state().editor().history_view().ticket;
                for (trigger, choice, expected) in [
                    ("Shape fill", "Solid fill", ShapeStyle::Both),
                    ("Shape outline", "No outline", ShapeStyle::Filled),
                    ("Shape fill", "No fill", ShapeStyle::Outline),
                    ("Shape outline", "Solid outline", ShapeStyle::Outline),
                ] {
                    h.get_by_role_and_label(Role::Button, &label(trigger))
                        .click();
                    h.run();
                    let caption = label(choice);
                    let row = h.get_by_role_and_label(Role::Button, &caption);
                    assert!(row.rect().height() <= 26.0);
                    row.click();
                    h.run();
                    assert!(h.query_by_role_and_label(Role::Button, &caption).is_none());
                    assert_eq!(h.state().editor().options.style, expected);
                    assert!(h.state().editor().document().same_semantics(&source));
                    assert_eq!(h.state().editor().history_view().ticket, ticket);
                    h.get_by_role_and_label(Role::Button, &label(trigger))
                        .click();
                    h.run();
                    assert_eq!(
                        h.get_by_role_and_label(Role::Button, &caption)
                            .accesskit_node()
                            .toggled(),
                        Some(Toggled::True)
                    );
                    h.key_press(Key::Escape);
                    h.run();
                }
                let logical = |n: f32| (n + 0.5) / ppp - 0.5;
                drag(
                    &mut h,
                    (logical(50.0), logical(50.0)),
                    (logical(80.0), logical(80.0)),
                );
                assert_eq!(top_pixel(&h, 65, 65), Rgba([255; 4]));
                assert_eq!(top_pixel(&h, 50, 65), Rgba([0, 0, 0, 255]));
                h.get_by_role_and_label(Role::Button, &label("Undo"))
                    .click();
                h.run();
                h.get_by_role_and_label(Role::Button, &label("Shape outline"))
                    .click();
                h.run();
                h.get_by_role_and_label(Role::Button, &label("No outline"))
                    .click();
                h.run();
                drag(
                    &mut h,
                    (logical(50.0), logical(50.0)),
                    (logical(80.0), logical(80.0)),
                );
                assert_eq!(top_pixel(&h, 65, 65), Rgba([0, 0, 0, 255]));
                h.get_by_role_and_label(Role::Button, &label("Undo"))
                    .click();
                h.run();
                assert!(h.state().editor().document().same_semantics(&source));
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}

#[test]
#[ignore = "Explicit offscreen historical shape paint captures"]
fn export_historical_shape_paint() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap();
    std::fs::create_dir_all(&output).unwrap();
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            let dir = scratch("historical-shape-paint-capture");
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
            let label = |s| translate(language, s);
            h.get_by_role_and_label(Role::Button, &label("Rectangle"))
                .click();
            h.run();
            for (name, choice) in [("outline", "Shape outline"), ("fill", "Shape fill")] {
                h.get_by_role_and_label(Role::Button, &label(choice))
                    .click();
                h.run();
                h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
                h.run_steps(4);
                h.render()
                    .unwrap()
                    .save(output.join(format!(
                        "classic-{}-{name}-{}.png",
                        variant.id(),
                        language.id()
                    )))
                    .unwrap();
                h.key_press(Key::Escape);
                h.run();
            }
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
