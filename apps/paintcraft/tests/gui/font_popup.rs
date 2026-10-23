use super::*;
use paintcraft_classic::TextOptions;

#[derive(Default)]
struct FontUi {
    options: TextOptions,
    families: Vec<String>,
    search: String,
}

#[test]
fn font_popup_keeps_search_fixed_while_scrolling_a_large_catalog_and_filters_from_the_top() {
    for (size, ppp) in [
        (Vec2::new(480.0, 500.0), 1.0),
        (Vec2::new(360.0, 360.0), 2.0),
    ] {
        let families: Vec<_> = (0..1000)
            .map(|i| {
                format!(
                    "Fixture font {i:04}{}",
                    if i == 999 {
                        " with a particularly long family name that must truncate"
                    } else {
                        ""
                    }
                )
            })
            .collect();
        let selected = families[999].clone();
        let options = TextOptions {
            family: families[0].clone(),
            ..Default::default()
        };
        let mut h = Harness::builder()
            .with_size(size)
            .with_pixels_per_point(ppp)
            .build_ui_state(
                |ui, state: &mut FontUi| {
                    paintcraft_classic::text_options(
                        ui,
                        &mut state.options,
                        &state.families,
                        &mut state.search,
                        false,
                    );
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx().request_discard("test: repeated font-popup layout");
                    }
                },
                FontUi {
                    options,
                    families,
                    search: String::new(),
                },
            );
        h.run();
        h.get_by_label("Font family").click();
        h.run();
        let search = h.get_by_label("Search fonts").rect();
        assert!(
            search.min.x >= 0.0
                && search.min.y >= 0.0
                && search.max.x <= size.x
                && search.max.y <= size.y
        );
        // Clicking the search field must keep the popup open.
        h.get_by_label("Search fonts").click();
        h.run();
        let point = h.get_by_label("Fixture font 0000").rect().center();
        h.input_mut().events.extend([
            Event::PointerMoved(point),
            Event::MouseWheel {
                unit: eframe::egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -100000.0),
                phase: eframe::egui::TouchPhase::Move,
                source: eframe::egui::MouseWheelSource::Unknown,
                modifiers: Modifiers::NONE,
            },
        ]);
        h.run_steps(12);
        assert_eq!(
            h.get_by_label("Search fonts").rect(),
            search,
            "search must not scroll with the list"
        );
        assert!(
            h.query_all_by_label_contains("Fixture font ").count() < 30,
            "only visible font rows should be created"
        );
        let last = h.get_by_label_contains("Fixture font 0999").rect();
        assert!(last.max.x <= size.x && last.max.y <= size.y);
        h.get_by_label_contains("Fixture font 0999").click();
        h.run();
        assert_eq!(h.state().options.family, selected);
        assert!(h.query_by_label("Search fonts").is_none());
        assert!(h.get_by_label("Font family").rect().width() <= 238.0);
        h.get_by_label("Font family").click();
        h.run();
        // Opening the popup focuses search without an additional click.
        h.event(Event::Text("Fixture font 0004".into()));
        h.run();
        assert_eq!(h.state().search, "Fixture font 0004");
        assert!(h.query_by_label("Fixture font 0004").is_some());
        h.get_by_label("Fixture font 0004").click();
        h.run();
        assert_eq!(h.state().options.family, "Fixture font 0004");
    }
}

#[test]
fn text_dropdown_rows_match_toolbar_metrics_and_keep_selected_choices_reachable() {
    use eframe::egui::{Role, accesskit::Toggled, epaint::Shape};
    for (size, ppp, dark) in [
        (Vec2::new(1100.0, 500.0), 1.0, false),
        (Vec2::new(480.0, 360.0), 2.0, true),
    ] {
        let mut h = Harness::builder()
            .with_size(size)
            .with_pixels_per_point(ppp)
            .build_ui_state(
                move |ui, state: &mut FontUi| {
                    ui.ctx().set_visuals(if dark {
                        eframe::egui::Visuals::dark()
                    } else {
                        eframe::egui::Visuals::light()
                    });
                    paintcraft_classic::text_options(
                        ui,
                        &mut state.options,
                        &state.families,
                        &mut state.search,
                        false,
                    );
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx()
                            .request_discard("test: repeated text-dropdown layout");
                    }
                },
                FontUi {
                    families: vec!["Selawik".into(), "Fixture family".into()],
                    ..Default::default()
                },
            );
        h.run();
        for label in ["Font family", "Font size", "Font size presets"] {
            assert_eq!(h.get_by_label(label).rect().height(), 40.0, "{label}");
        }
        h.get_by_label("Font size presets").click();
        h.run();
        let selected = h.get_by_role_and_label(Role::Button, "11 pt");
        assert_eq!(selected.rect().height(), 40.0);
        assert_eq!(selected.accesskit_node().toggled(), Some(Toggled::True));
        let rect = selected.rect();
        assert!(h.output().shapes.iter().any(|shape| {
            matches!(&shape.shape, Shape::Rect(painted)
                if rect.contains_rect(painted.rect)
                    && painted.rect.width() > rect.width() - 2.0
                    && painted.stroke.width > 0.0
                    && painted.stroke.color.a() > 0)
        }));
        h.get_by_label("72 pt").scroll_to_me();
        h.run();
        let last = h.get_by_label("72 pt").rect();
        assert!(last.min.y >= 0.0 && last.max.y <= size.y && last.max.x <= size.x);
        h.get_by_label("72 pt").click();
        h.run();
        assert_eq!(h.state().options.points, 72);
        assert!(h.query_by_label("72 pt").is_none());
        h.get_by_label("Font family").click();
        h.run();
        let selected = h.get_by_label("Selawik");
        assert_eq!(selected.rect().height(), 40.0);
        assert_eq!(selected.accesskit_node().toggled(), Some(Toggled::True));
        h.key_press(Key::Escape);
        h.run();
        assert!(h.query_by_label("Search fonts").is_none());
        assert_eq!(h.state().options.family, "Selawik");
        assert_eq!(h.state().options.points, 72);
    }
}

#[test]
fn narrow_text_toolbar_scrolls_to_background_and_back_without_changing_other_options() {
    for (width, ppp) in [(360.0, 1.0), (480.0, 2.0)] {
        let mut h = Harness::builder()
            .with_size(Vec2::new(width, 360.0))
            .with_pixels_per_point(ppp)
            .build_ui_state(
                |ui, state: &mut FontUi| {
                    paintcraft_classic::text_options(
                        ui,
                        &mut state.options,
                        &state.families,
                        &mut state.search,
                        false,
                    );
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx()
                            .request_discard("test: repeated narrow text layout");
                    }
                },
                FontUi::default(),
            );
        h.run();
        let original = h.state().options.clone();
        let start = h.get_by_label("Font family").rect().center();
        h.input_mut().events.extend([
            Event::PointerMoved(start),
            Event::MouseWheel {
                unit: eframe::egui::MouseWheelUnit::Point,
                delta: Vec2::new(-10000.0, 0.0),
                phase: eframe::egui::TouchPhase::Move,
                source: eframe::egui::MouseWheelSource::Unknown,
                modifiers: Modifiers::NONE,
            },
        ]);
        h.run_steps(12);
        let background = h.get_by_label("Background fill").rect();
        assert!(background.min.x >= 0.0 && background.max.x <= width);
        h.get_by_label("Background fill").click();
        h.run();
        let mut expected = original;
        expected.background_fill = true;
        assert_eq!(h.state().options, expected);
        h.get_by_label("Font family").scroll_to_me();
        h.run();
        let family = h.get_by_label("Font family").rect();
        assert!(family.min.x >= 0.0 && family.max.x <= width);
        assert_eq!(h.state().options, expected);
    }
}

#[test]
fn text_dropdowns_reveal_current_choices_and_keep_their_labels_left_aligned() {
    use eframe::egui::{Role, epaint::Shape};
    for (size, ppp, dark) in [
        (Vec2::new(1100.0, 500.0), 1.0, false),
        (Vec2::new(480.0, 360.0), 2.0, true),
    ] {
        let families: Vec<_> = (0..1000)
            .map(|i| format!("Fixture family {i:04}"))
            .collect();
        let options = TextOptions {
            family: families[900].clone(),
            points: 72,
            ..Default::default()
        };
        let original = options.clone();
        let mut h = Harness::builder()
            .with_size(size)
            .with_pixels_per_point(ppp)
            .build_ui_state(
                move |ui, state: &mut FontUi| {
                    ui.ctx().set_visuals(if dark {
                        eframe::egui::Visuals::dark()
                    } else {
                        eframe::egui::Visuals::light()
                    });
                    paintcraft_classic::text_options(
                        ui,
                        &mut state.options,
                        &state.families,
                        &mut state.search,
                        false,
                    );
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx()
                            .request_discard("test: repeated selected font layout");
                    }
                },
                FontUi {
                    options,
                    families,
                    search: String::new(),
                },
            );
        h.run();
        let entry = h.get_by_label("Font size").rect();
        let trigger = h.get_by_label("Font size presets").rect();
        assert_eq!(entry.height(), 40.0);
        assert_eq!(entry.right(), trigger.left());
        assert_eq!(entry.top(), trigger.top());
        assert_eq!(entry.width() + trigger.width(), 90.0);
        for (popup, selected, painted_label) in [
            ("Font family", "Fixture family 0900", "Fixture family 0900"),
            ("Font size presets", "72 pt", "72"),
        ] {
            h.get_by_label(popup).click();
            h.run();
            let row = h.get_by_role_and_label(Role::Button, selected).rect();
            let (clip, label) = h
                .output()
                .shapes
                .iter()
                .find_map(|shape| {
                    if let Shape::Text(text) = &shape.shape
                        && text.galley.job.text == painted_label
                        && row.contains(text.pos)
                    {
                        Some((shape.clip_rect, text))
                    } else {
                        None
                    }
                })
                .expect("current choice is painted on opening");
            assert!(
                clip.contains_rect(row),
                "selected row {row:?} outside {clip:?}"
            );
            assert!((10.0..=18.0).contains(&(label.pos.x - row.left())));
            assert_eq!(label.galley.job.sections[0].format.font_id.size, 16.0);
            assert!(h.query_all_by_label_contains("Fixture family ").count() < 30);
            if popup == "Font family" {
                let search = h.get_by_label("Search fonts").rect();
                let footer = h.get_by_label("1000 fonts").rect();
                assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
                    Shape::Text(text) if text.galley.job.text == "1000 fonts"
                        && shape.clip_rect.contains_rect(footer))));
                h.event(Event::Text("Fixture family 0004".into()));
                h.run();
                assert_eq!(h.get_by_label("Search fonts").rect(), search);
                let result = h.get_by_label("Fixture family 0004").rect();
                assert!(result.top() > search.bottom() && result.bottom() <= size.y);
                assert_eq!(h.state().options, original);
            } else {
                h.get_by_label("8 pt").scroll_to_me();
                h.run();
                let first = h.get_by_label("8 pt").rect();
                assert!(first.min.y >= 0.0 && first.max.y <= size.y);
            }
            h.key_press(Key::Escape);
            h.run();
            assert_eq!(h.state().options, original);
        }
        h.state_mut().options.points = 37;
        h.run();
        h.get_by_label("Font size presets").click();
        h.run();
        let nearest = h.get_by_label("36 pt");
        assert_eq!(
            nearest.accesskit_node().toggled(),
            Some(eframe::egui::accesskit::Toggled::False)
        );
        let row = nearest.rect();
        assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
            Shape::Text(text) if text.galley.job.text == "36" && row.contains(text.pos)
                && shape.clip_rect.contains_rect(row))));
        h.key_press(Key::Escape);
        h.run();
        assert_eq!(h.state().options.points, 37);
    }
}

#[test]
#[ignore = "Explicit GPU text dropdown review with a deterministic catalog"]
fn export_text_dropdowns() {
    use paintcraft_classic::ClassicVariant;
    let output = std::env::temp_dir().join("paintcraft-text-dropdowns");
    std::fs::create_dir_all(&output).unwrap();
    for (theme, name) in [
        (eframe::egui::Theme::Light, "light"),
        (eframe::egui::Theme::Dark, "dark"),
    ] {
        for (size, layout) in [
            (Vec2::new(1100.0, 500.0), "desktop"),
            (Vec2::new(480.0, 360.0), "compact"),
        ] {
            let families: Vec<_> = (0..1000)
                .map(|i| format!("Fixture family {i:04}"))
                .collect();
            let options = TextOptions {
                family: families[900].clone(),
                points: 72,
                ..Default::default()
            };
            let original = options.clone();
            let mut h = Harness::builder().with_size(size).build_ui_state(
                |ui, state: &mut FontUi| {
                    paintcraft_classic::text_options(
                        ui,
                        &mut state.options,
                        &state.families,
                        &mut state.search,
                        false,
                    );
                },
                FontUi {
                    options,
                    families,
                    search: String::new(),
                },
            );
            paintcraft_ui::fonts::install(&h.ctx);
            paintcraft_ui::theme::apply(&h.ctx, theme);
            paintcraft_classic::apply_style_for_theme(&h.ctx, ClassicVariant::V11, theme);
            h.run();
            for (popup, label) in [("Font family", "family"), ("Font size presets", "size")] {
                h.get_by_label(popup).click();
                h.run();
                h.render()
                    .unwrap()
                    .save(output.join(format!("{name}-{layout}-{label}.png")))
                    .unwrap();
                h.key_press(Key::Escape);
                h.run();
                assert_eq!(h.state().options, original);
            }
        }
    }
}
