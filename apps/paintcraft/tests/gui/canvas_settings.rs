use super::*;
use paintcraft::settings::{CanvasDefaults, ClassicVariant, Settings};

fn dialog(h: &mut Harness<App>, label: &str) {
    h.get_by_role_and_label(eframe::egui::Role::Button, "View")
        .click();
    h.run();
    h.get_by_label(label).click();
    h.run();
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
    h.get_by_label("New").click();
    h.run();
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
