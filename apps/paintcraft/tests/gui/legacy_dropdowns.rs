use super::*;
use paintcraft::settings::{ClassicVariant, Settings};
use paintcraft_core::types::brush::BrushMedia;

#[test]
fn historical_brush_tiles_and_sizes_support_keyboard_selection_without_document_changes() {
    use eframe::egui::{Role, accesskit::Toggled};
    use paintcraft::settings::Appearance;
    use paintcraft_tools::Tool;
    use paintcraft_ui::i18n::{Language, format_message, translate};
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
                let dir = scratch("historical-brush-and-size");
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
                h.query_all_by_role_and_label(Role::Button, &label("Brushes"))
                    .last()
                    .unwrap()
                    .click();
                h.run();
                let top = h.get_by_label(&label("Round brush")).rect().top();
                for (index, media) in BrushMedia::ALL.into_iter().enumerate() {
                    let name = if index == 0 {
                        "Round brush"
                    } else {
                        media.name()
                    };
                    let rect = h.get_by_label(&label(name)).rect();
                    assert_eq!(rect.size(), Vec2::new(80.0, 64.0));
                    assert_eq!(rect.top(), top + (index / 3) as f32 * 64.0);
                }
                h.get_by_label(&label("Round brush")).focus();
                h.run();
                for key in [
                    Key::ArrowRight,
                    Key::ArrowDown,
                    Key::ArrowLeft,
                    Key::ArrowUp,
                    Key::ArrowRight,
                ] {
                    h.key_press(key);
                    h.run();
                }
                h.key_press(Key::Enter);
                h.run();
                assert_eq!(
                    h.state().editor().options.media,
                    BrushMedia::CalligraphyBrush
                );
                h.query_all_by_role_and_label(Role::Button, &label("Brushes"))
                    .last()
                    .unwrap()
                    .click();
                h.run();
                h.get_by_label(&label("Oil brush")).focus();
                h.run();
                h.key_press(Key::Enter);
                h.run();
                assert_eq!(h.state().editor().tool, Tool::Brush);
                assert_eq!(h.state().editor().options.media, BrushMedia::Oil);
                assert!(h.query_by_label(&label("Oil brush")).is_none());
                h.query_all_by_role_and_label(Role::Button, &label("Brushes"))
                    .last()
                    .unwrap()
                    .click();
                h.run();
                assert_eq!(
                    h.get_by_label(&label("Oil brush"))
                        .accesskit_node()
                        .toggled(),
                    Some(Toggled::True)
                );
                h.key_press(Key::Escape);
                h.run();
                h.get_by_label(&label("Pencil")).click();
                h.run();
                h.get_by_label(&label("Stroke size")).click();
                h.run();
                h.get_by_label("5 px").focus();
                h.run();
                h.key_press(Key::Enter);
                h.run();
                assert_eq!(h.state().editor().options.pencil_size, 5);
                let brush_size = h.state().editor().options.size;
                h.get_by_label(&label("Stroke size")).click();
                h.run();
                assert_eq!(
                    h.get_by_label("5 px").accesskit_node().toggled(),
                    Some(Toggled::True)
                );
                h.get_by_role_and_label(Role::SpinButton, &label("Size in pixels"))
                    .set_value_accesskit(37.0);
                h.run();
                assert!(
                    h.query_by_label(&format_message(
                        language,
                        "Current: { $size } px",
                        &[("size", "37".to_string())]
                    ))
                    .is_some()
                );
                for size in [1, 3, 5, 8] {
                    assert_eq!(
                        h.get_by_label(&format!("{size} px"))
                            .accesskit_node()
                            .toggled(),
                        Some(Toggled::False)
                    );
                }
                h.key_press(Key::Escape);
                h.run();
                assert_eq!(h.state().editor().options.pencil_size, 37);
                assert_eq!(h.state().editor().options.size, brush_size);
                assert!(h.state().editor().document().same_semantics(&snapshot));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}

#[test]
fn historical_brush_and_size_popups_scroll_in_short_windows() {
    use eframe::egui::Role;
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        let dir = scratch("short-historical-dropdowns");
        Settings {
            classic_variant: Some(variant),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(640.0, 240.0));
        h.run();
        h.get_by_label("Brushes").click();
        h.run();
        h.get_by_label("Square brush").scroll_to_me();
        h.run();
        h.get_by_label("Square brush").click();
        h.run();
        assert!(!h.state().editor().options.round);
        h.get_by_label("Stroke size").scroll_to_me();
        h.run();
        h.get_by_label("Stroke size").click();
        h.run();
        h.get_by_role_and_label(Role::SpinButton, "Size in pixels")
            .scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::SpinButton, "Size in pixels")
            .set_value_accesskit(61.0);
        h.run();
        h.key_press(Key::Escape);
        h.run();
        assert_eq!(h.state().editor().options.size, 61);
        assert!(
            h.query_by_role_and_label(Role::SpinButton, "Size in pixels")
                .is_none()
        );
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit offscreen historical Brushes and Size popup captures"]
fn export_historical_brush_and_size_popups() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("paintcraft-historical-dropdowns"));
    std::fs::create_dir_all(&output).unwrap();
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        let dir = scratch("historical-dropdown-capture");
        Settings {
            classic_variant: Some(variant),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(1100.0, 720.0));
        h.run();
        h.get_by_label("Brushes").click();
        h.run();
        h.get_by_label("Oil brush").click();
        h.run();
        h.get_by_label("Brushes").click();
        h.run();
        h.event(Event::PointerMoved(Pos2::new(3.0, 3.0)));
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(output.join(format!("classic-{}-brushes.png", variant.id())))
            .unwrap();
        h.key_press(Key::Escape);
        h.run();
        h.get_by_label("Pencil").click();
        h.run();
        h.get_by_label("Stroke size").click();
        h.run();
        h.get_by_label("5 px").click();
        h.run();
        h.get_by_label("Stroke size").click();
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
            .save(output.join(format!("classic-{}-size.png", variant.id())))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
