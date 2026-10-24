use super::*;
use eframe::egui::Role;
use paintcraft_editor::execute::Selection;
fn choose(h: &mut Harness<App>, menu: &str, label: &str) {
    h.get_by_role_and_label(Role::Button, menu).click();
    h.run();
    h.get_by_label(label).click();
    h.run();
    assert!(
        h.query_by_label(label).is_none(),
        "a real action closes its popup"
    );
}
#[test]
fn rotate_selection_and_whole_canvas_route_to_exact_pixels_once_and_undo() {
    let mut h = harness_with(App::new(None), 1.0, true);
    let ink = at(&h, 40.0, 50.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(ink), press(ink), release(ink)]);
    h.run();
    h.get_by_role_and_label(Role::Button, "Select").click();
    h.run();
    let a = at(&h, 30.0, 40.0);
    let b = at(&h, 59.0, 79.0);
    h.input_mut().events.extend([
        Event::PointerMoved(a),
        press(a),
        Event::PointerMoved(b),
        release(b),
    ]);
    h.run();
    let before = h.state().editor().document_snapshot();
    let selection = h.state().editor().selection();
    let count = publications(&h);
    choose(&mut h, "Rotate", "Rotate right 90°");
    assert_eq!(publications(&h), count + 1);
    assert_eq!(h.state().editor().document().canvas, before.canvas);
    assert_eq!(top_pixel(&h, 59, 50), Rgba([0, 0, 0, 255]));
    assert_eq!(top_pixel(&h, 40, 50), Rgba([255; 4]));
    assert_eq!(
        h.state().editor().selection(),
        Selection::Rect(paintcraft_core::types::Rect::new(30, 40, 40, 30).unwrap())
    );
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&before));
    assert_eq!(h.state().editor().selection(), selection);
    h.event(key(Key::D, Modifiers::COMMAND));
    h.run();
    let before = h.state().editor().document_snapshot();
    let count = publications(&h);
    choose(&mut h, "Rotate", "Rotate right 90°");
    assert_eq!(publications(&h), count + 1);
    assert_eq!(h.state().editor().document().canvas, (486, 1000));
    assert_eq!(top_pixel(&h, 435, 40), Rgba([0, 0, 0, 255]));
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&before));
}
