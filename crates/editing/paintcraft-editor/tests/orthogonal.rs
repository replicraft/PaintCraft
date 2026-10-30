use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{
    ErrorCode, Rect, Rgba, orthogonal::OrthogonalTransform as T, resources::SharedBudget,
};
use paintcraft_document::{COORD_ABS_MAX, Document, Layer, POLICY, new_document};
use paintcraft_editor::{
    execute::{self, Selection},
    session::Session,
};
use paintcraft_raster::{Admission, RasterLayer};
use std::{cell::Cell, sync::Arc};
fn batch(doc: &Document, ops: Vec<Op>) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: doc.state_id,
        base_epoch: 0,
        seed: None,
        digest: "5".repeat(64),
        operations: ops,
    }
}
fn fill(id: uuid::Uuid, r: Rect, c: Rgba) -> Op {
    Op::FillRect {
        layer: LayerRef::Id(id),
        rect: r,
        color: c,
        mode: Mode::Replace,
    }
}
#[test]
fn all_orientations_keep_full_layer_extents_metadata_hidden_rgb_and_recorded_history() {
    // Known independent origin/selection cases for a 9x7 canvas and an offset 5x2 layer.
    for (t, p, s, size) in [
        (T::RotateRight, (2, -2), (3, 1), (2, 3)),
        (T::RotateLeft, (3, 6), (2, 5), (2, 3)),
        (T::Rotate180, (6, 2), (5, 3), (3, 2)),
        (T::FlipHorizontal, (6, 3), (5, 2), (3, 2)),
        (T::FlipVertical, (-2, 2), (1, 3), (3, 2)),
    ] {
        let mut doc = new_document(9, 7, Some(Rgba([255; 4])), 0, POLICY).unwrap();
        let id = uuid::Uuid::new_v4();
        let mut raster = RasterLayer::new(5, 2, Rgba([7, 8, 9, 0]));
        raster
            .blend_pixel(
                0,
                0,
                Rgba([23, 45, 67, 0]),
                Mode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        doc.roots.push(id);
        doc.layers.insert(
            id,
            Layer {
                id,
                name: "Hidden off-canvas".into(),
                origin: (-2, 3),
                visible: false,
                opacity: 123,
                filters: Default::default(),
                raster,
            },
        );
        let original = doc.clone();
        let mut session = Session::new(doc);
        session
            .apply(&batch(
                session.document(),
                vec![Op::SelectionSetRect {
                    rect: Rect::new(1, 2, 3, 2).unwrap(),
                }],
            ))
            .unwrap();
        let before = session.snapshot();
        let selection = session.selection();
        let mut b = batch(
            session.document(),
            vec![Op::CanvasTransform { transform: t }],
        );
        b.base_epoch = session.ticket().publication_epoch;
        assert!(session.apply(&b).unwrap().committed);
        let after = session.snapshot();
        assert_eq!(after.roots, before.roots);
        let l = &after.layers[&id];
        assert_eq!(l.origin, p);
        assert_eq!(
            (&l.name, l.visible, l.opacity),
            (&original.layers[&id].name, false, 123)
        );
        assert_eq!(
            session.selection(),
            Selection::Rect(Rect::from_origin_size(s.0, s.1, size.0, size.1).unwrap())
        );
        // Applying inverse reconstitutes every stored sample, including outside the canvas.
        let mut b = batch(
            session.document(),
            vec![Op::CanvasTransform {
                transform: t.inverse(),
            }],
        );
        b.base_epoch = session.ticket().publication_epoch;
        session.apply(&b).unwrap();
        assert!(session.document().same_semantics(&before));
        assert_eq!(session.selection(), selection);
        session.undo().unwrap();
        assert!(session.document().same_semantics(&after));
        session.undo().unwrap();
        assert!(session.document().same_semantics(&before));
        assert_eq!(session.selection(), selection);
        session.redo().unwrap();
        assert!(session.document().same_semantics(&after));
    }
}
#[test]
fn mixed_batch_reads_paint_rotation_flood_duplicate_crop_flip_and_later_paint_in_order() {
    let doc = new_document(3, 2, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let red = Rgba([255, 0, 0, 255]);
    let blue = Rgba([0, 0, 255, 255]);
    let hidden = Rgba([17, 33, 55, 0]);
    let green = Rgba([0, 255, 0, 255]);
    let b = batch(
        &doc,
        vec![
            fill(id, Rect::new(0, 0, 3, 1).unwrap(), red),
            fill(id, Rect::new(1, 1, 1, 1).unwrap(), hidden),
            Op::CanvasTransform {
                transform: T::RotateRight,
            },
            Op::FloodFill {
                layer: LayerRef::Id(id),
                at: (0, 2),
                color: blue,
                mode: Mode::Replace,
                tolerance: 0,
            },
            Op::LayerDuplicate {
                layer: LayerRef::Id(id),
                alias: "copy".into(),
                name: "Copy".into(),
            },
            Op::CanvasCrop {
                rect: Rect::new(0, 1, 2, 2).unwrap(),
            },
            Op::CanvasTransform {
                transform: T::FlipHorizontal,
            },
            Op::FillRect {
                layer: LayerRef::Alias("copy".into()),
                rect: Rect::new(1, 1, 1, 1).unwrap(),
                color: green,
                mode: Mode::Replace,
            },
        ],
    );
    let out = execute::execute(&doc, Selection::Inactive, &b).unwrap();
    assert_eq!(out.doc.canvas, (2, 2));
    for (index, l) in out.doc.layers_in_order().enumerate() {
        let expected = [[red, hidden], [red, if index == 0 { blue } else { green }]];
        for (y, row) in expected.iter().enumerate() {
            for (x, c) in row.iter().enumerate() {
                assert_eq!(l.raster.get(x as i64, y as i64), *c);
            }
        }
    }
}
#[test]
fn denial_and_cancellation_cannot_publish_or_allocate_a_private_prefix() {
    let doc = new_document(260, 3, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let b = batch(
        &doc,
        vec![
            fill(id, Rect::new(0, 0, 260, 3).unwrap(), Rgba([0, 0, 0, 255])),
            Op::CanvasTransform {
                transform: T::RotateRight,
            },
        ],
    );
    let budget = SharedBudget::new(0);
    let err = execute::execute_with_admission(
        &doc,
        Selection::Inactive,
        &b,
        Admission::with_residency(30 * 65536, budget.clone()),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourceLimit);
    assert_eq!(budget.usage().peak, 0);
    let mut session = Session::new(doc);
    let before = session.snapshot();
    session.set_staged_pixel_budget(0);
    assert!(session.apply(&b).is_err());
    assert!(Arc::ptr_eq(&before, &session.snapshot()));
    assert_eq!(session.undo_depth(), 0);
    let count = Cell::new(0);
    let err = execute::execute_with_admission_checked(
        &before,
        Selection::Inactive,
        &b,
        Admission::new(30 * 65536),
        &|| {
            count.set(count.get() + 1);
            if count.get() > 100 {
                Err(paintcraft_core::types::Error::new(
                    ErrorCode::OperationCancelled,
                    "test",
                ))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::OperationCancelled);
    assert!(session.document().same_semantics(&before));
}
#[test]
fn huge_uniform_and_invalid_transformed_origins_obey_sparse_and_coordinate_limits() {
    let doc = new_document(900_000, 800_000, Some(Rgba([3, 6, 9, 0])), 0, POLICY).unwrap();
    let out = execute::execute_with_admission(
        &doc,
        Selection::Inactive,
        &batch(
            &doc,
            vec![Op::CanvasTransform {
                transform: T::RotateRight,
            }],
        ),
        Admission::new(0),
    )
    .unwrap();
    assert_eq!(out.doc.canvas, (800_000, 900_000));
    assert_eq!(out.doc.layers[&doc.roots[0]].raster.resident_tiles(), 0);
    let mut doc = new_document(10, 10, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    doc.layers.get_mut(&id).unwrap().origin = (-COORD_ABS_MAX, 0);
    doc.layers.get_mut(&id).unwrap().raster = RasterLayer::new(1, 1, Rgba([255; 4]));
    let b = batch(
        &doc,
        vec![Op::CanvasTransform {
            transform: T::FlipHorizontal,
        }],
    );
    let mut session = Session::new(doc);
    let before = session.snapshot();
    assert_eq!(
        session.apply(&b).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(Arc::ptr_eq(&before, &session.snapshot()));
}
