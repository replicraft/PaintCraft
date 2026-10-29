use super::*;
fn editor() -> Editor {
    let mut editor = Editor::new_document(48, 32).unwrap();
    editor.select_tool(Tool::Brush);
    editor.options.size = 14;
    editor.color = Rgba([31, 79, 151, 191]);
    editor
}
fn point(x: i64, y: i64, force: f32) -> FrameworkPoint {
    FrameworkPoint {
        at: (x * 256, y * 256),
        force,
    }
}
#[test]
fn actual_framework_pressure_preview_release_one_undo_without_native_completion() {
    let mut outputs = Vec::new();
    for changing in [false, true] {
        let mut editor = editor();
        let budget = SharedBudget::new(64 << 20);
        let before = editor.document_snapshot();
        let depth = editor.undo_depth();
        assert!(
            editor
                .begin_framework_force(point(7, 8, if changing { 0.2 } else { 1.0 }), &budget)
                .unwrap()
        );
        editor
            .append_framework_force(Some(point(27, 17, if changing { 0.8 } else { 1.0 })), false)
            .unwrap();
        let batch = editor.preview_batch().unwrap().unwrap();
        let preview =
            paintcraft_editor::execute::execute(editor.document(), editor.selection(), &batch)
                .unwrap()
                .doc;
        let expected = paintcraft_render::render_flat(&preview).unwrap();
        editor.pointer_up();
        editor.run_pending();
        assert_eq!(editor.undo_depth(), depth);
        assert!(editor.framework_force_active());
        editor.append_framework_force(None, true).unwrap();
        editor.run_pending();
        assert_eq!(
            paintcraft_render::render_flat(editor.document()).unwrap(),
            expected
        );
        assert_eq!(editor.undo_depth(), depth + 1);
        let receipt = editor.take_native_publication().unwrap();
        assert!(receipt.contact_completion().is_none());
        assert!(receipt.activation().is_none());
        editor.request(55, Action::Undo);
        editor.run_pending();
        assert!(editor.document().same_semantics(&before));
        outputs.push(expected);
    }
    assert_ne!(outputs[0], outputs[1]);
}
#[test]
fn framework_cancel_invalid_force_and_admission_failure_leave_no_history_prefix() {
    let mut editor = editor();
    let before = editor.document_snapshot();
    let depth = editor.undo_depth();
    assert!(
        editor
            .begin_framework_force(point(8, 8, 0.5), &SharedBudget::new(64 << 20))
            .unwrap()
    );
    editor.cancel_gesture();
    editor.run_pending();
    assert!(editor.document().same_semantics(&before));
    assert_eq!(editor.undo_depth(), depth);
    assert!(
        editor
            .begin_framework_force(point(8, 8, f32::NAN), &SharedBudget::new(64 << 20))
            .is_err()
    );
    assert!(!editor.framework_force_active());
    assert!(
        editor
            .begin_framework_force(point(8, 8, 0.5), &SharedBudget::new(0))
            .is_err()
    );
    editor.run_pending();
    assert!(editor.document().same_semantics(&before));
    assert_eq!(editor.undo_depth(), depth);
}
#[test]
fn selected_nonround_media_pencil_and_eraser_keep_their_original_routes() {
    let budget = SharedBudget::new(64 << 20);
    for tool in [Tool::Pencil, Tool::Eraser, Tool::Fill, Tool::Text] {
        let mut editor = editor();
        editor.select_tool(tool);
        assert!(
            !editor
                .begin_framework_force(point(8, 8, 0.5), &budget)
                .unwrap()
        );
    }
    let mut editor = editor();
    editor.options.round = false;
    assert!(
        !editor
            .begin_framework_force(point(8, 8, 0.5), &budget)
            .unwrap()
    );
    editor.options.round = true;
    editor.options.media = paintcraft_core::types::brush::BrushMedia::Oil;
    assert!(
        !editor
            .begin_framework_force(point(8, 8, 0.5), &budget)
            .unwrap()
    );
}
