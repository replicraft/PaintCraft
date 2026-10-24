use super::*;
use eframe::egui::{Role, accesskit::Toggled};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};
use paintcraft_ui::i18n::{Language, format_message, translate};

fn hex(h: &mut Harness<App>, language: Language, value: &str) {
    h.get_by_label(&translate(language, "Hex RGB")).click();
    h.event(key(Key::A, Modifiers::COMMAND));
    h.event(Event::Text(value.into()));
    h.run();
}

#[test]
fn historical_color_wells_route_palette_and_staged_custom_edits_without_history() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
                let dir = scratch("historical-color-target");
                let custom = Rgba([31, 62, 93, 255]);
                let mut settings = Settings {
                    classic_variant: Some(variant),
                    language,
                    appearance,
                    ..Default::default()
                };
                settings.custom_colors[0] = Some(custom);
                settings.save(&dir).unwrap();
                let mut h =
                    harness_with(App::new(None).with_settings(dir.clone()), ppp, ppp == 2.0);
                h.set_size(Vec2::new(1480.0, 960.0));
                h.run();
                let label = |s| translate(language, s);
                let swatch = |hex: &str| {
                    format_message(language, "Color #{ $hex }", &[("hex", hex.to_owned())])
                };
                let snapshot = h.state().editor().document_snapshot();
                let ticket = h.state().editor().history_view().ticket;
                h.get_by_role_and_label(Role::ColorWell, &label("Fill color"))
                    .click();
                h.run();
                assert!(h.query_by_label(&label("Hex RGB")).is_none());
                assert_eq!(
                    h.get_by_role_and_label(Role::ColorWell, &label("Fill color"))
                        .accesskit_node()
                        .toggled(),
                    Some(Toggled::True)
                );
                h.get_by_label(&swatch("ED1C24")).click();
                h.run();
                assert_eq!(h.state().editor().secondary, Rgba([237, 28, 36, 255]));
                assert_eq!(h.state().editor().color, Rgba([0, 0, 0, 255]));
                h.get_by_role_and_label(Role::ColorWell, &label("Pencil color"))
                    .click();
                h.run();
                h.get_by_label(&swatch("22B14C")).click();
                h.run();
                assert_eq!(h.state().editor().color, Rgba([34, 177, 76, 255]));
                h.get_by_label(&swatch("00A2E8")).click_secondary();
                h.run();
                assert_eq!(h.state().editor().secondary, Rgba([0, 162, 232, 255]));
                h.get_by_role_and_label(Role::ColorWell, &label("Fill color"))
                    .click();
                h.run();
                h.get_by_label(&label("Edit colors…")).click();
                h.run();
                hex(&mut h, language, "#123456");
                h.get_by_label(&label("OK")).click();
                h.run();
                assert_eq!(h.state().editor().secondary, Rgba([18, 52, 86, 255]));
                h.get_by_label(&format_message(
                    language,
                    "Palette custom color { $number }",
                    &[("number", "1".into())],
                ))
                .click();
                h.run();
                assert_eq!(h.state().editor().secondary, custom);
                let empty = format_message(
                    language,
                    "Palette custom color { $number }",
                    &[("number", "2".into())],
                );
                h.get_by_label(&empty).click();
                h.run();
                hex(&mut h, language, "#ABCDEF");
                h.get_by_label(&label("Cancel")).click();
                h.run();
                assert_eq!(h.state().editor().secondary, custom);
                assert!(Settings::load(&dir).0.custom_colors[1].is_none());
                h.get_by_label(&empty).click();
                h.run();
                hex(&mut h, language, "#ABCDEF");
                h.get_by_label(&label("Add custom color")).click();
                h.run();
                h.get_by_label(&label("OK")).click();
                h.run();
                assert_eq!(h.state().editor().secondary, Rgba([171, 205, 239, 255]));
                assert_eq!(
                    Settings::load(&dir).0.custom_colors[1],
                    Some(Rgba([171, 205, 239, 255]))
                );
                assert_eq!(h.state().editor().color, Rgba([34, 177, 76, 255]));
                assert!(h.state().editor().document().same_semantics(&snapshot));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}

#[test]
#[ignore = "Explicit offscreen historical active Color 2 captures"]
fn export_historical_secondary_color_palette() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("paintcraft-historical-palette"));
    std::fs::create_dir_all(&output).unwrap();
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        let dir = scratch("historical-palette-capture");
        Settings {
            classic_variant: Some(variant),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(1100.0, 720.0));
        h.run();
        h.get_by_role_and_label(Role::ColorWell, "Fill color")
            .click();
        h.run();
        h.get_by_label("Color #ED1C24").click();
        h.run();
        h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
        h.run_steps(4);
        assert!(
            h.output()
                .platform_output
                .request_discard_reasons
                .is_empty()
        );
        h.render()
            .unwrap()
            .save(output.join(format!("classic-{}-color2.png", variant.id())))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn historical_double_click_edits_the_existing_custom_slot_for_color_two() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        let dir = scratch("historical-custom-double-click");
        let mut settings = Settings {
            classic_variant: Some(variant),
            ..Default::default()
        };
        settings.custom_colors[0] = Some(Rgba([10, 20, 30, 255]));
        settings.custom_colors[1] = Some(Rgba([40, 50, 60, 255]));
        settings.save(&dir).unwrap();
        // Pointer events must fall within the real double-click interval.
        // The ordinary harness advances 250 ms per event.
        let mut h = Harness::builder()
            .with_size(Vec2::new(1480.0, 960.0))
            .with_step_dt(1.0 / 60.0)
            .with_max_steps(60)
            .build_ui_state(
                |ui, app: &mut App| app.ui(ui),
                App::new(None)
                    .with_settings(dir.clone())
                    .with_initial_zoom(1.0),
            );
        h.set_size(Vec2::new(1480.0, 960.0));
        h.run();
        h.get_by_role_and_label(Role::ColorWell, "Fill color")
            .click();
        h.run();
        // Separate the earlier well click from the two-click gesture.
        h.run_steps(24);
        for _ in 0..2 {
            h.get_by_label("Palette custom color 1").click();
            h.step();
        }
        h.run();
        hex(&mut h, Language::English, "#123456");
        h.get_by_label("Add custom color").click();
        h.run();
        h.get_by_label("OK").click();
        h.run();
        let colors = Settings::load(&dir).0.custom_colors;
        assert_eq!(colors[0], Some(Rgba([18, 52, 86, 255])));
        assert_eq!(colors[1], Some(Rgba([40, 50, 60, 255])));
        assert_eq!(h.state().editor().color, Rgba([0, 0, 0, 255]));
        assert_eq!(h.state().editor().secondary, Rgba([18, 52, 86, 255]));
        assert_eq!(h.state().editor().undo_depth(), 0);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
