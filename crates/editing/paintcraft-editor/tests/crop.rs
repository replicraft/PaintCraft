use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba, resources::SharedBudget};
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_editor::{
    execute::{self, Selection},
    session::Session,
};
use paintcraft_raster::Admission;
use std::{cell::Cell, sync::Arc};
fn batch(doc: &Document, operations: Vec<Op>) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: doc.state_id,
        base_epoch: 0,
        seed: None,
        digest: "7".repeat(64),
        operations,
    }
}
fn fill(id: uuid::Uuid, rect: Rect, color: Rgba) -> Op {
    Op::FillRect {
        layer: LayerRef::Id(id),
        rect,
        color,
        mode: Mode::Replace,
    }
}

#[test]
fn destructive_crop_preserves_layers_hidden_rgb_and_exact_undo() {
    let doc = new_document(520, 270, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let mut session = Session::new(doc);
    let prep = batch(
        session.document(),
        vec![
            fill(
                id,
                Rect::new(255, 255, 1, 1).unwrap(),
                Rgba([23, 45, 67, 0]),
            ),
            fill(
                id,
                Rect::new(300, 260, 1, 1).unwrap(),
                Rgba([20, 40, 60, 128]),
            ),
            Op::LayerAdd {
                alias: "offset".into(),
                name: "Offset hidden".into(),
                width: 40,
                height: 40,
                origin: (250, 245),
                default: Rgba([30, 60, 90, 0]),
            },
        ],
    );
    session.apply(&prep).unwrap();
    let offset = *session.document().roots.last().unwrap();
    let before = session.snapshot();
    let metadata = session.document().layers[&offset].clone();
    let b = batch(
        session.document(),
        vec![Op::CanvasCrop {
            rect: Rect::new(250, 250, 20, 18).unwrap(),
        }],
    );
    // A fresh digest/base pairing produces a deterministic derived revision;
    // the batch's epoch is the current session epoch, not the initial helper's 0.
    let mut b = b;
    b.base_epoch = session.ticket().publication_epoch;
    assert!(session.apply(&b).unwrap().committed);
    assert_eq!(session.document().canvas, (20, 18));
    assert_eq!(session.document().roots, before.roots);
    assert_eq!(session.selection(), Selection::Inactive);
    let background = &session.document().layers[&id];
    assert_eq!(background.origin, (0, 0));
    assert_eq!(background.raster.get(5, 5), Rgba([23, 45, 67, 0]));
    let layer = &session.document().layers[&offset];
    assert_eq!(layer.name, metadata.name);
    assert_eq!(layer.visible, metadata.visible);
    assert_eq!(layer.opacity, metadata.opacity);
    assert_eq!(layer.origin, (0, 0));
    assert_eq!((layer.raster.width(), layer.raster.height()), (20, 18));
    assert_eq!(layer.raster.default_pixel(), Rgba([30, 60, 90, 0]));
    let after = session.snapshot();
    session.undo().unwrap();
    assert!(session.document().same_semantics(&before));
    session.redo().unwrap();
    assert!(session.document().same_semantics(&after));
    let mut expanded = session.document().layers[&id].raster.clone();
    expanded.expand_to(520, 270).unwrap();
    assert_eq!(
        expanded.get(50, 10),
        Rgba([255; 4]),
        "discarded content cannot reappear after expansion"
    );
}

#[test]
fn crop_reads_prior_paint_then_supports_flood_duplicate_and_second_crop_in_one_batch() {
    let doc = new_document(40, 30, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let ops = vec![
        fill(
            id,
            Rect::new(12, 10, 10, 10).unwrap(),
            Rgba([30, 60, 90, 128]),
        ),
        Op::CanvasCrop {
            rect: Rect::new(10, 8, 20, 20).unwrap(),
        },
        Op::FloodFill {
            layer: LayerRef::Id(id),
            at: (3, 3),
            color: Rgba([255, 0, 0, 255]),
            mode: Mode::Replace,
            tolerance: 0,
        },
        Op::LayerDuplicate {
            layer: LayerRef::Id(id),
            alias: "copy".into(),
            name: "Copy".into(),
        },
        Op::CanvasCrop {
            rect: Rect::new(1, 1, 14, 14).unwrap(),
        },
    ];
    let result = execute::execute(
        &doc,
        Selection::Rect(Rect::new(0, 0, 40, 30).unwrap()),
        &batch(&doc, ops),
    )
    .unwrap();
    assert_eq!(result.doc.canvas, (14, 14));
    assert_eq!(result.doc.roots.len(), 2);
    for layer in result.doc.layers_in_order() {
        for y in 0..14 {
            for x in 0..14 {
                let expected = if (1..11).contains(&x) && (1..11).contains(&y) {
                    Rgba([255, 0, 0, 255])
                } else {
                    Rgba([255; 4])
                };
                assert_eq!(layer.raster.get(x, y), expected, "{x},{y}");
            }
        }
    }
    assert_eq!(result.selection, Selection::Inactive);
}

#[test]
fn crop_peak_is_admitted_before_any_private_decoded_allocation() {
    let doc = new_document(520, 8, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let b = batch(
        &doc,
        vec![
            fill(id, Rect::new(0, 0, 520, 8).unwrap(), Rgba([0, 0, 0, 255])),
            Op::CanvasCrop {
                rect: Rect::new(1, 0, 518, 8).unwrap(),
            },
        ],
    );
    let budget = SharedBudget::new(0);
    let error = execute::execute_with_admission(
        &doc,
        Selection::Inactive,
        &b,
        Admission::with_residency(10 * 65536, budget.clone()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(budget.usage().peak, 0);
    assert_eq!(budget.usage().reserved, 0);
    let mut session = Session::new(doc);
    let before = session.snapshot();
    session.set_staged_pixel_budget(0);
    assert!(session.apply(&b).is_err());
    assert!(Arc::ptr_eq(&before, &session.snapshot()));
    assert_eq!(session.undo_depth(), 0);
}

#[test]
fn uniform_crop_requires_no_pixel_storage_and_cancellation_preserves_owner() {
    let doc = new_document(1_000_000, 1_000_000, Some(Rgba([37, 19, 83, 0])), 0, POLICY).unwrap();
    let b = batch(
        &doc,
        vec![Op::CanvasCrop {
            rect: Rect::new(17, 23, 900_000, 900_000).unwrap(),
        }],
    );
    let result = execute::execute_with_admission(
        &doc,
        Selection::Inactive,
        &b,
        Admission::with_residency(0, SharedBudget::new(0)),
    )
    .unwrap();
    assert_eq!(result.doc.canvas, (900_000, 900_000));
    assert_eq!(result.doc.layers[&doc.roots[0]].raster.resident_tiles(), 0);
    let session = Session::new(doc);
    let before = session.snapshot();
    let calls = Cell::new(0);
    let error = session
        .preparation()
        .unwrap()
        .apply(&b, || {
            calls.set(calls.get() + 1);
            if calls.get() == 15 {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel crop"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert!(Arc::ptr_eq(&before, &session.snapshot()));
    assert_eq!(session.undo_depth(), 0);
}

#[test]
fn crop_matches_independent_world_space_samples_for_kept_partial_hidden_and_offcanvas_layers() {
    let mut doc = new_document(20, 16, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    for (index, (origin, width, height)) in [
        ((6, 7), 7, 6),
        ((2, 5), 8, 9),
        ((-2, -3), 12, 10),
        ((30, 30), 3, 3),
    ]
    .into_iter()
    .enumerate()
    {
        let id = uuid::Uuid::new_v4();
        let mut raster = paintcraft_raster::RasterLayer::new(width, height, Rgba([11, 22, 33, 0]));
        let mut admission = Admission::new(65536);
        for y in 0..height {
            for x in 0..width {
                if (x + y) % 3 == 0 {
                    raster
                        .blend_pixel(
                            i64::from(x),
                            i64::from(y),
                            Rgba([
                                index as u8 * 40 + x as u8,
                                y as u8,
                                87,
                                if x % 2 == 0 { 0 } else { 128 },
                            ]),
                            Mode::Replace,
                            &mut admission,
                        )
                        .unwrap();
                }
            }
        }
        doc.layers.insert(
            id,
            paintcraft_document::Layer {
                id,
                name: format!("Layer {index}"),
                origin,
                visible: index % 2 == 0,
                opacity: 100 + index as u8,
                raster,
            },
        );
        doc.roots.push(id);
    }
    let rect = Rect::new(5, 4, 12, 11).unwrap();
    let result = execute::execute(
        &doc,
        Selection::Empty,
        &batch(&doc, vec![Op::CanvasCrop { rect }]),
    )
    .unwrap();
    assert_eq!(result.doc.roots, doc.roots);
    for id in &doc.roots {
        let before = &doc.layers[id];
        let after = &result.doc.layers[id];
        assert_eq!(
            (&after.name, after.visible, after.opacity),
            (&before.name, before.visible, before.opacity)
        );
        for y in 0..11 {
            for x in 0..12 {
                let expected = if before.extent().contains(x + 5, y + 4) {
                    before
                        .raster
                        .get(x + 5 - before.origin.0, y + 4 - before.origin.1)
                } else {
                    Rgba::TRANSPARENT
                };
                let actual = if after.extent().contains(x, y) {
                    after.raster.get(x - after.origin.0, y - after.origin.1)
                } else {
                    Rgba::TRANSPARENT
                };
                assert_eq!(actual, expected, "{id} at {x},{y}");
            }
        }
    }
}

#[test]
fn full_canvas_crop_of_already_bounded_layers_is_a_semantic_noop_without_pixel_admission() {
    let doc = new_document(32, 24, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let mut session = Session::new(doc);
    session.set_staged_pixel_budget(0);
    let before = session.snapshot();
    let b = batch(
        session.document(),
        vec![Op::CanvasCrop {
            rect: Rect::new(0, 0, 32, 24).unwrap(),
        }],
    );
    assert!(!session.apply(&b).unwrap().committed);
    assert!(Arc::ptr_eq(&before, &session.snapshot()));
    assert_eq!(session.undo_depth(), 0);
}
