use super::*;
fn capture(e: &Editor) -> paintcraft_tools::StretchSkewEdit {
    let ticket = e.session.ticket();
    paintcraft_tools::StretchSkewEdit {
        owner: e.session.owner_id(),
        state_id: ticket.state_id,
        publication_epoch: ticket.publication_epoch,
        target: e.target,
        width: 4,
        height: 1,
        shear: (0, 0),
        clear: Rgba([255; 4]),
    }
}

#[test]
fn actual_controller_action_uses_one_transaction_and_recorded_undo() {
    let doc = paintcraft_document::import_image(
        2,
        1,
        &[17, 34, 51, 0, 200, 100, 50, 128],
        &"66".repeat(32),
    )
    .unwrap();
    let mut e = Editor::with_document(doc, None);
    let before = e.document_snapshot();
    e.request(9901, Action::StretchSkewImage(capture(&e)));
    e.run_pending();
    assert_eq!(e.document().canvas, (4, 1));
    let id = e.document().roots[0];
    let l = &e.document().layers[&id];
    assert_eq!(l.raster.get(0, 0), Rgba([17, 34, 51, 0]));
    assert_eq!(l.raster.get(1, 0), Rgba([17, 34, 51, 0]));
    assert_eq!(l.raster.get(3, 0), Rgba([200, 100, 50, 128]));
    assert_eq!(e.undo_depth(), 1);
    e.request(9902, Action::Undo);
    e.run_pending();
    assert!(e.document().same_semantics(&before));
}

#[test]
fn queued_transform_cannot_follow_a_changed_target_or_an_earlier_edit() {
    let mut e = Editor::new_document(2, 1).unwrap();
    e.add_layer();
    let first = e.document().roots[0];
    let second = e.document().roots[1];
    assert!(e.set_target(first));
    let before = e.document_snapshot();
    let depth = e.undo_depth();
    assert!(e.request(9910, Action::StretchSkewImage(capture(&e))));
    assert!(e.set_target(second));
    e.run_pending();
    assert!(e.document().same_semantics(&before));
    assert_eq!(e.undo_depth(), depth);
    assert!(e.status().contains("source or target changed"));
    let capture = capture(&e);
    assert!(e.request(9911, Action::AddLayer));
    assert!(e.request(9912, Action::StretchSkewImage(capture)));
    e.run_pending();
    assert_eq!(e.document().canvas, (2, 1));
    assert_eq!(e.document().roots.len(), 3);
    assert_eq!(e.undo_depth(), depth + 1);
    assert!(e.status().contains("source or target changed"));
}
#[test]
fn wrong_owner_is_rejected_before_enqueue_or_publication() {
    let mut e = Editor::new_document(2, 1).unwrap();
    let before = e.document_snapshot();
    let depth = e.undo_depth();
    let mut edit = capture(&e);
    edit.owner = uuid::Uuid::new_v4();
    assert!(!e.request(9920, Action::StretchSkewImage(edit)));
    e.run_pending();
    assert!(e.document().same_semantics(&before));
    assert_eq!(e.undo_depth(), depth);
    assert!(!e.has_pending_edits());
}
