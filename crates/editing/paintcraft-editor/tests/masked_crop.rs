//! Canvas crop is geometric; the selection chooses its tight bounds in the tool adapter.
use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{Rect, Rgba};
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_editor::{
    execute::{self, Selection},
    session::Session,
};
use paintcraft_raster::selection::BinaryMask;

fn batch(doc: &Document, epoch: u64, operations: Vec<Op>) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: doc.state_id,
        base_epoch: epoch,
        seed: None,
        digest: "a".repeat(64),
        operations,
    }
}

#[test]
fn crop_with_irregular_selection_preserves_all_pixels_inside_crop_and_exact_selection_history() {
    let mut session = Session::new(new_document(9, 8, Some(Rgba([255; 4])), 0, POLICY).unwrap());
    let id = session.document().roots[0];
    let prepare = batch(
        session.document(),
        0,
        vec![
            Op::FillRect {
                layer: LayerRef::Id(id),
                rect: Rect::new(5, 4, 1, 1).unwrap(),
                color: Rgba([71, 92, 113, 0]),
                mode: Mode::Replace,
            },
            Op::SelectionSetPolygon {
                points: vec![(2, 2), (6, 2), (2, 5)],
            },
        ],
    );
    session.apply(&prepare).unwrap();
    let before = session.snapshot();
    let selected = session.selection();
    let Selection::Mask(mask) = &selected else {
        panic!("mask expected")
    };
    assert_eq!(mask.coverage(5, 4), 0);
    let rect = Rect::new(2, 2, 4, 3).unwrap();
    let crop = batch(
        session.document(),
        session.ticket().publication_epoch,
        vec![Op::CanvasCrop { rect }],
    );
    let denied = execute::execute_with_budget(&before, selected.clone(), &crop, 0);
    assert!(denied.is_err());
    assert!(session.document().same_semantics(&before));
    assert_eq!(session.selection(), selected);
    session.apply(&crop).unwrap();
    let after = session.snapshot();
    assert_eq!(after.canvas, (4, 3));
    assert_eq!(session.selection(), Selection::Inactive);
    for y in 0..3 {
        for x in 0..4 {
            assert_eq!(
                after.layers[&id].raster.get(x, y),
                before.layers[&id].raster.get(x + 2, y + 2)
            );
        }
    }
    assert_eq!(after.layers[&id].raster.get(3, 2), Rgba([71, 92, 113, 0]));
    session.undo().unwrap().unwrap();
    assert!(session.document().same_semantics(&before));
    assert_eq!(session.selection(), selected);
    session.redo().unwrap().unwrap();
    assert!(session.document().same_semantics(&after));
    assert_eq!(session.selection(), Selection::Inactive);
}

#[test]
fn explicit_canvas_crop_is_independent_of_an_active_empty_mask() {
    let doc = new_document(9, 8, None, 0, POLICY).unwrap();
    let selection = Selection::Mask(BinaryMask::uniform(doc.canvas_rect(), false).unwrap());
    let result = execute::execute(
        &doc,
        selection,
        &batch(
            &doc,
            0,
            vec![Op::CanvasCrop {
                rect: Rect::new(1, 2, 3, 4).unwrap(),
            }],
        ),
    )
    .unwrap();
    assert_eq!(result.doc.canvas, (3, 4));
    assert_eq!(result.selection, Selection::Inactive);
}
