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
        assert!(h.get_by_label("Font family").rect().width() <= 220.0);
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
