use super::*;
use paintcraft_editor::execute::Selection;

const WHITE: Rgba = Rgba([255; 4]);

#[test]
fn clear_replaces_all_layers_and_off_canvas_pixels_and_undo_restores_them() {
    let mut editor = Editor::new_document(12, 9).unwrap();
    editor.add_layer();
    let layer = editor.target().unwrap();
    editor.pointer_down((2, 3));
    editor.pointer_up();
    editor.run_pending();
    editor.request(
        1,
        Action::SetVisible {
            layer,
            visible: false,
        },
    );
    editor.request(
        2,
        Action::ResizeCanvas {
            width: 1,
            height: 1,
        },
    );
    editor.request(
        3,
        Action::Select {
            corners: Some(((0, 0), (0, 0))),
        },
    );
    editor.run_pending();
    let source = editor.document_snapshot();
    let selection = editor.selection();
    let depth = editor.undo_depth();
    let path = PathBuf::from("clear-test.pcnd");
    editor.set_path(path.clone());
    editor.session.mark_saved_state(source.state_id);
    // A pending stroke is cancelled rather than applied to the new background.
    editor.pointer_down((0, 0));
    editor.request(4, Action::ClearCanvas { color: WHITE });
    assert_eq!(editor.run_pending(), 1);
    let cleared = editor.document_snapshot();
    assert!(!editor.gesture_in_progress());
    assert_eq!(cleared.document_id, source.document_id);
    assert_eq!(cleared.title, source.title);
    assert_eq!(editor.path(), Some(path.as_path()));
    assert_eq!(cleared.canvas, (1, 1));
    assert_eq!(cleared.roots.len(), 1);
    assert_eq!(editor.selection(), Selection::Inactive);
    assert_eq!(editor.undo_depth(), depth + 1);
    assert!(editor.is_dirty());
    let background = &cleared.layers[&editor.target().unwrap()];
    assert_eq!(background.raster.resident_tiles(), 0);
    assert_eq!(background.raster.get(0, 0), WHITE);
    editor.request(5, Action::Undo);
    editor.run_pending();
    assert!(editor.document().same_semantics(&source));
    assert_eq!(editor.selection(), selection);
    assert!(!editor.is_dirty());
    editor.request(6, Action::Redo);
    editor.run_pending();
    assert!(editor.document().same_semantics(&cleared));
}

#[test]
fn clear_is_sparse_at_maximum_size_and_repeating_it_is_a_noop() {
    let mut editor = Editor::new_document(1_048_576, 1_048_576).unwrap();
    editor.session.set_staged_pixel_budget(0);
    let original = editor.document_snapshot();
    editor.request(1, Action::ClearCanvas { color: WHITE });
    assert_eq!(editor.run_pending(), 0);
    assert!(!editor.is_dirty());
    assert_eq!(editor.document().state_id, original.state_id);
    let color = Rgba([18, 45, 67, 255]);
    editor.request(2, Action::ClearCanvas { color });
    assert_eq!(editor.run_pending(), 1);
    assert_eq!(editor.document().canvas, original.canvas);
    assert_eq!(editor.canvas_color(), color);
    let target = editor.target().unwrap();
    assert_eq!(editor.document().layers[&target].raster.resident_tiles(), 0);
    editor.request(3, Action::ClearCanvas { color });
    assert_eq!(editor.run_pending(), 0);
    assert_eq!(editor.target(), Some(target));
}
