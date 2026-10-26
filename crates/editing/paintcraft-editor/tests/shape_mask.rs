use paintcraft_commands::{Batch, LayerRef, Op};
use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba, text::MaskSpan};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::session::Session;
use paintcraft_history::Label;
use std::{cell::Cell, sync::Arc};
fn batch(session: &Session, operations: Vec<Op>) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: session.document().state_id,
        base_epoch: session.ticket().publication_epoch,
        seed: None,
        digest: "0".repeat(64),
        operations,
    }
}
fn mask(layer: LayerRef, color: Rgba) -> Op {
    Op::MaskSpans {
        layer,
        color,
        spans: vec![MaskSpan {
            x0: 0,
            x1: 7,
            y: 2,
            coverage: 128,
        }],
    }
}
fn initial(session: &Session, color: Rgba) -> Batch {
    batch(
        session,
        vec![
            Op::LayerAdd {
                alias: "shape".into(),
                name: "Shape".into(),
                width: 8,
                height: 8,
                origin: (0, 0),
                default: Rgba::TRANSPARENT,
            },
            mask(LayerRef::Alias("shape".into()), color),
        ],
    )
}
#[test]
fn shape_mask_obeys_existing_selection_compositing_and_one_shape_undo() {
    let mut session =
        Session::new(new_document(8, 8, Some(Rgba([9, 8, 7, 255])), 0, POLICY).unwrap());
    let select = batch(
        &session,
        vec![Op::SelectionSetRect {
            rect: Rect::new(2, 2, 2, 2).unwrap(),
        }],
    );
    session.apply(&select).unwrap();
    let before = session.snapshot();
    let selection = session.selection();
    let depth = session.undo_depth();
    let layer = session.document().roots[0];
    let paint = batch(
        &session,
        vec![mask(LayerRef::Id(layer), Rgba([200, 40, 10, 128]))],
    );
    let prepared = session
        .preparation()
        .unwrap()
        .apply_shape_mask(&paint, || Ok(()))
        .unwrap();
    assert!(Arc::ptr_eq(&before, &session.snapshot()));
    session.publish_prepared(prepared, || Ok(())).unwrap();
    assert_eq!(session.undo_depth(), depth + 1);
    assert_eq!(
        session.history_view().entries.last().unwrap().label,
        Label::Shape
    );
    assert_eq!(session.selection(), selection);
    for y in 0..8 {
        for x in 0..8 {
            let actual = session.document().layers[&layer].raster.get(x, y);
            if y == 2 && (2..4).contains(&x) {
                assert_ne!(actual, Rgba([9, 8, 7, 255]));
            } else {
                assert_eq!(actual, Rgba([9, 8, 7, 255]));
            }
        }
    }
    session.undo().unwrap();
    assert!(session.document().same_semantics(&before));
    session.redo().unwrap();
    assert_eq!(
        session.history_view().entries.last().unwrap().label,
        Label::Shape
    );
}
#[test]
fn initial_shape_is_atomic_and_ineffective_mask_does_not_create_an_empty_layer() {
    let mut session = Session::new(new_document(8, 8, None, 0, POLICY).unwrap());
    let before = session.snapshot();
    let zero = initial(&session, Rgba::TRANSPARENT);
    let prepared = session
        .preparation()
        .unwrap()
        .apply_initial_shape_mask(&zero, || Ok(()))
        .unwrap();
    assert!(
        !session
            .publish_prepared(prepared, || Ok(()))
            .unwrap()
            .committed
    );
    assert!(Arc::ptr_eq(&before, &session.snapshot()));
    assert!(!session.is_dirty());
    assert_eq!(session.undo_depth(), 0);
    let paint = initial(&session, Rgba([200, 40, 10, 255]));
    let prepared = session
        .preparation()
        .unwrap()
        .apply_initial_shape_mask(&paint, || Ok(()))
        .unwrap();
    assert!(session.document().layers.is_empty());
    session.publish_prepared(prepared, || Ok(())).unwrap();
    assert_eq!(session.document().layers.len(), 1);
    assert_eq!(session.undo_depth(), 1);
    assert_eq!(
        session.history_view().entries.last().unwrap().label,
        Label::Shape
    );
    let after = session.snapshot();
    session.undo().unwrap();
    assert!(session.document().layers.is_empty());
    session.redo().unwrap();
    assert!(session.document().same_semantics(&after));
}
#[test]
fn cancelled_stale_and_non_mask_shape_preparations_never_publish() {
    let mut session = Session::new(new_document(8, 8, None, 0, POLICY).unwrap());
    let before = session.snapshot();
    let paint = initial(&session, Rgba([1, 2, 3, 255]));
    let total = Cell::new(0);
    drop(
        session
            .preparation()
            .unwrap()
            .apply_initial_shape_mask(&paint, || {
                total.set(total.get() + 1);
                Ok(())
            })
            .unwrap(),
    );
    for stop in 0..total.get() {
        let calls = Cell::new(0);
        let result = session
            .preparation()
            .unwrap()
            .apply_initial_shape_mask(&paint, || {
                let n = calls.get();
                calls.set(n + 1);
                if n == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "test"))
                } else {
                    Ok(())
                }
            });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert!(Arc::ptr_eq(&before, &session.snapshot()));
    }
    let stale = session
        .preparation()
        .unwrap()
        .apply_initial_shape_mask(&paint, || Ok(()))
        .unwrap();
    session.apply(&paint).unwrap();
    let current = session.snapshot();
    assert_eq!(
        session.publish_prepared(stale, || Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert!(Arc::ptr_eq(&current, &session.snapshot()));
    let non_mask = batch(
        &session,
        vec![Op::SelectionSetRect {
            rect: Rect::new(0, 0, 1, 1).unwrap(),
        }],
    );
    assert_eq!(
        session
            .preparation()
            .unwrap()
            .apply_shape_mask(&non_mask, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert!(Arc::ptr_eq(&current, &session.snapshot()));
}
