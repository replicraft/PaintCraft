use super::studio::studio_harness;
use super::*;

fn button(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn secondary_brush_preview_matches_publication_and_one_undo_restores_the_stroke() {
    let (mut h, dir) = studio_harness(true);
    with_layer(&mut h);
    h.get_by_label("Color #FF0000").click_secondary();
    h.run();
    h.get_by_label("Brush").click();
    h.run();
    let a = at(&h, 180.0, 30.0);
    let b = at(&h, 240.0, 30.0);
    let before = publications(&h);
    h.event(button(a, PointerButton::Secondary, true));
    h.event(Event::PointerMoved(b));
    h.run();
    assert_eq!(publications(&h), before);
    let preview = h.state().rendered().to_vec();
    assert_eq!(top_pixel(&h, 200, 30), Rgba::TRANSPARENT);
    h.event(button(b, PointerButton::Secondary, false));
    h.run();
    assert_eq!(publications(&h), before + 1);
    assert_eq!(h.state().rendered(), preview);
    assert_eq!(top_pixel(&h, 200, 30), Rgba([255, 0, 0, 255]));
    assert_eq!(h.state().editor().color, Rgba([0, 0, 0, 255]));
    assert_eq!(h.state().editor().secondary, Rgba([255, 0, 0, 255]));
    h.get_by_label("Undo").click();
    h.run();
    assert_eq!(top_pixel(&h, 200, 30), Rgba::TRANSPARENT);
    h.get_by_label("Redo").click();
    h.run();
    assert_eq!(h.state().rendered(), preview);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unrelated_button_release_cannot_finalize_or_extend_the_active_gesture() {
    let (mut h, dir) = studio_harness(true);
    with_layer(&mut h);
    h.get_by_label("Color #FF0000").click_secondary();
    h.run();
    let a = at(&h, 180.0, 40.0);
    let b = at(&h, 200.0, 40.0);
    let wrong = at(&h, 250.0, 180.0);
    let before = publications(&h);
    h.input_mut().events.extend([
        button(a, PointerButton::Secondary, true),
        Event::PointerMoved(b),
        button(wrong, PointerButton::Primary, true),
        button(wrong, PointerButton::Primary, false),
    ]);
    h.run();
    assert!(h.state().editor().gesture_in_progress());
    assert_eq!(publications(&h), before);
    assert_eq!(h.state().editor().gesture_points(), &[(180, 40), (200, 40)]);
    h.event(button(b, PointerButton::Secondary, false));
    h.run();
    assert_eq!(publications(&h), before + 1);
    assert_eq!(top_pixel(&h, 190, 40), Rgba([255, 0, 0, 255]));
    assert_eq!(top_pixel(&h, 230, 120), Rgba::TRANSPARENT);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cancelling_secondary_gestures_releases_ownership_and_unsupported_buttons_do_nothing() {
    let (mut h, dir) = studio_harness(false);
    with_layer(&mut h);
    let a = at(&h, 180.0, 40.0);
    let before = publications(&h);
    h.event(button(a, PointerButton::Secondary, true));
    h.run();
    h.key_press(Key::Escape);
    h.run();
    h.event(button(a, PointerButton::Secondary, false));
    h.run();
    assert_eq!(publications(&h), before);
    h.get_by_label("Eraser").click();
    h.run();
    h.event(button(a, PointerButton::Secondary, true));
    h.event(button(a, PointerButton::Secondary, false));
    h.run();
    assert_eq!(publications(&h), before);
    h.get_by_label("Pencil").click();
    h.run();
    h.event(button(a, PointerButton::Primary, true));
    h.event(button(a, PointerButton::Primary, false));
    h.run();
    assert_eq!(publications(&h), before + 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn switching_documents_cancels_the_button_owner_instead_of_publishing_into_the_new_tab() {
    let (mut h, dir) = studio_harness(true);
    with_layer(&mut h);
    let snapshot = h.state().editor().document_snapshot();
    let a = at(&h, 180.0, 30.0);
    h.event(button(a, PointerButton::Secondary, true));
    h.run();
    assert!(h.state().editor().gesture_in_progress());
    h.get_by_label("New").click();
    h.run();
    assert_eq!(h.state().document_count(), 2);
    h.event(button(a, PointerButton::Secondary, false));
    h.run();
    assert!(!h.state().editor().is_dirty());
    h.get_by_label("Document 1: Untitled 1 *").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&snapshot));
    assert!(!h.state().editor().gesture_in_progress());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "requires a native GPU; writes a review image to /tmp"]
fn render_studio_secondary_stroke_for_review() {
    let (mut h, dir) = studio_harness(false);
    with_layer(&mut h);
    h.get_by_label("Color #FF0000").click_secondary();
    h.run();
    h.get_by_label("Brush").click();
    h.run();
    let a = at(&h, 180.0, 30.0);
    let b = at(&h, 240.0, 60.0);
    h.input_mut().events.extend([
        button(a, PointerButton::Secondary, true),
        Event::PointerMoved(b),
        button(b, PointerButton::Secondary, false),
    ]);
    h.run();
    h.render()
        .unwrap()
        .save("/tmp/paintcraft-secondary-stroke.png")
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn modal_entry_cancels_an_active_secondary_gesture_even_when_it_consumes_the_release() {
    let (mut h, dir) = studio_harness(true);
    with_layer(&mut h);
    let file = dir.join("unrelated.pcnd");
    std::fs::write(&file, b"precious").unwrap();
    h.state_mut().set_path_text(&file.to_string_lossy());
    let snapshot = h.state().editor().document_snapshot();
    let before = publications(&h);
    let a = at(&h, 180.0, 30.0);
    h.event(button(a, PointerButton::Secondary, true));
    h.run();
    assert!(h.state().editor().gesture_in_progress());
    h.event(key(Key::S, Modifiers::COMMAND));
    h.run();
    assert!(!h.state().editor().gesture_in_progress());
    h.event(button(a, PointerButton::Secondary, false));
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(publications(&h), before);
    assert!(h.state().editor().document().same_semantics(&snapshot));
    assert_eq!(std::fs::read(&file).unwrap(), b"precious");
    h.event(button(a, PointerButton::Primary, true));
    h.event(button(a, PointerButton::Primary, false));
    h.run();
    assert_eq!(publications(&h), before + 1);
    std::fs::remove_dir_all(dir).unwrap();
}
