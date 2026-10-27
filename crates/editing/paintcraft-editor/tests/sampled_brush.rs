use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba, resources::SharedBudget};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::{execute, session::Session};
use paintcraft_history::Label;
use paintcraft_raster::sampled_brush::{self, SampledTip};
use std::cell::Cell;

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
fn setup() -> (Session, Batch) {
    let mut session =
        Session::new(new_document(8, 8, Some(Rgba([9, 8, 7, 0])), 0, POLICY).unwrap());
    let selection = Rect::new(3, 2, 2, 2).unwrap();
    let select = batch(&session, vec![Op::SelectionSetRect { rect: selection }]);
    session.apply(&select).unwrap();
    let layer = session.document().roots[0];
    let budget = SharedBudget::new(1 << 20);
    let tip = SampledTip::from_alpha(
        3,
        2,
        (1, 1),
        &[0, 64, 255, 128, 192, 1],
        &budget,
        &|| Ok(()),
    )
    .unwrap();
    let coverage = sampled_brush::coverage(
        &tip,
        &[(3, 3)],
        1,
        256,
        session.document().canvas_rect(),
        &budget,
        &|| Ok(()),
    )
    .unwrap();
    let operation = Op::MaskSpans {
        layer: LayerRef::Id(layer),
        mode: paintcraft_commands::MaskMode::SourceOver,
        color: Rgba([100, 50, 25, 128]),
        spans: coverage.spans().to_vec(),
    };
    let paint = batch(&session, vec![operation]);
    (session, paint)
}

#[test]
fn sampled_masks_match_independent_pixels_clip_selection_and_record_one_brush_undo() {
    let (mut session, paint) = setup();
    let before = session.snapshot();
    let depth = session.undo_depth();
    let selection = session.selection();
    let preview = execute::execute(&before, selection.clone(), &paint).unwrap();
    let prepared = session
        .preparation()
        .unwrap()
        .apply_mask_stroke(&paint, || Ok(()))
        .unwrap();
    assert!(session.document().same_semantics(&before));
    assert_eq!(session.undo_depth(), depth);
    let receipt = session.publish_prepared(prepared, || Ok(())).unwrap();
    assert!(receipt.committed);
    assert_eq!(session.undo_depth(), depth + 1);
    assert_eq!(
        session.history_view().entries.last().unwrap().label,
        Label::Brush
    );
    let layer = session.document().roots[0];
    for y in 0..8 {
        for x in 0..8 {
            let expected = match (x, y) {
                (3, 2) => Rgba([100, 50, 25, 32]),
                (4, 2) => Rgba([100, 50, 25, 128]),
                (3, 3) => Rgba([100, 50, 25, 96]),
                (4, 3) => Rgba([100, 50, 25, 1]),
                _ => Rgba([9, 8, 7, 0]),
            };
            assert_eq!(session.document().layers[&layer].raster.get(x, y), expected);
            assert_eq!(preview.doc.layers[&layer].raster.get(x, y), expected);
        }
    }
    let after = session.snapshot();
    session.undo().unwrap();
    assert!(session.document().same_semantics(&before));
    assert_eq!(session.selection(), selection);
    session.redo().unwrap();
    assert!(session.document().same_semantics(&after));
}

#[test]
fn ordinary_mask_batches_retain_the_existing_text_history_label() {
    let (mut session, paint) = setup();
    session.apply(&paint).unwrap();
    assert_eq!(
        session.history_view().entries.last().unwrap().label,
        Label::Text
    );
}

#[test]
fn cancelled_dropped_stale_and_non_mask_preparations_never_publish() {
    let (mut session, paint) = setup();
    let before = session.snapshot();
    let ticket = session.ticket();
    let depth = session.undo_depth();
    let total = Cell::new(0);
    drop(
        session
            .preparation()
            .unwrap()
            .apply_mask_stroke(&paint, || {
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
            .apply_mask_stroke(&paint, || {
                let n = calls.get();
                calls.set(n + 1);
                if n == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "test"))
                } else {
                    Ok(())
                }
            });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert!(session.document().same_semantics(&before));
        assert_eq!(session.ticket(), ticket);
        assert_eq!(session.undo_depth(), depth);
    }
    let dropped = session
        .preparation()
        .unwrap()
        .apply_mask_stroke(&paint, || Ok(()))
        .unwrap();
    drop(dropped);
    assert_eq!(session.ticket(), ticket);
    let stale = session
        .preparation()
        .unwrap()
        .apply_mask_stroke(&paint, || Ok(()))
        .unwrap();
    let layer = session.document().roots[0];
    let change = batch(
        &session,
        vec![Op::FillRect {
            layer: LayerRef::Id(layer),
            rect: Rect::new(3, 2, 1, 1).unwrap(),
            color: Rgba([1, 2, 3, 255]),
            mode: Mode::Replace,
        }],
    );
    assert_eq!(
        session
            .preparation()
            .unwrap()
            .apply_mask_stroke(&change, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    session.apply(&change).unwrap();
    let changed = session.snapshot();
    assert_eq!(
        session.publish_prepared(stale, || Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert!(session.document().same_semantics(&changed));
}

#[test]
fn empty_coverage_is_not_a_dirty_document_or_history_entry() {
    let (mut session, _) = setup();
    session.mark_saved();
    let before = session.ticket();
    let depth = session.undo_depth();
    let empty = batch(&session, vec![]);
    let prepared = session
        .preparation()
        .unwrap()
        .apply_mask_stroke(&empty, || Ok(()))
        .unwrap();
    assert!(
        !session
            .publish_prepared(prepared, || Ok(()))
            .unwrap()
            .committed
    );
    assert_eq!(session.ticket(), before);
    assert_eq!(session.undo_depth(), depth);
    assert!(!session.is_dirty());
    let layer = session.document().roots[0];
    let alpha_zero = batch(
        &session,
        vec![Op::MaskSpans {
            layer: LayerRef::Id(layer),
            mode: paintcraft_commands::MaskMode::SourceOver,
            color: Rgba::TRANSPARENT,
            spans: vec![paintcraft_core::types::text::MaskSpan {
                x0: 3,
                x1: 4,
                y: 2,
                coverage: 255,
            }],
        }],
    );
    let prepared = session
        .preparation()
        .unwrap()
        .apply_mask_stroke(&alpha_zero, || Ok(()))
        .unwrap();
    assert!(
        !session
            .publish_prepared(prepared, || Ok(()))
            .unwrap()
            .committed
    );
    assert_eq!(session.ticket(), before);
    assert_eq!(session.undo_depth(), depth);
    assert!(!session.is_dirty());
}

#[test]
fn first_sampled_stroke_is_one_atomic_layer_and_brush_edit_with_no_empty_layer_noop() {
    use paintcraft_core::types::text::MaskSpan;
    fn initial(session: &Session, alpha: u8, x: i64) -> Batch {
        batch(
            session,
            vec![
                Op::LayerAdd {
                    alias: "first".into(),
                    name: "Layer 1".into(),
                    width: 8,
                    height: 8,
                    origin: (0, 0),
                    default: Rgba::TRANSPARENT,
                },
                Op::MaskSpans {
                    layer: LayerRef::Alias("first".into()),
                    mode: paintcraft_commands::MaskMode::SourceOver,
                    color: Rgba([11, 22, 33, alpha]),
                    spans: vec![MaskSpan {
                        y: 2,
                        x0: x,
                        x1: x,
                        coverage: 255,
                    }],
                },
            ],
        )
    }
    let mut session = Session::new(new_document(8, 8, None, 0, POLICY).unwrap());
    let before = session.snapshot();
    let paint = initial(&session, 255, 3);
    let denied = session
        .preparation()
        .unwrap()
        .apply_initial_mask_stroke_with_admission(
            &paint,
            paintcraft_raster::Admission::new(0),
            || Ok(()),
        );
    assert_eq!(denied.unwrap_err().code, ErrorCode::ResourceLimit);
    assert!(session.document().same_semantics(&before));
    assert_eq!(session.undo_depth(), 0);
    let calls = Cell::new(0);
    drop(
        session
            .preparation()
            .unwrap()
            .apply_initial_mask_stroke(&paint, || {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .unwrap(),
    );
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let result = session
            .preparation()
            .unwrap()
            .apply_initial_mask_stroke(&paint, || {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "test"))
                } else {
                    Ok(())
                }
            });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert!(session.document().same_semantics(&before));
        assert_eq!(session.undo_depth(), 0);
    }
    for (alpha, x) in [(0, 3), (255, 30)] {
        let noop = initial(&session, alpha, x);
        let prepared = session
            .preparation()
            .unwrap()
            .apply_initial_mask_stroke(&noop, || Ok(()))
            .unwrap();
        assert!(
            !session
                .publish_prepared(prepared, || Ok(()))
                .unwrap()
                .committed
        );
        assert!(session.document().same_semantics(&before));
        assert_eq!(session.undo_depth(), 0);
    }
    let mut wrong = paint.clone();
    if let Op::LayerAdd { default, .. } = &mut wrong.operations[0] {
        *default = Rgba([255; 4]);
    }
    assert_eq!(
        session
            .preparation()
            .unwrap()
            .apply_initial_mask_stroke(&wrong, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    let prepared = session
        .preparation()
        .unwrap()
        .apply_initial_mask_stroke(&paint, || Ok(()))
        .unwrap();
    session.publish_prepared(prepared, || Ok(())).unwrap();
    assert_eq!(session.document().layers.len(), 1);
    let id = session.document().roots[0];
    assert_eq!(
        session.document().layers[&id].raster.get(3, 2),
        Rgba([11, 22, 33, 255])
    );
    assert_eq!(session.undo_depth(), 1);
    assert_eq!(
        session.history_view().entries.last().unwrap().label,
        Label::Brush
    );
    let after = session.snapshot();
    session.undo().unwrap();
    assert!(session.document().same_semantics(&before));
    session.redo().unwrap();
    assert!(session.document().same_semantics(&after));
    assert!(
        session
            .preparation()
            .unwrap()
            .apply_initial_mask_stroke(&initial(&session, 255, 4), || Ok(()))
            .is_err()
    );
}
