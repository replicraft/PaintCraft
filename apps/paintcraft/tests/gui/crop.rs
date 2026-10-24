use super::*;
use eframe::egui::Role;

#[test]
fn crop_requires_selection_changes_real_pixels_and_undo_restores_selection_and_canvas() {
    let mut h = harness_with(App::new(None), 1.0, true);
    assert!(
        h.get_by_role_and_label(Role::Button, "Crop")
            .accesskit_node()
            .is_disabled()
    );
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
    assert!(
        !h.get_by_role_and_label(Role::Button, "Crop")
            .accesskit_node()
            .is_disabled()
    );
    let before = h.state().editor().document_snapshot();
    let selection = h.state().editor().selection();
    let publications = h.state().editor().publications();
    h.get_by_role_and_label(Role::Button, "Crop").click();
    h.run();
    assert_eq!(h.state().editor().document().canvas, (30, 40));
    assert_eq!(h.state().editor().publications(), publications + 1);
    assert_eq!(top_pixel(&h, 10, 10), Rgba([0, 0, 0, 255]));
    assert_eq!(
        h.state().editor().selection(),
        paintcraft_editor::execute::Selection::Inactive
    );
    assert!(
        h.get_by_role_and_label(Role::Button, "Crop")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&before));
    assert_eq!(h.state().editor().selection(), selection);
}
