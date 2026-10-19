use super::*;
use paintcraft::settings::{Experience, Settings};

fn studio_harness(two_passes: bool) -> (Harness<'static, App>, PathBuf) {
    let dir = scratch(&format!("studio-{}", uuid::Uuid::new_v4()));
    Settings {
        experience: Experience::Studio,
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    (
        harness_with(App::new(None).with_settings(dir.clone()), 1.0, two_passes),
        dir,
    )
}

#[test]
fn studio_remembers_the_experience_without_showing_the_classic_picker() {
    let (mut h, dir) = studio_harness(false);
    assert_eq!(h.state().experience(), Experience::Studio);
    assert!(!h.state().picker_open());
    with_layer(&mut h);
    let before = h.state().editor().document_snapshot();
    let target = h.state().editor().target();
    let undo = h.state().editor().undo_depth();
    h.get_by_label("Classic").click();
    h.run();
    assert!(
        h.state().picker_open(),
        "Classic's first entry still needs its own choice"
    );
    h.get_by_label("Continue").click();
    h.run();
    h.get_by_label("Studio").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&before));
    assert_eq!(h.state().editor().target(), target);
    assert_eq!(h.state().editor().undo_depth(), undo);
    assert_eq!(Settings::load(&dir).0.experience, Experience::Studio);
    let restarted = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
    assert_eq!(restarted.state().experience(), Experience::Studio);
    assert!(!restarted.state().picker_open());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn floating_tools_do_not_paint_under_the_window_and_brush_is_undoable() {
    let (mut h, dir) = studio_harness(true);
    with_layer(&mut h);
    let before = publications(&h);
    // Move Tools over the document, then click its Brush button. A floating
    // window must own the click even where the canvas rectangle also contains it.
    let from = Pos2::new(45.0, 160.0);
    let to = h.state().canvas_rect().unwrap().min + Vec2::new(50.0, 50.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(from), press(from)]);
    h.step();
    h.event(Event::PointerMoved(to));
    h.step();
    h.event(release(to));
    h.step();
    h.run();
    h.get_by_role_and_label(eframe::egui::Role::Button, "Brush")
        .click();
    h.run();
    assert_eq!(
        publications(&h),
        before,
        "floating tool clicks must not draw through to the canvas"
    );
    assert_eq!(h.state().editor().tool, paintcraft::controller::Tool::Brush);
    drag(&mut h, (200.0, 20.0), (260.0, 40.0));
    assert_eq!(publications(&h), before + 1);
    assert!(top_pixel(&h, 230, 30).a() > 0);
    h.get_by_label("Undo").click();
    h.run();
    assert_eq!(top_pixel(&h, 230, 30), Rgba::TRANSPARENT);
    h.get_by_label("Redo").click();
    h.run();
    assert!(top_pixel(&h, 230, 30).a() > 0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn layers_window_can_be_hidden_and_reopened_without_losing_its_target() {
    let (mut h, dir) = studio_harness(false);
    with_layer(&mut h);
    let target = h.state().editor().target();
    h.get_by_label("Window").click();
    h.run();
    h.get_by_role_and_label(eframe::egui::Role::CheckBox, "Layers")
        .click();
    h.run();
    // Close and reopen the menu so the next activation is a separate real click.
    h.event(key(Key::Escape, Modifiers::NONE));
    h.step();
    h.event(key_up(Key::Escape, Modifiers::NONE));
    h.step();
    assert!(h.query_by_label("Add layer").is_none());
    h.get_by_label("Window").click();
    h.run();
    h.get_by_role_and_label(eframe::egui::Role::CheckBox, "Layers")
        .click();
    h.run();
    h.event(key(Key::Escape, Modifiers::NONE));
    h.step();
    h.event(key_up(Key::Escape, Modifiers::NONE));
    h.step();
    assert!(h.query_by_label("Add layer").is_some());
    assert_eq!(h.state().editor().target(), target);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "requires a native GPU; writes a review image to /tmp"]
fn render_studio_workspace_for_review() {
    let (mut h, dir) = studio_harness(false);
    with_layer(&mut h);
    h.get_by_role_and_label(eframe::egui::Role::Button, "Brush")
        .click();
    h.run();
    drag(&mut h, (170.0, 80.0), (260.0, 140.0));
    h.render()
        .unwrap()
        .save("/tmp/paintcraft-studio-workspace.png")
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}
