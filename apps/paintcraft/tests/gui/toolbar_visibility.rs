//! Window-free toolbar visibility, persistence and coordinate safety checks.
use super::*;
use eframe::egui::{Role, accesskit::Toggled};
use paintcraft::settings::{ClassicVariant, Experience, Settings};

fn app(dir: &std::path::Path, auto_hide: bool) -> App {
    Settings {
        classic_variant: Some(ClassicVariant::V11),
        classic_toolbar_auto_hide: auto_hide,
        ..Default::default()
    }
    .save(dir)
    .unwrap();
    App::new(None).with_settings(dir.to_path_buf())
}

fn view(h: &mut Harness<App>) {
    h.get_by_role_and_label(Role::Button, "View").click();
    h.run();
}

#[test]
fn toolbar_modes_are_exclusive_persist_and_reveal_without_document_changes() {
    let dir = scratch("toolbar-modes");
    let mut h = harness_with(app(&dir, false), 1.0, true);
    let document = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    let initial_height = h.state().canvas_viewport().unwrap().height();
    view(&mut h);
    assert_eq!(
        h.get_by_label("Always show toolbar")
            .accesskit_node()
            .toggled(),
        Some(Toggled::True)
    );
    assert_eq!(
        h.get_by_label("Automatically hide toolbar")
            .accesskit_node()
            .toggled(),
        Some(Toggled::False)
    );
    assert!(!h.get_by_label("Thumbnail").accesskit_node().is_disabled());
    h.get_by_label("Automatically hide toolbar").click();
    h.run();
    assert!(Settings::load(&dir).0.classic_toolbar_auto_hide);
    assert!(h.query_by_role_and_label(Role::Button, "Pencil").is_none());
    assert!(h.state().canvas_viewport().unwrap().height() > initial_height + 80.0);
    h.get_by_role_and_label(Role::Button, "View").hover();
    h.run();
    assert!(h.query_by_role_and_label(Role::Button, "Pencil").is_some());
    h.event(Event::PointerMoved(
        h.state().canvas_viewport().unwrap().center(),
    ));
    h.run();
    assert!(h.query_by_role_and_label(Role::Button, "Pencil").is_none());
    view(&mut h);
    assert_eq!(
        h.get_by_label("Always show toolbar")
            .accesskit_node()
            .toggled(),
        Some(Toggled::False)
    );
    assert_eq!(
        h.get_by_label("Automatically hide toolbar")
            .accesskit_node()
            .toggled(),
        Some(Toggled::True)
    );
    h.get_by_label("Always show toolbar").click();
    h.run();
    h.event(Event::PointerMoved(
        h.state().canvas_viewport().unwrap().center(),
    ));
    h.run();
    assert!(h.query_by_role_and_label(Role::Button, "Pencil").is_some());
    assert!(!Settings::load(&dir).0.classic_toolbar_auto_hide);
    assert!(h.state().editor().document().same_semantics(&document));
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    drop(h);
    let mut saved = Settings::load(&dir).0;
    saved.classic_toolbar_auto_hide = true;
    saved.save(&dir).unwrap();
    let h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, true);
    assert!(h.query_by_role_and_label(Role::Button, "Pencil").is_none());
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn auto_hiding_holds_canvas_mapping_through_coalesced_presses_and_releases() {
    for ppp in [1.0, 2.0] {
        let dir = scratch("toolbar-gesture");
        let mut h = harness_with(app(&dir, true), ppp, true);
        let before = publications(&h);
        let point = |h: &Harness<App>, x: f32, y: f32| {
            h.state().canvas_rect().unwrap().min + Vec2::new((x + 0.5) / ppp, (y + 0.5) / ppp)
        };
        let p = point(&h, 100.0, 100.0);
        h.input_mut()
            .events
            .extend([Event::PointerMoved(p), press(p), release(p)]);
        h.run();
        assert_eq!(top_pixel(&h, 100, 100), Rgba([0, 0, 0, 255]));
        h.get_by_role_and_label(Role::Button, "View").hover();
        h.run();
        assert!(h.query_by_role_and_label(Role::Button, "Pencil").is_some());
        let canvas = h.state().canvas_rect().unwrap();
        let a = point(&h, 100.0, 180.0);
        let b = point(&h, 115.0, 185.0);
        h.input_mut()
            .events
            .extend([Event::PointerMoved(a), press(a), Event::PointerMoved(b)]);
        h.run();
        assert_eq!(h.state().canvas_rect().unwrap(), canvas);
        h.event(release(b));
        h.step();
        assert_eq!(
            h.state().canvas_rect().unwrap(),
            canvas,
            "release uses the same mapping"
        );
        h.run();
        assert_eq!(top_pixel(&h, 100, 180), Rgba([0, 0, 0, 255]));
        assert_eq!(top_pixel(&h, 115, 185), Rgba([0, 0, 0, 255]));
        assert_eq!(publications(&h), before + 2);
        assert!(h.query_by_role_and_label(Role::Button, "Pencil").is_none());
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn auto_hidden_text_options_stay_revealed_while_a_preset_popup_is_open() {
    let dir = scratch("toolbar-text-popup");
    let mut h = harness_with(app(&dir, true), 1.0, true);
    h.get_by_role_and_label(Role::Button, "View").hover();
    h.run();
    h.get_by_role_and_label(Role::Button, "Text").click();
    h.run();
    h.get_by_label("Font size presets").click();
    h.run();
    let viewport = h.state().canvas_viewport().unwrap();
    h.event(Event::PointerMoved(viewport.center()));
    h.run();
    assert!(h.query_by_label("Font size").is_some());
    assert_eq!(h.state().canvas_viewport().unwrap(), viewport);
    h.get_by_label("36 pt").scroll_to_me();
    h.run();
    h.get_by_label("36 pt").click();
    h.run();
    assert_eq!(Settings::load(&dir).0.text.points, 36);
    assert!(!h.state().editor().is_dirty());
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn studio_does_not_apply_classic_toolbar_visibility_preferences() {
    let dir = scratch("studio-toolbar-visibility");
    Settings {
        experience: Experience::Studio,
        classic_toolbar_auto_hide: true,
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, true);
    h.event(Event::PointerMoved(
        h.state().canvas_viewport().unwrap().center(),
    ));
    h.run();
    assert!(h.query_by_role_and_label(Role::Button, "Pencil").is_some());
    view(&mut h);
    assert!(h.query_by_label("Automatically hide toolbar").is_none());
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn view_menu_remains_scrollable_to_lower_commands_on_short_displays() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1100.0, 360.0))
        .with_pixels_per_point(2.0)
        .build_ui_state(|ui, app: &mut App| app.ui(ui), App::new(None));
    h.run();
    view(&mut h);
    h.get_by_label("Default canvas size…").scroll_to_me();
    h.run();
    let rect = h.get_by_label("Default canvas size…").rect();
    assert!(rect.min.y >= 0.0 && rect.max.y <= 360.0, "{rect:?}");
    h.get_by_label("Default canvas size…").click();
    h.run();
    assert!(h.query_by_label("Remember last canvas size").is_some());
    h.get_by_label("Cancel").click();
    h.run();
    view(&mut h);
    h.get_by_label("Change Classic skin…").scroll_to_me();
    h.run();
    let rect = h.get_by_label("Change Classic skin…").rect();
    assert!(rect.min.y >= 0.0 && rect.max.y <= 360.0, "{rect:?}");
}

#[test]
fn ribbon_visibility_chevron_uses_persisted_modes_and_is_reachable_after_scrolling() {
    use paintcraft::settings::Appearance;
    for (width, ppp, appearance) in [
        (1360.0, 1.0, Appearance::Light),
        (1100.0, 2.0, Appearance::Dark),
        (480.0, 1.0, Appearance::Light),
    ] {
        let dir = scratch("ribbon-visibility-chevron");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = Harness::builder()
            .with_size(Vec2::new(width, 900.0))
            .with_pixels_per_point(ppp)
            .build_ui_state(
                |ui, app: &mut App| {
                    app.ui(ui);
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx()
                            .request_discard("test: repeated ribbon visibility layout");
                    }
                },
                App::new(None).with_settings(dir.clone()),
            );
        h.run();
        let document = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let initial = h.state().canvas_viewport().unwrap();
        h.get_by_label("Toolbar display options").scroll_to_me();
        h.run();
        let trigger = h.get_by_label("Toolbar display options").rect();
        assert_eq!(trigger.size(), Vec2::new(28.0, 24.0));
        assert!(trigger.min.x >= 0.0 && trigger.max.x <= width && trigger.max.y <= initial.top());
        h.get_by_label("Toolbar display options").click();
        h.run();
        assert_eq!(
            h.get_by_label("Toolbar display options")
                .accesskit_node()
                .toggled(),
            Some(Toggled::True)
        );
        for (label, selected) in [
            ("Always show toolbar", true),
            ("Automatically hide toolbar", false),
        ] {
            let node = h.get_by_label(label);
            let rect = node.rect();
            assert_eq!(rect.height(), 40.0);
            assert!(
                rect.min.x >= 0.0
                    && rect.max.x <= width
                    && rect.min.y >= 0.0
                    && rect.max.y <= 900.0
            );
            assert_eq!(
                node.accesskit_node().toggled(),
                Some(if selected {
                    Toggled::True
                } else {
                    Toggled::False
                })
            );
        }
        h.key_press(Key::Escape);
        h.run();
        assert!(h.query_by_label("Automatically hide toolbar").is_none());
        assert!(!Settings::load(&dir).0.classic_toolbar_auto_hide);
        h.get_by_label("Toolbar display options").click();
        h.run();
        h.get_by_label("Automatically hide toolbar").click();
        h.run();
        assert!(Settings::load(&dir).0.classic_toolbar_auto_hide);
        h.event(Event::PointerMoved(
            h.state().canvas_viewport().unwrap().center(),
        ));
        h.run();
        assert!(h.query_by_label("Toolbar display options").is_none());
        assert!(h.state().canvas_viewport().unwrap().height() > initial.height() + 80.0);
        h.get_by_role_and_label(Role::Button, "View").hover();
        h.run();
        h.get_by_label("Toolbar display options").scroll_to_me();
        h.run();
        h.get_by_label("Toolbar display options").click();
        h.run();
        assert_eq!(
            h.get_by_label("Automatically hide toolbar")
                .accesskit_node()
                .toggled(),
            Some(Toggled::True)
        );
        h.get_by_label("Always show toolbar").click();
        h.run();
        assert!(!Settings::load(&dir).0.classic_toolbar_auto_hide);
        assert!(h.state().editor().document().same_semantics(&document));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "requires GPU; exports ribbon visibility menus and hidden layout"]
fn export_ribbon_visibility_menu() {
    use paintcraft::settings::Appearance;
    let output = std::env::temp_dir().join("paintcraft-ribbon-visibility");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        let dir = scratch("ribbon-visibility-review");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(1360.0, 900.0));
        h.run();
        h.get_by_label("Toolbar display options").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-always-show.png")))
            .unwrap();
        h.get_by_label("Automatically hide toolbar").click();
        h.run();
        h.get_by_role_and_label(Role::Button, "View").hover();
        h.run();
        h.get_by_label("Toolbar display options").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-auto-hide.png")))
            .unwrap();
        h.key_press(Key::Escape);
        h.run();
        h.event(Event::PointerMoved(
            h.state().canvas_viewport().unwrap().center(),
        ));
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-hidden.png")))
            .unwrap();
        h.get_by_role_and_label(Role::Button, "View").hover();
        h.run();
        h.get_by_label("Toolbar display options").click();
        h.run();
        h.get_by_label("Always show toolbar").click();
        h.run();
        h.set_size(Vec2::new(480.0, 500.0));
        h.run();
        h.get_by_label("Toolbar display options").scroll_to_me();
        h.run();
        h.get_by_label("Toolbar display options").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{theme}-compact-menu.png")))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
