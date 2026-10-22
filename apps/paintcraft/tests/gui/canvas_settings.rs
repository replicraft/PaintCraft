use super::*;
use paintcraft::settings::{CanvasDefaults, ClassicVariant, Settings};

fn dialog(h: &mut Harness<App>, label: &str) {
    h.get_by_role_and_label(eframe::egui::Role::Button, "View")
        .click();
    h.run();
    h.get_by_label(label).scroll_to_me();
    h.run();
    h.get_by_label(label).click();
    h.run();
}

#[test]
fn responsive_canvas_dialogs_keep_actions_fixed_and_cancel_staged_dimensions_and_policy() {
    use paintcraft::settings::Appearance;
    for (size, ppp, appearance) in [
        (Vec2::new(1360.0, 900.0), 1.0, Appearance::Light),
        (Vec2::new(700.0, 500.0), 2.0, Appearance::Dark),
        (Vec2::new(360.0, 500.0), 1.0, Appearance::Dark),
        (Vec2::new(480.0, 360.0), 2.0, Appearance::Light),
    ] {
        let dir = scratch("responsive-canvas-size");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            canvas: CanvasDefaults {
                remember_last: false,
                last: Some((800, 600)),
                ..Default::default()
            },
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = Harness::builder()
            .with_size(size)
            .with_pixels_per_point(ppp)
            .build_ui_state(
                |ui, app: &mut App| {
                    app.ui(ui);
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx()
                            .request_discard("test: repeated canvas-size layout");
                    }
                },
                App::new(None).with_settings(dir.clone()),
            );
        h.run();
        let saved = std::fs::read(dir.join("settings.json")).unwrap();
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        for defaults in [false, true] {
            dialog(
                &mut h,
                if defaults {
                    "Default canvas size…"
                } else {
                    "Canvas size…"
                },
            );
            let footer = h.get_by_label("Apply").rect();
            h.run_steps(8);
            assert_eq!(
                h.get_by_label("Apply").rect(),
                footer,
                "layout must settle at {size:?}"
            );
            for label in ["Apply", "Cancel"] {
                let rect = h.get_by_label(label).rect();
                assert!(
                    rect.min.x >= 0.0
                        && rect.min.y >= 0.0
                        && rect.max.x <= size.x
                        && rect.max.y <= size.y,
                    "{label}: {rect:?} outside {size:?}"
                );
            }
            for (label, value) in [("Width", 640.0), ("Height", 480.0)] {
                h.get_by_label(label).scroll_to_me();
                h.run();
                let field = h.get_by_label(label).rect();
                assert!(
                    field.min.x >= 0.0
                        && field.min.y >= 0.0
                        && field.max.x <= size.x
                        && field.max.y < footer.min.y,
                    "{label}: {field:?} must remain reachable above the actions"
                );
                h.get_by_label(label).set_value_accesskit(value);
                h.run();
            }
            if defaults {
                h.get_by_label("Remember last canvas size").scroll_to_me();
                h.run();
                h.get_by_label("Remember last canvas size").click();
                h.run();
                assert!(h.query_by_label("Last canvas: 800 × 600 pixels").is_some());
            }
            assert_eq!(
                h.get_by_label("Apply").rect(),
                footer,
                "scrolling must not move actions"
            );
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
            if defaults {
                h.get_by_label("Cancel").click();
            } else {
                h.key_press(Key::Escape);
            }
            h.run();
            assert!(h.query_by_label("Apply").is_none());
            assert!(h.state().editor().document().same_semantics(&snapshot));
            assert_eq!(h.state().editor().history_view().ticket, ticket);
            assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
        }
        // The same narrow layout still applies one real undoable resize.
        dialog(&mut h, "Canvas size…");
        for (label, value) in [("Width", 640.0), ("Height", 480.0)] {
            h.get_by_label(label).scroll_to_me();
            h.run();
            h.get_by_label(label).set_value_accesskit(value);
            h.run();
        }
        let before = publications(&h);
        h.get_by_label("Apply").click();
        h.run();
        assert_eq!(h.state().editor().document().canvas, (640, 480));
        assert_eq!(publications(&h), before + 1);
        assert_eq!(Settings::load(&dir).0.canvas.last, Some((640, 480)));
        h.event(key(Key::Z, Modifiers::COMMAND));
        h.run();
        assert!(h.state().editor().document().same_semantics(&snapshot));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn classic_defaults_and_remembered_canvas_size_survive_restart_without_editing_the_image() {
    assert_eq!(App::new(None).editor().document().canvas, (1000, 486));
    let dir = scratch("canvas-defaults");
    let settings = Settings {
        classic_variant: Some(ClassicVariant::V11),
        canvas: CanvasDefaults {
            configured: (640, 480),
            remember_last: true,
            last: Some((800, 600)),
        },
        ..Default::default()
    };
    settings.save(&dir).unwrap();
    let mut h = harness_with(App::new(None).with_settings(dir.clone()), 2.0, true);
    assert_eq!(h.state().editor().document().canvas, (800, 600));
    let original = h.state().editor().document_snapshot();
    dialog(&mut h, "Canvas size…");
    h.get_by_label("Width").set_value_accesskit(900.0);
    h.get_by_label("Height").set_value_accesskit(700.0);
    h.run();
    h.get_by_label("Apply").click();
    h.run();
    assert_eq!(h.state().editor().document().canvas, (900, 700));
    assert_eq!(
        h.state().editor().document().document_id,
        original.document_id
    );
    assert_eq!(h.state().editor().undo_depth(), 1);
    assert_eq!(
        h.state()
            .editor()
            .history_view()
            .entries
            .last()
            .unwrap()
            .label
            .text(),
        "Resize canvas"
    );
    let snapshot = h.state().editor().document_snapshot();
    let bytes = paintcraft_formats::pcnd::write(&snapshot).unwrap();
    let reopened = paintcraft_formats::pcnd::read(&bytes).unwrap();
    assert!(reopened.same_semantics(&snapshot));
    assert_eq!(
        top_pixel(&h, 850, 650),
        Rgba([255; 4]),
        "expanded background is white and editable"
    );
    let point = h.state().canvas_rect().unwrap().min + Vec2::new(850.5, 650.5) / 2.0;
    h.input_mut()
        .events
        .extend([Event::PointerMoved(point), press(point), release(point)]);
    h.run();
    assert_eq!(top_pixel(&h, 850, 650), Rgba([0, 0, 0, 255]));
    h.get_by_label("Undo").click();
    h.run();
    assert_eq!(top_pixel(&h, 850, 650), Rgba([255; 4]));
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&original));
    assert_eq!(Settings::load(&dir).0.canvas.last, Some((900, 700)));
    classic_file_action(&mut h, "New");
    assert_eq!(h.state().editor().document().canvas, (900, 700));
    let new_snapshot = h.state().editor().document_snapshot();
    dialog(&mut h, "Default canvas size…");
    h.get_by_label("Use configured default size").click();
    h.run();
    h.get_by_label("Apply").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&new_snapshot));
    assert_eq!(h.state().editor().undo_depth(), 0);
    assert!(!h.state().editor().is_dirty());
    let restarted = App::new(None).with_settings(dir.clone());
    assert_eq!(restarted.editor().document().canvas, (640, 480));
    let saved = Settings::load(&dir).0;
    assert_eq!(saved.canvas.last, Some((900, 700)));
    dialog(&mut h, "Default canvas size…");
    h.get_by_label("Width").set_value_accesskit(512.0);
    h.run();
    h.get_by_label("Remember last canvas size").click();
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(Settings::load(&dir).0, saved);
    assert!(h.state().editor().document().same_semantics(&new_snapshot));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cancelling_canvas_resize_keeps_pixels_history_and_preferences() {
    let mut h = harness_with(App::new(None), 1.0, true);
    let before = h.state().editor().document_snapshot();
    dialog(&mut h, "Canvas size…");
    h.get_by_label("Width").set_value_accesskit(64.0);
    h.run();
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state().editor().document().same_semantics(&before));
    assert_eq!(h.state().editor().undo_depth(), 0);
    assert!(!h.state().editor().is_dirty());
}

#[test]
#[ignore = "requires a native GPU; writes a review image to /tmp"]
fn render_classic_canvas_preferences_for_review() {
    let dir = scratch("canvas-preferences-review");
    Settings {
        classic_variant: Some(ClassicVariant::V11),
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
    dialog(&mut h, "Default canvas size…");
    h.render()
        .unwrap()
        .save("/tmp/paintcraft-classic-canvas-preferences.png")
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}
