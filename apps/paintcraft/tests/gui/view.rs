use super::studio::studio_harness;
use super::*;
use eframe::egui::Role;

fn view_action(h: &mut Harness<App>, label: &str) {
    h.get_by_role_and_label(Role::Button, "View").click();
    h.run();
    h.get_by_role_and_label(Role::Button, "View")
        .get_by_label_contains(label)
        .click();
    h.run();
}

#[test]
fn zoom_shortcuts_menu_and_status_presets_share_mapping_without_history_changes() {
    let (mut h, dir) = studio_harness(true);
    let before = publications(&h);
    let rendered = h.state().rendered().to_vec();
    h.event(key(Key::Minus, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap().width(), 160.0);
    view_action(&mut h, "Zoom in");
    assert_eq!(h.state().canvas_rect().unwrap().width(), 320.0);
    h.event(key(Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT));
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap().width(), 640.0);
    h.get_by_label("4×").click();
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap().width(), 1280.0);
    view_action(&mut h, "Pixel grid");
    assert_eq!(h.state().rendered(), rendered);
    h.event(key(Key::Num0, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap().width(), 320.0);
    assert_eq!(publications(&h), before);
    assert!(!h.state().editor().is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn fit_selection_magnifies_and_scrolls_to_the_real_selected_rectangle() {
    let (mut h, dir) = studio_harness(true);
    h.get_by_label("Select").click();
    h.run();
    drag(&mut h, (180.0, 20.0), (220.0, 50.0));
    let selection = h.state().editor().selection();
    let before = publications(&h);
    h.event(key(Key::B, Modifiers::COMMAND | Modifiers::SHIFT));
    h.run();
    assert!(h.state().canvas_rect().unwrap().width() > 320.0);
    let canvas = h.state().canvas_rect().unwrap();
    let center = canvas.min + Vec2::new(200.5, 35.5) * (canvas.width() / 320.0);
    assert!(
        (200.0..900.0).contains(&center.x) && (200.0..850.0).contains(&center.y),
        "selection center {center:?} must be visible after fitting"
    );
    assert_eq!(h.state().editor().selection(), selection);
    assert_eq!(publications(&h), before);
    view_action(&mut h, "Zoom to window");
    let rect = h.state().canvas_rect().unwrap();
    assert!(rect.width() > 320.0 && rect.width() < 1100.0);
    assert_eq!(publications(&h), before);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn fractional_zoom_paints_the_mapped_document_pixel_and_actual_size_restores_physical_pixels() {
    let (mut h, dir) = studio_harness(false);
    with_layer(&mut h);
    h.event(key(Key::Minus, Modifiers::COMMAND));
    h.run();
    let canvas = h.state().canvas_rect().unwrap();
    let pos = canvas.min + Vec2::new(180.5, 30.5) * (canvas.width() / 320.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(pos), press(pos), release(pos)]);
    h.run();
    assert_eq!(top_pixel(&h, 180, 30), Rgba([0, 0, 0, 255]));
    assert_eq!(top_pixel(&h, 181, 30), Rgba::TRANSPARENT);
    h.event(key(Key::Num0, Modifiers::COMMAND));
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap().width(), 320.0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn fitting_large_documents_and_actual_size_work_at_multiple_display_scales() {
    use paintcraft::settings::{Experience, Settings};
    for ppp in [1.0, 2.0] {
        let dir = scratch(&format!("view-fit-{}", uuid::Uuid::new_v4()));
        Settings {
            experience: Experience::Studio,
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let doc = paintcraft::controller::Editor::new_document(1600, 1000).unwrap();
        let file = dir.join("large.pcnd");
        std::fs::write(
            &file,
            paintcraft_formats::pcnd::write(doc.document()).unwrap(),
        )
        .unwrap();
        let mut h = harness_with(App::new(Some(file)).with_settings(dir.clone()), ppp, true);
        let before = publications(&h);
        h.event(key(Key::B, Modifiers::COMMAND));
        h.run();
        let fitted = h.state().canvas_rect().unwrap();
        assert!(fitted.width() < 1100.0 && fitted.height() < 750.0);
        assert_eq!(publications(&h), before);
        h.event(key(Key::Num0, Modifiers::COMMAND));
        h.run();
        assert_eq!(h.state().canvas_rect().unwrap().width(), 1600.0 / ppp);
        assert!(!h.state().editor().is_dirty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "requires a native GPU; writes a review image to /tmp"]
fn render_studio_pixel_grid_for_review() {
    let (mut h, dir) = studio_harness(false);
    h.get_by_label("8×").click();
    h.run();
    view_action(&mut h, "Pixel grid");
    h.render()
        .unwrap()
        .save("/tmp/paintcraft-studio-pixel-grid.png")
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}
