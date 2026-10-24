use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba};
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_editor::{
    execute::{self, Selection},
    session::Session,
};
use std::{cell::Cell, sync::Arc};

fn batch(doc: &Document, operations: Vec<Op>) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: doc.state_id,
        base_epoch: 0,
        seed: None,
        digest: "1".repeat(64),
        operations,
    }
}
fn contour(doc: &Document, closed: bool, fill: Option<Rgba>) -> Op {
    Op::Contour {
        layer: LayerRef::Id(doc.roots[0]),
        points: vec![(2, 2), (12, 2), (12, 12), (2, 12)],
        closed,
        stroke: Some((Rgba([20, 80, 150, 128]), 3)),
        fill,
        mode: Mode::SourceOver,
    }
}

#[test]
fn filled_and_outlined_contour_is_one_admitted_edit_with_selection_and_undo() {
    let white = Rgba([255; 4]);
    let fill = Rgba([140, 20, 80, 96]);
    let stroke = Rgba([20, 80, 150, 128]);
    let doc = new_document(16, 16, Some(white), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let ops = vec![
        Op::SelectionSetRect {
            rect: Rect::new(4, 0, 12, 16).unwrap(),
        },
        contour(&doc, true, Some(fill)),
    ];
    let b = batch(&doc, ops);
    let mut session = Session::new(doc);
    let before = session.snapshot();
    let prepared = session.preparation().unwrap().apply(&b, || Ok(())).unwrap();
    assert!(Arc::ptr_eq(&before, &session.snapshot()));
    session.publish_prepared(prepared, || Ok(())).unwrap();
    let raster = &session.document().layers[&id].raster;
    assert_eq!(raster.get(2, 2), white, "selection excludes the left edge");
    assert_eq!(raster.get(7, 7), fill.source_over(white));
    assert_eq!(
        raster.get(12, 12),
        stroke.source_over(white),
        "corner stamps blend once"
    );
    let after = session.snapshot();
    assert_eq!(session.undo_depth(), 1);
    session.undo().unwrap();
    assert!(session.document().same_semantics(&before));
    session.redo().unwrap();
    assert!(session.document().same_semantics(&after));
}

#[test]
fn cancelled_or_denied_contours_leave_source_and_history_unchanged() {
    let doc = new_document(32, 32, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let b = batch(&doc, vec![contour(&doc, true, Some(Rgba([0, 0, 0, 255])))]);
    let mut session = Session::new(doc);
    let before = session.snapshot();
    let ticket = session.ticket();
    let calls = Cell::new(0);
    let error = session
        .preparation()
        .unwrap()
        .apply(&b, || {
            calls.set(calls.get() + 1);
            if calls.get() == 10 {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel contour"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    session.set_staged_pixel_budget(0);
    assert!(session.apply(&b).is_err());
    assert_eq!(session.ticket(), ticket);
    assert_eq!(session.undo_depth(), 0);
    assert!(Arc::ptr_eq(&before, &session.snapshot()));
}

#[test]
fn open_contour_does_not_connect_its_last_point_back_to_the_first() {
    let doc = new_document(16, 16, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let result = execute::execute(
        &doc,
        Selection::Inactive,
        &batch(&doc, vec![contour(&doc, false, None)]),
    )
    .unwrap();
    assert_eq!(result.doc.layers[&id].raster.get(2, 7), Rgba([255; 4]));
    assert_ne!(result.doc.layers[&id].raster.get(12, 7), Rgba([255; 4]));
}
