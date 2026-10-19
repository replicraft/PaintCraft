use super::*;

#[test]
fn pending_edit_makes_a_queued_history_click_stale_instead_of_erasing_new_work() {
    let mut editor = Editor::new_document(16, 16).unwrap();
    editor.request(1, Action::AddLayer);
    editor.run_pending();
    let view = editor.history_view();
    let layer = editor.target().unwrap();
    editor.request(
        2,
        Action::Stroke {
            point_scale: 1,
            layer,
            points: vec![(3, 3)],
            color: Rgba([0, 0, 0, 255]),
            size: 1,
            round: false,
        },
    );
    editor.request(
        3,
        Action::NavigateHistory {
            owner: view.ticket.owner,
            state_id: view.ticket.base.state_id,
            epoch: view.ticket.base.publication_epoch,
            history_id: None,
        },
    );
    assert_eq!(editor.run_pending(), 1);
    assert_eq!(
        editor.document().layers[&layer].raster.get(3, 3),
        Rgba([0, 0, 0, 255])
    );
    assert!(editor.status().contains("History not changed"));
}

#[test]
fn repeating_one_history_activation_does_not_navigate_twice() {
    let mut editor = Editor::new_document(16, 16).unwrap();
    editor.request(1, Action::AddLayer);
    editor.run_pending();
    let view = editor.history_view();
    let action = Action::NavigateHistory {
        owner: view.ticket.owner,
        state_id: view.ticket.base.state_id,
        epoch: view.ticket.base.publication_epoch,
        history_id: None,
    };
    assert!(editor.request(2, action.clone()));
    assert!(!editor.request(2, action));
    assert_eq!(editor.run_pending(), 1);
    assert_eq!(editor.undo_depth(), 0);
    assert!(editor.history_view().entries[1].redo);
}
