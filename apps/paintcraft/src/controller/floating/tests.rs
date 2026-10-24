use super::*;

fn fixture() -> Editor {
    let mut editor = Editor::new_document(12, 8).unwrap();
    editor.color = Rgba([23, 45, 67, 255]);
    editor.pointer_down((2, 2));
    editor.pointer_move((4, 2));
    editor.pointer_up();
    assert_eq!(editor.run_pending(), 1);
    editor.request(
        101,
        Action::Select {
            corners: Some(((2, 2), (4, 3))),
        },
    );
    editor.run_pending();
    editor.select_tool(Tool::Select);
    editor
}
fn preview(editor: &Editor) -> Document {
    let batch = editor.preview_batch().unwrap().unwrap();
    paintcraft_editor::execute::execute(editor.document(), editor.selection(), &batch)
        .unwrap()
        .doc
}

#[test]
fn classic_move_keeps_one_frozen_source_and_publishes_one_exact_undoable_edit() {
    let mut editor = fixture();
    let base = editor.document_snapshot();
    let selection = editor.selection();
    let depth = editor.undo_depth();
    editor.pointer_down((2, 2));
    assert!(editor.has_floating_selection());
    for at in [(5, 4), (3, 2), (7, 5)] {
        editor.pointer_move(at);
        assert!(Arc::ptr_eq(&base, &editor.document_snapshot()));
        assert_eq!(editor.selection(), selection);
        assert_eq!(editor.undo_depth(), depth);
    }
    let expected = preview(&editor);
    let displayed = editor.display_selection();
    editor.pointer_up();
    editor.pointer_up();
    assert_eq!(editor.run_pending(), 1);
    assert!(editor.document().same_semantics(&expected));
    assert_eq!(editor.selection(), displayed);
    assert_eq!(editor.undo_depth(), depth + 1);
    editor.request(102, Action::Undo);
    editor.run_pending();
    assert!(editor.document().same_semantics(&base));
    assert_eq!(editor.selection(), selection);
    editor.request(103, Action::Redo);
    editor.run_pending();
    assert!(editor.document().same_semantics(&expected));
}

#[test]
fn studio_repeated_drags_use_the_original_source_until_explicit_finish() {
    let mut editor = fixture();
    editor.set_floating_commit_on_release(false);
    let base = editor.document_snapshot();
    let depth = editor.undo_depth();
    editor.pointer_down((2, 2));
    editor.pointer_move((5, 3));
    editor.pointer_up();
    assert!(editor.has_floating_selection());
    assert!(!editor.pointer_is_down());
    assert_eq!(editor.run_pending(), 0);
    assert!(Arc::ptr_eq(&base, &editor.document_snapshot()));
    editor.pointer_down((5, 3));
    editor.pointer_move((7, 4));
    editor.pointer_up();
    let expected = preview(&editor);
    assert_eq!(editor.undo_depth(), depth);
    editor.finish_selection_move();
    editor.finish_selection_move();
    assert_eq!(editor.run_pending(), 1);
    assert!(editor.document().same_semantics(&expected));
    assert_eq!(editor.undo_depth(), depth + 1);
}

#[test]
fn returning_to_origin_and_cancelling_leave_document_selection_and_history_unchanged() {
    let mut editor = fixture();
    editor.set_floating_commit_on_release(false);
    let base = editor.document_snapshot();
    let selection = editor.selection();
    let depth = editor.undo_depth();
    for cancel in [false, true] {
        editor.pointer_down((2, 2));
        editor.pointer_move((6, 5));
        editor.pointer_up();
        if cancel {
            editor.cancel_gesture();
        } else {
            editor.pointer_down((6, 5));
            editor.pointer_move((2, 2));
            editor.pointer_up();
            editor.finish_selection_move();
        }
        assert_eq!(editor.run_pending(), 0);
        assert!(Arc::ptr_eq(&base, &editor.document_snapshot()));
        assert_eq!(editor.selection(), selection);
        assert_eq!(editor.undo_depth(), depth);
    }
}

#[test]
fn cancelling_with_repeated_undo_activation_never_undoes_an_older_edit() {
    let mut editor = fixture();
    let base = editor.document_snapshot();
    let depth = editor.undo_depth();
    editor.pointer_down((2, 2));
    editor.pointer_move((7, 5));
    assert!(!editor.request(211, Action::Undo));
    assert!(!editor.request(211, Action::Undo));
    assert_eq!(editor.run_pending(), 0);
    assert!(Arc::ptr_eq(&base, &editor.document_snapshot()));
    assert_eq!(editor.undo_depth(), depth);
    assert!(!editor.has_floating_selection());
}

#[test]
fn foreign_release_and_stale_source_or_target_cannot_commit_a_move() {
    let mut editor = fixture();
    let base = editor.document_snapshot();
    editor.pointer_down((2, 2));
    editor.pointer_move((6, 4));
    editor.pointer_up_button(PointerButton::Secondary);
    assert!(editor.pointer_is_down());
    assert_eq!(editor.run_pending(), 0);
    editor.target = None;
    assert!(editor.preview().is_none());
    editor.pointer_up();
    assert_eq!(editor.run_pending(), 0);
    assert!(Arc::ptr_eq(&base, &editor.document_snapshot()));
    editor.target = Some(base.roots[0]);
    editor.pointer_down((2, 2));
    editor.pointer_move((6, 4));
    editor
        .commit(
            vec![Value::object([("op", Value::str("selection.clear"))])],
            "Competing selection",
        )
        .unwrap();
    assert!(editor.preview().is_none());
    editor.pointer_up();
    assert_eq!(editor.run_pending(), 0);
    assert!(editor.document().same_semantics(&base));
}

#[test]
fn file_guard_finishes_a_floating_draft_but_preserves_unfinished_shape_guard() {
    let mut editor = fixture();
    editor.set_floating_commit_on_release(false);
    editor.pointer_down((2, 2));
    editor.pointer_move((6, 4));
    editor.pointer_up();
    assert!(editor.shape_file_ready());
    assert_eq!(editor.run_pending(), 1);
    editor.select_tool(Tool::Polygon);
    editor.pointer_down((1, 1));
    editor.pointer_move((8, 1));
    editor.pointer_up();
    assert!(editor.is_constructing_shape());
    assert!(!editor.shape_file_ready());
    assert!(editor.is_constructing_shape());
}

#[test]
fn transparent_selection_captures_color_two_rgb_and_clear_policy_at_press() {
    let mut editor = fixture();
    editor.options.transparent_selection = true;
    editor.secondary = Rgba([23, 45, 67, 0]);
    editor.options.eraser_color = Rgba([200, 201, 202, 255]);
    let layer = editor.target().unwrap();
    editor.pointer_down((2, 2));
    editor.pointer_move((7, 4));
    let expected = preview(&editor);
    editor.options.transparent_selection = false;
    editor.secondary = Rgba([1, 2, 3, 255]);
    editor.options.eraser_color = Rgba::TRANSPARENT;
    assert!(preview(&editor).same_semantics(&expected));
    editor.pointer_up();
    assert_eq!(editor.run_pending(), 1);
    let document = editor.document();
    for x in 2..5 {
        assert_eq!(
            document.layers[&layer].raster.get(x, 2),
            Rgba([200, 201, 202, 255])
        );
    }
    for x in 7..10 {
        assert_eq!(document.layers[&layer].raster.get(x, 4), Rgba([255; 4]));
    }
    assert!(document.same_semantics(&expected));
}
