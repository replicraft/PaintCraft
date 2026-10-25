use super::*;
use eframe::egui::{Role, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};
use paintcraft_editor::execute::Selection;
use paintcraft_ui::i18n::{Language, translate};

fn open(h: &mut Harness<App>, language: Language) {
    h.get_by_role_and_label(Role::Button, &translate(language, "Selection options"))
        .click();
    h.run();
}

#[test]
fn historical_selection_menu_has_compact_sections_and_executes_real_delete_and_undo() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
                let dir = scratch("historical-selection-menu");
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
                h.get_by_role_and_label(Role::Button, &translate(language, "Pencil"))
                    .click();
                h.run();
                drag(&mut h, (30.0, 30.0), (36.0, 30.0));
                assert_eq!(top_pixel(&h, 32, 30), Rgba([0, 0, 0, 255]));
                let snapshot = h.state().editor().document_snapshot();
                let ticket = h.state().editor().history_view().ticket;
                let label = |s| translate(language, s);
                open(&mut h, language);
                assert!(
                    h.query_by_role_and_label(Role::Label, &label("Selection shapes"))
                        .is_some()
                );
                assert!(
                    h.query_by_role_and_label(Role::Label, &label("Selection options"))
                        .is_some()
                );
                let mut bottom = 0.0;
                for name in [
                    "Rectangular selection",
                    "Free-form selection",
                    "Select all",
                    "Invert selection",
                    "Delete",
                    "Transparent selection",
                    "Deselect",
                ] {
                    let rect = h.get_by_role_and_label(Role::Button, &label(name)).rect();
                    assert_eq!(rect.height(), 24.0);
                    assert!(
                        rect.width() >= 220.0 && rect.width() <= 224.0,
                        "{name}: {rect:?}"
                    );
                    assert!(rect.top() >= bottom);
                    bottom = rect.bottom();
                    let text = h
                        .output()
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            Shape::Text(text)
                                if text.galley.job.text == label(name)
                                    && rect.contains(text.pos) =>
                            {
                                Some(text)
                            }
                            _ => None,
                        })
                        .expect("visible historical selection caption");
                    assert_eq!(text.galley.job.sections[0].format.font_id.size, 12.0);
                }
                for name in ["Delete", "Deselect"] {
                    assert!(
                        h.get_by_role_and_label(Role::Button, &label(name))
                            .accesskit_node()
                            .is_disabled()
                    );
                }
                assert!(
                    !h.get_by_role_and_label(Role::Button, &label("Transparent selection"))
                        .accesskit_node()
                        .is_disabled()
                );
                assert!(!h.state().editor().options.transparent_selection);
                h.ctx.style_mut_of(h.ctx.theme(), |style| {
                    style.interaction.tooltip_delay = 0.0;
                    style.interaction.show_tooltips_only_when_still = false;
                });
                h.get_by_role_and_label(Role::Button, &label("Transparent selection"))
                    .hover();
                h.run();
                assert_eq!(
                    h.query_all_by_role_and_label(
                        Role::Label,
                        &label("When moving a selection, skip pixels whose RGB matches Color 2; alpha is ignored.")
                    )
                    .count(),
                    1
                );
                h.get_by_role_and_label(Role::Button, &label("Transparent selection"))
                    .click();
                h.run();
                assert!(h.state().editor().document().same_semantics(&snapshot));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
                assert!(h.state().editor().options.transparent_selection);
                assert!(
                    h.query_by_role_and_label(Role::Button, &label("Rectangular selection"))
                        .is_none()
                );
                open(&mut h, language);
                assert_eq!(
                    h.get_by_label(&label("Transparent selection"))
                        .accesskit_node()
                        .toggled(),
                    Some(eframe::egui::accesskit::Toggled::True)
                );
                h.get_by_label(&label("Transparent selection")).click();
                h.run();
                assert!(!h.state().editor().options.transparent_selection);
                assert!(h.state().editor().document().same_semantics(&snapshot));
                assert_eq!(h.state().editor().history_view().ticket, ticket);
                open(&mut h, language);
                h.get_by_role_and_label(Role::Button, &label("Rectangular selection"))
                    .focus();
                h.run();
                h.key_press(Key::Enter);
                h.run();
                assert_eq!(h.state().editor().tool, paintcraft_tools::Tool::Select);
                assert!(
                    h.query_by_role_and_label(Role::Button, &label("Deselect"))
                        .is_none()
                );
                open(&mut h, language);
                h.get_by_role_and_label(Role::Button, &label("Select all"))
                    .click();
                h.run();
                assert_eq!(
                    h.state().editor().selection(),
                    Selection::Rect(h.state().editor().document().canvas_rect())
                );
                open(&mut h, language);
                h.get_by_role_and_label(Role::Button, &label("Delete"))
                    .click();
                h.run();
                assert_eq!(top_pixel(&h, 32, 30), Rgba([255; 4]));
                h.get_by_role_and_label(Role::Button, &label("Undo"))
                    .click();
                h.run();
                assert_eq!(top_pixel(&h, 32, 30), Rgba([0, 0, 0, 255]));
                open(&mut h, language);
                h.get_by_role_and_label(Role::Button, &label("Deselect"))
                    .click();
                h.run();
                assert_eq!(h.state().editor().selection(), Selection::Inactive);
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}

#[test]
fn historical_selection_menu_scrolls_to_commands_in_short_windows() {
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        let dir = scratch("short-historical-selection");
        Settings {
            classic_variant: Some(variant),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(480.0, 240.0));
        h.run();
        open(&mut h, Language::English);
        h.get_by_role_and_label(Role::Button, "Select all")
            .scroll_to_me();
        h.run();
        h.get_by_role_and_label(Role::Button, "Select all").click();
        h.run();
        open(&mut h, Language::English);
        h.get_by_role_and_label(Role::Button, "Deselect")
            .scroll_to_me();
        h.run();
        let rect = h.get_by_role_and_label(Role::Button, "Deselect").rect();
        assert!(
            rect.min.x >= 0.0 && rect.min.y >= 0.0 && rect.max.x <= 480.0 && rect.max.y <= 240.0
        );
        h.get_by_role_and_label(Role::Button, "Deselect").click();
        h.run();
        assert_eq!(h.state().editor().selection(), Selection::Inactive);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit offscreen historical Selection menu captures"]
fn export_historical_selection_menus() {
    let output = std::env::var_os("PAINTCRAFT_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("paintcraft-historical-selection"));
    std::fs::create_dir_all(&output).unwrap();
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            let dir = scratch("historical-selection-capture");
            Settings {
                classic_variant: Some(variant),
                language,
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
            h.set_size(Vec2::new(1100.0, 720.0));
            h.run();
            open(&mut h, language);
            h.get_by_role_and_label(Role::Button, &translate(language, "Select all"))
                .click();
            h.run();
            open(&mut h, language);
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
                .save(output.join(format!(
                    "classic-{}-selection-{}.png",
                    variant.id(),
                    language.id()
                )))
                .unwrap();
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
