use super::*;
fn editor() -> Editor {
    Editor::with_document(
        paintcraft_document::import_image(
            2,
            1,
            &[17, 34, 51, 0, 200, 100, 50, 128],
            "native-rotation/v1",
        )
        .unwrap(),
        None,
    )
}
fn capture(e: &Editor) -> paintcraft_tools::RotateEdit {
    let t = e.session.ticket();
    paintcraft_tools::RotateEdit {
        owner: e.session.owner_id(),
        state_id: t.state_id,
        publication_epoch: t.publication_epoch,
        target: e.target,
        selection: e.selection(),
        coefficients: (0, 1 << 30),
        clear: Rgba::TRANSPARENT,
    }
}
#[test]
fn actual_captured_rotation_preserves_hidden_rgb_and_one_undo_redo() {
    let mut e = editor();
    let before = e.document_snapshot();
    assert!(e.request(19901, Action::RotateImage(capture(&e))));
    e.run_pending();
    assert_eq!(e.document().canvas, (1, 2));
    let l = &e.document().layers[&e.document().roots[0]];
    assert_eq!(l.raster.get(0, 0), Rgba([17, 34, 51, 0]));
    assert_eq!(l.raster.get(0, 1), Rgba([200, 100, 50, 128]));
    assert_eq!(e.undo_depth(), 1);
    let rotated = e.document_snapshot();
    e.request(19902, Action::Undo);
    e.run_pending();
    assert!(e.document().same_semantics(&before));
    e.request(19903, Action::Redo);
    e.run_pending();
    assert!(e.document().same_semantics(&rotated));
}
#[test]
fn queued_rotation_does_not_rebind_to_later_target_or_selection_or_aba() {
    let mut e = editor();
    e.add_layer();
    let first = e.document().roots[0];
    let second = e.document().roots[1];
    assert!(e.set_target(first));
    let edit = capture(&e);
    assert!(e.request(19910, Action::RotateImage(edit.clone())));
    assert!(e.set_target(second));
    let before = e.document_snapshot();
    let depth = e.undo_depth();
    e.run_pending();
    assert!(e.document().same_semantics(&before));
    assert_eq!(e.undo_depth(), depth);
    assert!(e.status().contains("changed"));
    assert!(!e.request(19911, Action::RotateImage(edit)));
    assert!(e.set_target(first));
    let edit = capture(&e);
    e.request(
        19912,
        Action::Select {
            corners: Some(((0, 0), (0, 0))),
        },
    );
    e.run_pending();
    assert!(!e.request(19913, Action::RotateImage(edit)));
    let edit = capture(&e);
    e.request(19914, Action::AddLayer);
    e.run_pending();
    e.request(19915, Action::Undo);
    e.run_pending();
    assert_eq!(e.document().state_id, edit.state_id);
    assert!(!e.request(19916, Action::RotateImage(edit)));
}
#[test]
fn identity_and_wrong_owner_do_not_publish_or_queue() {
    let mut e = editor();
    let before = e.document_snapshot();
    let mut edit = capture(&e);
    edit.coefficients = (1 << 30, 0);
    assert!(e.request(19920, Action::RotateImage(edit)));
    e.run_pending();
    assert!(e.document().same_semantics(&before));
    assert_eq!(e.undo_depth(), 0);
    let mut edit = capture(&e);
    edit.owner = Uuid::new_v4();
    assert!(!e.request(19921, Action::RotateImage(edit)));
    assert!(!e.has_pending_edits());
}
