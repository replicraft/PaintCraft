use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{
    ErrorCode, Rect, Rgba, orthogonal::OrthogonalTransform as T, resources::SharedBudget,
};
use paintcraft_document::{Document, Layer, POLICY, new_document};
use paintcraft_editor::{
    execute::{self, Selection},
    session::Session,
};
use paintcraft_raster::{Admission, RasterLayer};
use std::sync::Arc;
fn batch(doc: &Document, ops: Vec<Op>) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: doc.state_id,
        base_epoch: 0,
        seed: None,
        digest: "6".repeat(64),
        operations: ops,
    }
}
fn apply(s: &mut Session, ops: Vec<Op>) {
    let mut b = batch(s.document(), ops);
    b.base_epoch = s.ticket().publication_epoch;
    s.apply(&b).unwrap();
}
fn fill(id: uuid::Uuid, x: i64, y: i64, c: Rgba) -> Op {
    Op::FillRect {
        layer: LayerRef::Id(id),
        rect: Rect::new(x, y, 1, 1).unwrap(),
        color: c,
        mode: Mode::Replace,
    }
}
#[test]
fn selected_target_only_clears_and_overwrites_once_with_exact_history_and_off_canvas_storage() {
    let red = Rgba([255, 0, 0, 255]);
    let hidden = Rgba([17, 33, 55, 0]);
    for (t, points, size) in [
        (T::RotateRight, [(1, 0), (0, 2)], (2, 3)),
        (T::RotateLeft, [(0, 2), (1, 0)], (2, 3)),
        (T::Rotate180, [(2, 1), (0, 0)], (3, 2)),
        (T::FlipHorizontal, [(2, 0), (0, 1)], (3, 2)),
        (T::FlipVertical, [(0, 1), (2, 0)], (3, 2)),
    ] {
        for clear in [Rgba([255; 4]), Rgba::TRANSPARENT] {
            let mut doc = new_document(6, 4, Some(Rgba([255; 4])), 0, POLICY).unwrap();
            let background = doc.roots[0];
            let id = uuid::Uuid::new_v4();
            let mut raster = RasterLayer::new(5, 4, Rgba([11, 22, 33, 0]));
            let mut a = Admission::new(65536);
            raster
                .blend_pixel(0, 0, red, Mode::Replace, &mut a)
                .unwrap();
            raster
                .blend_pixel(2, 1, hidden, Mode::Replace, &mut a)
                .unwrap();
            doc.roots.push(id);
            doc.layers.insert(
                id,
                Layer {
                    id,
                    name: "Selected hidden layer".into(),
                    origin: (3, 1),
                    visible: false,
                    opacity: 123,
                    filters: Default::default(),
                    raster,
                },
            );
            let mut s = Session::new(doc);
            apply(
                &mut s,
                vec![Op::SelectionSetRect {
                    rect: Rect::new(3, 1, 3, 2).unwrap(),
                }],
            );
            let before = s.snapshot();
            let selection = s.selection();
            apply(
                &mut s,
                vec![Op::TransformSelection {
                    layer: LayerRef::Id(id),
                    transform: t,
                    clear,
                }],
            );
            let after = s.snapshot();
            assert_eq!(after.canvas, before.canvas);
            assert_eq!(after.roots, before.roots);
            let l = &after.layers[&id];
            assert_eq!(
                (&l.name, l.visible, l.opacity),
                (&before.layers[&id].name, false, 123)
            );
            assert_eq!(l.origin, (3, 1));
            assert_eq!((l.raster.width(), l.raster.height()), (5, 4));
            assert_eq!(l.raster.get(points[0].0, points[0].1), red);
            assert_eq!(l.raster.get(points[1].0, points[1].1), hidden);
            assert_eq!(l.raster.get(4, 3), before.layers[&id].raster.get(4, 3));
            if size.0 == 2 {
                assert_eq!(l.raster.get(2, 1), clear);
                assert_eq!(l.raster.get(2, 0), clear);
            }
            for y in 0..4 {
                for x in 0..6 {
                    assert_eq!(
                        after.layers[&background].raster.get(x, y),
                        before.layers[&background].raster.get(x, y)
                    );
                }
            }
            assert_eq!(
                s.selection(),
                Selection::Rect(Rect::new(3, 1, size.0, size.1).unwrap())
            );
            s.undo().unwrap();
            assert!(s.document().same_semantics(&before));
            assert_eq!(s.selection(), selection);
            s.redo().unwrap();
            assert!(s.document().same_semantics(&after));
        }
    }
}
#[test]
fn compound_reads_rotated_selection_before_flood_duplicate_whole_flip_and_second_selection_transform()
 {
    let doc = new_document(5, 4, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let red = Rgba([255, 0, 0, 255]);
    let blue = Rgba([0, 0, 255, 255]);
    let green = Rgba([0, 255, 0, 255]);
    let hidden = Rgba([17, 33, 55, 0]);
    let b = batch(
        &doc,
        vec![
            fill(id, 1, 1, red),
            fill(id, 2, 1, blue),
            fill(id, 3, 1, hidden),
            Op::SelectionSetRect {
                rect: Rect::new(1, 1, 3, 1).unwrap(),
            },
            Op::TransformSelection {
                layer: LayerRef::Id(id),
                transform: T::RotateRight,
                clear: Rgba([255; 4]),
            },
            Op::FloodFill {
                layer: LayerRef::Id(id),
                at: (1, 2),
                color: green,
                mode: Mode::Replace,
                tolerance: 0,
            },
            Op::LayerDuplicate {
                layer: LayerRef::Id(id),
                alias: "copy".into(),
                name: "Copy".into(),
            },
            Op::CanvasTransform {
                transform: T::FlipHorizontal,
            },
            Op::TransformSelection {
                layer: LayerRef::Alias("copy".into()),
                transform: T::RotateLeft,
                clear: Rgba([255; 4]),
            },
        ],
    );
    let out = execute::execute(&doc, Selection::Inactive, &b).unwrap();
    assert_eq!(out.doc.canvas, (5, 4));
    assert_eq!(
        out.selection,
        Selection::Rect(Rect::new(3, 1, 2, 1).unwrap())
    );
    let old = &out.doc.layers[&id];
    assert_eq!(old.raster.get(3, 2), green);
    assert_eq!(old.raster.get(3, 3), hidden);
    let copy = &out.doc.layers[out.doc.roots.last().unwrap()];
    assert_eq!(copy.raster.width(), 6);
    assert_eq!(copy.raster.get(3, 1), red);
    assert_eq!(copy.raster.get(4, 1), green);
    assert_eq!(copy.raster.get(5, 1), hidden);
    assert_eq!(copy.raster.get(3, 2), Rgba([255; 4]));
}
#[test]
fn inactive_empty_invalid_target_and_preallocation_denial_preserve_owner() {
    let doc = new_document(8, 6, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let op = Op::TransformSelection {
        layer: LayerRef::Id(id),
        transform: T::RotateRight,
        clear: Rgba::TRANSPARENT,
    };
    assert_eq!(
        execute::execute(&doc, Selection::Inactive, &batch(&doc, vec![op.clone()]))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    let out = execute::execute_with_admission(
        &doc,
        Selection::Empty,
        &batch(&doc, vec![op.clone()]),
        Admission::new(0),
    )
    .unwrap();
    assert!(out.doc.same_semantics(&doc));
    assert_eq!(out.selection, Selection::Empty);
    let mut session = Session::new(doc);
    apply(
        &mut session,
        vec![Op::SelectionSetRect {
            rect: Rect::new(1, 1, 3, 2).unwrap(),
        }],
    );
    let before = session.snapshot();
    let selection = session.selection();
    let mut b = batch(&before, vec![op]);
    b.base_epoch = session.ticket().publication_epoch;
    let budget = SharedBudget::new(0);
    let error = execute::execute_with_admission(
        &before,
        selection.clone(),
        &b,
        Admission::with_residency(40 * 65536, budget.clone()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(budget.usage().peak, 0);
    session.set_staged_pixel_budget(0);
    assert!(session.apply(&b).is_err());
    assert!(Arc::ptr_eq(&before, &session.snapshot()));
    assert_eq!(session.selection(), selection);
    let b = batch(
        &before,
        vec![Op::TransformSelection {
            layer: LayerRef::Id(uuid::Uuid::new_v4()),
            transform: T::FlipVertical,
            clear: Rgba::TRANSPARENT,
        }],
    );
    assert!(execute::execute(&before, Selection::Empty, &b).is_err());
}

#[test]
fn selected_region_outside_offset_layer_rebases_without_turning_new_padding_into_default_pixels() {
    let mut doc = new_document(8, 6, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = uuid::Uuid::new_v4();
    let default = Rgba([11, 22, 33, 0]);
    let red = Rgba([255, 0, 0, 255]);
    let mut raster = RasterLayer::new(5, 4, default);
    raster
        .blend_pixel(0, 0, red, Mode::Replace, &mut Admission::new(65536))
        .unwrap();
    doc.layers.insert(
        id,
        Layer {
            id,
            name: "Offset".into(),
            origin: (3, 1),
            visible: true,
            opacity: 255,
            filters: Default::default(),
            raster,
        },
    );
    doc.roots.push(id);
    let mut s = Session::new(doc);
    apply(
        &mut s,
        vec![Op::SelectionSetRect {
            rect: Rect::new(1, 0, 4, 2).unwrap(),
        }],
    );
    let before = s.snapshot();
    apply(
        &mut s,
        vec![Op::TransformSelection {
            layer: LayerRef::Id(id),
            transform: T::RotateRight,
            clear: Rgba([255; 4]),
        }],
    );
    let l = &s.document().layers[&id];
    assert_eq!(l.origin, (1, 0));
    assert_eq!((l.raster.width(), l.raster.height()), (7, 5));
    assert_eq!(l.raster.default_pixel(), default);
    assert_eq!(l.raster.get(0, 2), red);
    assert_eq!(l.raster.get(2, 1), Rgba([255; 4]));
    assert_eq!(l.raster.get(6, 0), Rgba::TRANSPARENT);
    assert_eq!(l.raster.get(6, 4), default);
    assert_eq!(
        s.selection(),
        Selection::Rect(Rect::new(1, 0, 2, 4).unwrap())
    );
    s.undo().unwrap();
    assert!(s.document().same_semantics(&before));
}
