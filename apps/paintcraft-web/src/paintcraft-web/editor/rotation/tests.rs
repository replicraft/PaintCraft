use super::*;
use crate::editor::WHITE;
use paintcraft_core::types::{Rect, Rgba, rotation::COEFFICIENT_ONE};
use paintcraft_tools::Action;
use std::sync::Arc;

fn captured(editor: &Editor, coefficients: (i32, i32)) -> RotateEdit {
    let ticket = editor.session.history_ticket();
    RotateEdit {
        owner: ticket.owner,
        state_id: ticket.base.state_id,
        publication_epoch: ticket.base.publication_epoch,
        target: editor.target,
        selection: editor.session.selection(),
        coefficients,
        clear: WHITE,
    }
}
fn source() -> Editor {
    Editor::from_document(
        paintcraft_document::import_image(
            2,
            1,
            &[17, 34, 51, 0, 200, 100, 50, 128],
            &"66".repeat(32),
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn captured_rotation_cardinal_preserves_hidden_rgb_one_history_and_stale_undo() {
    let mut e = source();
    let before = e.session.snapshot();
    let edit = captured(&e, (0, COEFFICIENT_ONE));
    e.apply(Action::RotateImage(edit.clone())).unwrap();
    assert_eq!(e.session.document().canvas, (1, 2));
    let raster = &e.session.document().layers[&e.target.unwrap()].raster;
    assert_eq!(raster.get(0, 0), Rgba([17, 34, 51, 0]));
    assert_eq!(raster.get(0, 1), Rgba([200, 100, 50, 128]));
    assert_eq!(e.session.undo_depth(), 1);
    e.apply(Action::Undo).unwrap();
    assert!(e.session.document().same_semantics(&before));
    let ticket = e.session.history_ticket();
    assert_eq!(
        e.apply(Action::RotateImage(edit)).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(e.session.history_ticket(), ticket);
    e.apply(Action::Redo).unwrap();
    assert_eq!(e.session.document().canvas, (1, 2));
}
#[test]
fn identity_and_invalid_or_foreign_capture_keep_source_and_private_gesture() {
    let mut e = source();
    let before = e.session.snapshot();
    let ticket = e.session.history_ticket();
    let identity = captured(&e, (COEFFICIENT_ONE, 0));
    e.apply(Action::RotateImage(identity.clone())).unwrap();
    assert!(Arc::ptr_eq(&before, &e.session.snapshot()));
    assert_eq!(e.session.history_ticket(), ticket);
    assert_eq!(e.session.undo_depth(), 0);
    e.tools.tool = paintcraft_tools::Tool::Pencil;
    e.tools.pointer_down_subpixel(
        e.target.unwrap(),
        (0, 0),
        paintcraft_input::PointerButton::Primary,
    );
    assert!(e.tools.gesture_in_progress());
    for kind in 0..3 {
        let mut bad = identity.clone();
        match kind {
            0 => bad.owner = uuid::Uuid::new_v4(),
            1 => bad.target = None,
            _ => bad.selection = Selection::Empty,
        }
        assert_eq!(
            e.apply(Action::RotateImage(bad)).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        assert!(Arc::ptr_eq(&before, &e.session.snapshot()));
        assert_eq!(e.session.history_ticket(), ticket);
        assert!(e.tools.gesture_in_progress());
    }
    let mut invalid = identity;
    invalid.coefficients = (0, 0);
    assert_eq!(
        e.apply(Action::RotateImage(invalid)).unwrap_err().code,
        ErrorCode::InputSchema
    );
    assert_eq!(e.session.history_ticket(), ticket);
    assert!(e.tools.gesture_in_progress());
}
#[test]
fn captured_selected_rotation_clears_old_samples_without_canvas_resize() {
    let mut rgba = vec![255; 4 * 2 * 4];
    rgba[0..8].copy_from_slice(&[255, 0, 0, 255, 0, 255, 0, 255]);
    let mut e = Editor::from_document(
        paintcraft_document::import_image(4, 2, &rgba, &"77".repeat(32)).unwrap(),
    )
    .unwrap();
    e.apply(Action::Select {
        corners: Some(((0, 0), (1, 0))),
    })
    .unwrap();
    assert_eq!(
        e.session.selection(),
        Selection::Rect(Rect {
            x0: 0,
            y0: 0,
            x1: 2,
            y1: 1
        })
    );
    let before = e.session.snapshot();
    let edit = captured(&e, (0, COEFFICIENT_ONE));
    e.apply(Action::RotateImage(edit)).unwrap();
    assert_eq!(e.session.document().canvas, (4, 2));
    let raster = &e.session.document().layers[&e.target.unwrap()].raster;
    for y in 0..2 {
        for x in 0..4 {
            let expected = match (x, y) {
                (0, 0) => Rgba([255, 0, 0, 255]),
                (0, 1) => Rgba([0, 255, 0, 255]),
                _ => WHITE,
            };
            assert_eq!(raster.get(x, y), expected, "pixel{x},{y}");
        }
    }
    e.apply(Action::Undo).unwrap();
    assert!(e.session.document().same_semantics(&before));
}
#[test]
fn rotation_growing_existing_four_megapixel_canvas_refuses_without_publication() {
    let mut e = Editor::new(2048, 2048).unwrap();
    let before = e.session.snapshot();
    let ticket = e.session.history_ticket();
    let coefficients = paintcraft_core::types::rotation::coefficients_from_degrees(45.0).unwrap();
    let edit = captured(&e, coefficients);
    assert_eq!(
        e.apply(Action::RotateImage(edit)).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(Arc::ptr_eq(&before, &e.session.snapshot()));
    assert_eq!(e.session.history_ticket(), ticket);
    assert_eq!(e.session.undo_depth(), 0);
}
