use super::*;

#[test]
fn brush_preview_is_the_published_pixels_with_one_undo_step() {
    let mut editor = Editor::new_document(32, 32).unwrap();
    editor.request(1, Action::AddLayer);
    editor.run_pending();
    let layer = editor.target().unwrap();
    editor.request(
        2,
        Action::SetOpacity {
            layer,
            opacity: 128,
        },
    );
    editor.run_pending();
    editor.select_tool(Tool::Brush);
    editor.options.size = 5;
    editor.color = Rgba([32, 64, 128, 128]);
    let base = editor.document_snapshot();
    let depth = editor.undo_depth();
    editor.pointer_down((2, 3));
    editor.pointer_move((18, 14));
    editor.pointer_move((2, 3));
    let batch = editor.preview_batch().unwrap().unwrap();
    let preview =
        paintcraft_editor::execute::execute(editor.document(), editor.selection(), &batch)
            .unwrap()
            .doc;
    assert!(editor.document().same_semantics(&base));
    let pixels = paintcraft_render::render_flat(&preview).unwrap();
    assert!((0..32).any(|y| (0..32).any(|x| {
        let a = preview.layers[&layer].raster.get(x, y).a();
        a > 0 && a < 128
    })));
    editor.pointer_up();
    editor.run_pending();
    assert_eq!(
        pixels,
        paintcraft_render::render_flat(editor.document()).unwrap()
    );
    assert_eq!(editor.undo_depth(), depth + 1);
    editor.request(3, Action::Undo);
    editor.run_pending();
    assert!(editor.document().same_semantics(&base));
}

#[test]
fn cancelled_preview_never_publishes() {
    let mut editor = Editor::new_document(16, 16).unwrap();
    editor.request(1, Action::AddLayer);
    editor.run_pending();
    editor.select_tool(Tool::Brush);
    let before = editor.document_snapshot();
    editor.pointer_down((3, 3));
    assert!(editor.preview_batch().is_some());
    editor.cancel_gesture();
    assert!(editor.preview_batch().is_none());
    assert!(editor.document().same_semantics(&before));
}
