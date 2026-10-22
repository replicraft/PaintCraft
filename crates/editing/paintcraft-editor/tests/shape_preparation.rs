use paintcraft_commands::{Batch, LayerRef, Mode, Op, ShapeKind};
use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba, resources::SharedBudget};
use paintcraft_document::{COORD_ABS_MAX, Document, POLICY, new_document};
use paintcraft_editor::session::Session;
use paintcraft_raster::{Admission, RasterLayer};
use std::cell::Cell;
fn batch(doc: &Document, operations: Vec<Op>) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: doc.state_id,
        base_epoch: 0,
        seed: None,
        digest: "0".repeat(64),
        operations,
    }
}
#[test]
fn enormous_admitted_ellipse_commits_exact_clipped_pixels_and_recorded_history() {
    let old = Rgba([31, 63, 127, 0]);
    let color = Rgba([17, 88, 156, 128]);
    let doc = new_document(8, 8, Some(old), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let batch = batch(
        &doc,
        vec![Op::Shape {
            layer: LayerRef::Id(id),
            kind: ShapeKind::Ellipse,
            from: (-COORD_ABS_MAX, -COORD_ABS_MAX),
            to: (COORD_ABS_MAX, COORD_ABS_MAX),
            fill: Some(color),
            stroke: None,
            mode: Mode::SourceOver,
        }],
    );
    let mut session = Session::new(doc);
    let base = session.ticket();
    assert!(session.apply(&batch).unwrap().committed);
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(session.document().layers[&id].raster.get(x, y), color);
        }
    }
    assert!(session.undo().unwrap().is_some());
    assert_eq!(session.ticket().state_id, base.state_id);
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(session.document().layers[&id].raster.get(x, y), old);
        }
    }
    assert!(session.redo().unwrap().is_some());
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(session.document().layers[&id].raster.get(x, y), color);
        }
    }
}
#[test]
fn cancelled_shape_rows_discard_logical_prefix_before_decoded_admission() {
    for kind in [ShapeKind::Rect, ShapeKind::Ellipse] {
        let doc = new_document(300, 300, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
        let id = doc.roots[0];
        let batch = batch(
            &doc,
            vec![
                Op::FillRect {
                    layer: LayerRef::Id(id),
                    rect: Rect::new(0, 0, 1, 1).unwrap(),
                    color: Rgba([1, 2, 3, 255]),
                    mode: Mode::Replace,
                },
                Op::Shape {
                    layer: LayerRef::Id(id),
                    kind,
                    from: (0, 0),
                    to: (299, 299),
                    fill: Some(Rgba([17, 88, 156, 128])),
                    stroke: Some((Rgba([1, 2, 3, 255]), 2)),
                    mode: Mode::SourceOver,
                },
            ],
        );
        let prefix_calls = Cell::new(0);
        let mut prefix = batch.clone();
        prefix.operations.truncate(1);
        paintcraft_editor::execute::execute_with_admission_checked(
            &doc,
            paintcraft_editor::execute::Selection::Inactive,
            &prefix,
            Admission::new(4 * 65_536),
            &|| {
                prefix_calls.set(prefix_calls.get() + 1);
                Ok(())
            },
        )
        .unwrap();
        let stop = prefix_calls.get() + 64;
        let session = Session::new(doc);
        let before = session.snapshot();
        let ticket = session.ticket();
        let history = session.history_view();
        let bytes = RasterLayer::TILE_BYTES as u64;
        let budget = SharedBudget::new(4 * bytes);
        let seen = Cell::new(0);
        let error = session
            .preparation()
            .unwrap()
            .apply_with_admission(
                &batch,
                Admission::with_residency(4 * 65_536, budget.clone()),
                || {
                    {
                        seen.set(seen.get() + 1);
                        if seen.get() == stop {
                            return Err(Error::new(
                                ErrorCode::OperationCancelled,
                                "cancel streamed shape",
                            ));
                        }
                    }
                    Ok(())
                },
            )
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.operation_index, Some(1));
        assert_eq!(budget.usage().peak, 0);
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(session.ticket(), ticket);
        assert_eq!(session.history_view(), history);
        assert!(std::sync::Arc::ptr_eq(&before, &session.snapshot()));
    }
}
