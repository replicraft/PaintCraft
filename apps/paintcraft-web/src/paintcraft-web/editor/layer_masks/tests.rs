use super::*;
use paintcraft_core::types::Rect;
use paintcraft_editor::execute::Selection;
use std::sync::Arc;
fn select(editor: &mut Editor, rect: Rect) {
    editor
        .commit(vec![Value::object([
            ("op", Value::str("selection.set_rect")),
            (
                "rect",
                Value::object([
                    ("x", Value::Int(rect.x0)),
                    ("y", Value::Int(rect.y0)),
                    ("width", Value::Int(rect.width())),
                    ("height", Value::Int(rect.height())),
                ]),
            ),
        ])])
        .unwrap();
}
fn operation(editor: &Editor, action: u8) -> LayerMaskEdit {
    let layer = LayerRef::Id(editor.target.unwrap());
    editor.layer_mask_capture(match action {
        0 => LayerMaskOp::FromSelection {
            layer,
            enabled: true,
        },
        1 => LayerMaskOp::Invert { layer },
        2 => LayerMaskOp::SetEnabled {
            layer,
            enabled: false,
        },
        _ => LayerMaskOp::Remove { layer },
    })
}
fn pixels(editor: &Editor) -> Vec<u8> {
    use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
    use paintcraft_render::spatial::{WorkspaceAdmission, with_workspace};
    // Explicit independent fixture host; never a returned quote or UI fallback.
    let budget = SharedBudget::new(1 << 20);
    with_workspace(
        WorkspaceAdmission::new(
            budget.scope(AllocationClass::SharedBudget),
            1 << 20,
            268435456,
        ),
        || paintcraft_render::render_flat(editor.session.document()),
    )
    .unwrap()
}
#[test]
fn captured_selection_mask_pixels_noop_invert_disable_remove_and_single_undo() {
    let mut editor = Editor::new(4, 2).unwrap();
    let target = editor.target.unwrap();
    let raster = editor.session.document().layers[&target].raster.clone();
    select(&mut editor, Rect::new(1, 0, 2, 1).unwrap());
    let depth = editor.session.undo_depth();
    let edit = operation(&editor, 0);
    editor.apply(Action::LayerMask(edit)).unwrap();
    assert_eq!(editor.session.undo_depth(), depth + 1);
    let field = editor.session.document().layers[&target]
        .mask
        .as_ref()
        .unwrap()
        .clone();
    assert_eq!(field.coverage().domain(), Rect::new(0, 0, 4, 2).unwrap());
    let mut expected = vec![0; 32];
    expected[4..12].fill(255);
    assert_eq!(pixels(&editor), expected);
    let source = editor.session.snapshot();
    let ticket = editor.session.ticket();
    editor
        .apply(Action::LayerMask(operation(&editor, 0)))
        .unwrap();
    assert!(Arc::ptr_eq(&source, &editor.session.snapshot()));
    assert_eq!(editor.session.ticket(), ticket);
    assert_eq!(editor.session.undo_depth(), depth + 1);
    editor.apply(Action::Undo).unwrap();
    assert!(editor.session.document().layers[&target].mask.is_none());
    editor.apply(Action::Redo).unwrap();
    assert_eq!(pixels(&editor), expected);
    select(&mut editor, Rect::new(0, 1, 1, 1).unwrap());
    assert_eq!(
        editor.session.document().layers[&target]
            .mask
            .as_ref()
            .unwrap(),
        &field
    );
    editor
        .apply(Action::LayerMask(operation(&editor, 1)))
        .unwrap();
    assert_eq!(
        editor.session.document().layers[&target]
            .mask
            .as_ref()
            .unwrap()
            .coverage()
            .coverage(1, 0),
        0
    );
    assert_eq!(
        editor.session.document().layers[&target]
            .mask
            .as_ref()
            .unwrap()
            .coverage()
            .coverage(0, 1),
        255
    );
    editor
        .apply(Action::LayerMask(operation(&editor, 2)))
        .unwrap();
    assert_eq!(pixels(&editor), vec![255; 32]);
    let bytes = paintcraft_formats::pcnd::write_compact(editor.session.document()).unwrap();
    assert_eq!(bytes[4], 7);
    assert!(
        editor
            .session
            .document()
            .same_semantics(&read_saved_mask(&bytes))
    );
    editor
        .apply(Action::LayerMask(operation(&editor, 3)))
        .unwrap();
    assert!(editor.session.document().layers[&target].mask.is_none());
    editor.apply(Action::Undo).unwrap();
    assert!(
        !editor.session.document().layers[&target]
            .mask
            .as_ref()
            .unwrap()
            .enabled()
    );
    assert_eq!(editor.session.document().layers[&target].raster, raster);
    assert_eq!(editor.target, Some(target));
}
#[test]
fn active_empty_is_real_zero_field_and_inactive_refusal_does_not_publish() {
    let mut editor = Editor::new(2, 1).unwrap();
    let before = editor.session.snapshot();
    let ticket = editor.session.ticket();
    let error = editor
        .apply(Action::LayerMask(operation(&editor, 0)))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InputSchema);
    assert!(Arc::ptr_eq(&before, &editor.session.snapshot()));
    assert_eq!(editor.session.ticket(), ticket);
    assert_eq!(editor.session.undo_depth(), 0);
    select(&mut editor, Rect::new(50, 50, 1, 1).unwrap());
    assert_eq!(editor.session.selection(), Selection::Empty);
    editor
        .apply(Action::LayerMask(operation(&editor, 0)))
        .unwrap();
    let mask = editor.session.document().layers[&editor.target.unwrap()]
        .mask
        .as_ref()
        .unwrap();
    assert!(mask.enabled());
    assert_eq!(mask.coverage().stats().runs, 0);
    assert_eq!(pixels(&editor), vec![0; 8]);
}
#[test]
fn stale_selection_epoch_owner_target_and_private_input_preserve_everything() {
    use paintcraft_input::PointerButton;
    let mut editor = Editor::new(2, 2).unwrap();
    select(&mut editor, Rect::new(0, 0, 1, 1).unwrap());
    let captured = operation(&editor, 0);
    let state = editor.session.ticket().state_id;
    select(&mut editor, Rect::new(1, 0, 1, 1).unwrap());
    assert_eq!(editor.session.ticket().state_id, state);
    assert_ne!(
        editor.session.ticket().publication_epoch,
        captured.publication_epoch
    );
    let before = editor.session.snapshot();
    let ticket = editor.session.ticket();
    let selection = editor.session.selection();
    let depth = editor.session.undo_depth();
    assert_eq!(
        editor.apply(Action::LayerMask(captured)).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    let mut foreign = operation(&editor, 0);
    foreign.owner = Uuid::new_v4();
    assert_eq!(
        editor.apply(Action::LayerMask(foreign)).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    let edit = operation(&editor, 0);
    let target = editor.target.take();
    assert_eq!(
        editor.apply(Action::LayerMask(edit)).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    editor.target = target;
    editor.tools.tool = paintcraft_tools::Tool::Pencil;
    editor
        .tools
        .pointer_down_subpixel(target.unwrap(), (256, 256), PointerButton::Primary);
    assert_eq!(
        editor
            .apply(Action::LayerMask(operation(&editor, 0)))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert!(editor.tools.gesture_in_progress());
    editor.tools.cancel_gesture();
    editor.tools.begin_text(target.unwrap(), (0, 0));
    editor.tools.type_text("keep this editable");
    let stamp = editor.tools.text_stamp();
    assert_eq!(
        editor
            .apply(Action::LayerMask(operation(&editor, 0)))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(editor.tools.text_stamp(), stamp);
    assert!(Arc::ptr_eq(&before, &editor.session.snapshot()));
    assert_eq!(editor.session.ticket(), ticket);
    assert_eq!(editor.session.selection(), selection);
    assert_eq!(editor.session.undo_depth(), depth);
    assert_eq!(editor.target, target);
}
#[test]
fn persistent_admission_failure_preserves_selection_history_and_can_retry() {
    fn refuse(_: &Document, _: &dyn Fn() -> Result<()>) -> Result<()> {
        Err(Error::new(
            ErrorCode::ResourceLimit,
            "own mask admission sentinel",
        ))
    }
    let mut editor = Editor::new(2, 2).unwrap();
    select(&mut editor, Rect::new(0, 0, 1, 1).unwrap());
    let edit = operation(&editor, 0);
    let before = editor.session.snapshot();
    let ticket = editor.session.ticket();
    let depth = editor.session.undo_depth();
    let selection = editor.session.selection();
    editor.session.set_persistent_validator(Some(refuse));
    let error = editor.apply(Action::LayerMask(edit.clone())).unwrap_err();
    assert_eq!(error.message, "own mask admission sentinel");
    assert!(Arc::ptr_eq(&before, &editor.session.snapshot()));
    assert_eq!(editor.session.ticket(), ticket);
    assert_eq!(editor.session.undo_depth(), depth);
    assert_eq!(editor.session.selection(), selection);
    editor.session.set_persistent_validator(Some(
        paintcraft_formats::pcnd::check_compact_metadata_admission,
    ));
    editor.apply(Action::LayerMask(edit)).unwrap();
    assert_eq!(editor.session.undo_depth(), depth + 1);
}

fn read_saved_mask(bytes: &[u8]) -> Document {
    use paintcraft_formats::pcnd::{self, CoverageAdmission, MetadataAdmission, ReadAdmission};
    let realm = paintcraft_core::types::resources::SharedBudget::new(1 << 20);
    pcnd::read_with_admission_checked(
        bytes,
        8,
        65536,
        1,
        ReadAdmission {
            metadata: MetadataAdmission {
                bytes: 0,
                budget: &realm,
            },
            coverage: CoverageAdmission {
                bytes: 1 << 20,
                budget: &realm,
                limits: paintcraft_raster::coverage_mask::Limits::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap()
}
