//! Selection expectations are independent pixel fields; unmasked kernel results
//! supply the already-qualified paint values, never the mask's bounds or samples.
use paintcraft_commands::{Batch, LayerRef, Mode, Op, ShapeKind};
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba, brush::BrushMedia, resources::SharedBudget, shapes::ShapePreset,
    text::MaskSpan,
};
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_editor::{
    execute::{self, Selection},
    session::Session,
};
use paintcraft_raster::{
    Admission,
    selection::{BinaryMask, MaskLimits},
};
use std::{cell::Cell, sync::Arc};
const W: u32 = 32;
const H: u32 = 24;
fn document(origin: (i64, i64)) -> Document {
    let mut doc = new_document(W, H, Some(Rgba([21, 51, 91, 127])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let layer = doc.layers.get_mut(&id).unwrap();
    layer.origin = origin;
    let mut admission = Admission::new(65536);
    for y in 0..i64::from(H) {
        for x in 0..i64::from(W) {
            layer
                .raster
                .blend_pixel(
                    x,
                    y,
                    Rgba([x as u8 * 7, y as u8 * 9, 73, ((x + y) % 4 * 85) as u8]),
                    Mode::Replace,
                    &mut admission,
                )
                .unwrap();
        }
    }
    doc
}
fn batch(doc: &Document, operations: Vec<Op>) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: doc.state_id,
        base_epoch: 0,
        seed: None,
        digest: "9".repeat(64),
        operations,
    }
}
fn base_field(x: i64, y: i64) -> bool {
    (0..i64::from(W)).contains(&x) && (0..i64::from(H)).contains(&y) && (x / 3 + y / 2) % 3 != 1
}
fn selected(variant: u8, x: i64, y: i64) -> bool {
    match variant {
        0 => base_field(x, y),
        1 => (0..i64::from(W)).contains(&x) && (0..i64::from(H)).contains(&y) && !base_field(x, y),
        2 => (3..22).contains(&x) && (2..17).contains(&y) && base_field(x, y),
        3 => base_field(x - 2, y - 1),
        _ => unreachable!(),
    }
}
fn mask(variant: u8) -> BinaryMask {
    let spans = (0..i64::from(H))
        .flat_map(|y| {
            (0..i64::from(W)).filter_map(move |x| {
                base_field(x, y).then_some(MaskSpan {
                    y,
                    x0: x,
                    x1: x,
                    coverage: 255,
                })
            })
        })
        .collect::<Vec<_>>();
    let mask = BinaryMask::from_spans(
        Rect::new(0, 0, W.into(), H.into()).unwrap(),
        false,
        &spans,
        &SharedBudget::new(1 << 20),
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    match variant {
        0 => mask,
        1 => mask.inverted(),
        2 => mask
            .clipped(Rect::new(3, 2, 19, 15).unwrap())
            .unwrap()
            .unwrap(),
        3 => mask.translated(2, 1).unwrap(),
        _ => unreachable!(),
    }
}
fn operations(id: uuid::Uuid) -> Vec<Op> {
    let layer = LayerRef::Id(id);
    let color = Rgba([197, 21, 93, 111]);
    let mut ops = vec![
        Op::FillRect {
            layer: layer.clone(),
            rect: Rect::new(-2, -2, 40, 30).unwrap(),
            color,
            mode: Mode::SourceOver,
        },
        Op::FillRect {
            layer: layer.clone(),
            rect: Rect::new(1, 1, 25, 20).unwrap(),
            color: Rgba([33, 71, 123, 0]),
            mode: Mode::Replace,
        },
        Op::PencilPath {
            layer: layer.clone(),
            points: vec![(-2, 1), (28, 19), (1, 22)],
            color,
            mode: Mode::SourceOver,
        },
        Op::Contour {
            layer: layer.clone(),
            points: vec![(1, 1), (28, 3), (20, 20), (4, 14)],
            closed: true,
            stroke: Some((color, 3)),
            fill: Some(Rgba([21, 199, 43, 93])),
            mode: Mode::SourceOver,
        },
        Op::Text {
            layer: layer.clone(),
            at: (-2, 2),
            text: "Ab".into(),
            scale: 2,
            color,
            mode: Mode::SourceOver,
        },
        Op::MaskSpans {
            layer: layer.clone(),
            color,
            mode: paintcraft_commands::MaskMode::SourceOver,
            spans: (0..20)
                .map(|y| MaskSpan {
                    y,
                    x0: 0,
                    x1: 29,
                    coverage: (y * 11 + 1) as u8,
                })
                .collect(),
        },
    ];
    for mode in [
        paintcraft_commands::MaskMode::Replace,
        paintcraft_commands::MaskMode::DestinationOut,
    ] {
        ops.push(Op::MaskSpans {
            layer: layer.clone(),
            color: Rgba([11, 29, 73, 191]),
            mode,
            spans: vec![MaskSpan {
                y: 7,
                x0: -2,
                x1: 29,
                coverage: 127,
            }],
        });
    }
    for media in BrushMedia::ALL {
        ops.push(Op::StrokePath {
            media,
            layer: layer.clone(),
            points: vec![(-2, 1), (15, 18), (29, 4)],
            color,
            mode: Mode::SourceOver,
            size: 7,
            round: true,
            smooth: media == BrushMedia::Plain,
            point_scale: 1,
        });
    }
    for smooth in [false, true] {
        for round in [false, true] {
            ops.push(Op::StrokePath {
                media: BrushMedia::Plain,
                layer: layer.clone(),
                points: vec![(2, 1), (24, 18), (4, 22)],
                color: Rgba([41, 81, 121, 0]),
                mode: Mode::Replace,
                size: 5,
                round,
                smooth,
                point_scale: 1,
            });
        }
    }
    for kind in [ShapeKind::Line, ShapeKind::Rect, ShapeKind::Ellipse]
        .into_iter()
        .chain(ShapePreset::ALL.into_iter().map(ShapeKind::Preset))
    {
        ops.push(Op::Shape {
            layer: layer.clone(),
            kind,
            from: (1, 1),
            to: (28, 21),
            stroke: Some((color, 3)),
            fill: Some(Rgba([39, 97, 201, 123])),
            mode: Mode::SourceOver,
        });
    }
    ops
}
fn assert_gate(before: &Document, unrestricted: &Document, masked: &Document, variant: u8) {
    for id in &before.roots {
        let a = &before.layers[id];
        let b = &unrestricted.layers[id];
        let c = &masked.layers[id];
        assert_eq!(a.origin, c.origin);
        assert_eq!(
            (a.raster.width(), a.raster.height()),
            (c.raster.width(), c.raster.height())
        );
        for y in 0..i64::from(H) {
            for x in 0..i64::from(W) {
                let expected = if selected(variant, x + a.origin.0, y + a.origin.1) {
                    b.raster.get(x, y)
                } else {
                    a.raster.get(x, y)
                };
                assert_eq!(
                    c.raster.get(x, y),
                    expected,
                    "variant{variant}, local ({x},{y}) origin{:?}",
                    a.origin
                );
            }
        }
    }
}
#[test]
fn every_paint_route_obeys_binary_coverage_once_with_layer_origins() {
    for origin in [(-3, 1), (4, -2)] {
        let doc = document(origin);
        for variant in 0..4 {
            let selection = Selection::Mask(mask(variant));
            for (n, op) in operations(doc.roots[0]).into_iter().enumerate() {
                let b = batch(&doc, vec![op]);
                let unrestricted = execute::execute(&doc, Selection::Inactive, &b).unwrap();
                let masked = execute::execute(&doc, selection.clone(), &b)
                    .unwrap_or_else(|e| panic!("operation{n}: {e:?}"));
                assert_gate(&doc, &unrestricted.doc, &masked.doc, variant);
                assert_eq!(masked.selection, selection);
            }
        }
    }
}
#[test]
fn overlapping_translucent_operations_mask_before_logical_reads_and_replay() {
    let doc = document((-3, 1));
    let id = doc.roots[0];
    let layer = LayerRef::Id(id);
    let fill = Op::FillRect {
        layer: layer.clone(),
        rect: Rect::new(1, 1, 20, 18).unwrap(),
        color: Rgba([193, 27, 91, 111]),
        mode: Mode::SourceOver,
    };
    let ops = vec![
        fill.clone(),
        fill,
        Op::LayerDuplicate {
            layer,
            alias: "copy".into(),
            name: "Copy".into(),
        },
        Op::FillRect {
            layer: LayerRef::Alias("copy".into()),
            rect: Rect::new(1, 1, 20, 18).unwrap(),
            color: Rgba([21, 151, 33, 99]),
            mode: Mode::SourceOver,
        },
    ];
    let unrestricted =
        execute::execute(&doc, Selection::Inactive, &batch(&doc, ops.clone())).unwrap();
    let out = execute::execute(&doc, Selection::Mask(mask(0)), &batch(&doc, ops)).unwrap();
    for (before_id, after_id) in [(id, id), (id, out.aliases["copy"])] {
        let a = &doc.layers[&before_id];
        let b = &unrestricted.doc.layers[&after_id];
        let c = &out.doc.layers[&after_id];
        for y in 0..i64::from(H) {
            for x in 0..i64::from(W) {
                let expected = if selected(0, x + a.origin.0, y + a.origin.1) {
                    b.raster.get(x, y)
                } else {
                    a.raster.get(x, y)
                };
                assert_eq!(c.raster.get(x, y), expected);
            }
        }
    }
}
#[test]
fn masked_batch_failures_and_cancellation_leave_session_and_realms_unchanged() {
    let mut s = Session::new(document((0, 0)));
    let id = s.document().roots[0];
    let mut b = batch(
        s.document(),
        vec![Op::SelectionSetPolygon {
            points: vec![(1, 1), (29, 1), (1, 22)],
        }],
    );
    s.apply(&b).unwrap();
    let before = s.snapshot();
    let selection = s.selection();
    let history = s.history_view();
    b = batch(
        s.document(),
        vec![Op::FillRect {
            layer: LayerRef::Id(id),
            rect: Rect::new(0, 0, W.into(), H.into()).unwrap(),
            color: Rgba([19, 71, 173, 111]),
            mode: Mode::SourceOver,
        }],
    );
    b.base_epoch = s.ticket().publication_epoch;
    let mut total = 0;
    let count = Cell::new(0);
    let realm = SharedBudget::new(1 << 20);
    let metadata = SharedBudget::new(1 << 20);
    let admission = || {
        Admission::with_residency(65536, realm.clone()).with_paint_metadata_budget(metadata.clone())
    };
    let prepared = s
        .preparation()
        .unwrap()
        .apply_with_admission(&b, admission(), || {
            count.set(count.get() + 1);
            Ok(())
        })
        .unwrap();
    total += count.get();
    drop(prepared);
    assert_eq!(realm.usage().reserved, 0);
    assert_eq!(metadata.usage().reserved, 0);
    for at in 1..=total {
        let n = Cell::new(0);
        let err = s
            .preparation()
            .unwrap()
            .apply_with_admission(&b, admission(), || {
                n.set(n.get() + 1);
                if n.get() == at {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel masked batch",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::OperationCancelled);
        assert_eq!(realm.usage().reserved, 0);
        assert_eq!(metadata.usage().reserved, 0);
        assert_eq!(s.history_view(), history);
        assert_eq!(s.selection(), selection);
        assert!(Arc::ptr_eq(&s.snapshot(), &before));
    }
    let fail = s
        .preparation()
        .unwrap()
        .apply_with_admission(&b, Admission::with_residency(0, realm.clone()), || Ok(()))
        .unwrap_err();
    assert_eq!(fail.code, ErrorCode::ResourceLimit);
    assert_eq!(s.history_view(), history);
    assert!(Arc::ptr_eq(&s.snapshot(), &before));
}
#[test]
fn active_empty_is_noop_and_uniform_full_matches_unrestricted() {
    let doc = document((0, 0));
    for full in [false, true] {
        let selected = Selection::Mask(BinaryMask::uniform(doc.canvas_rect(), full).unwrap());
        let b = batch(&doc, operations(doc.roots[0]));
        let out = execute::execute(&doc, selected, &b).unwrap();
        if full {
            let plain = execute::execute(&doc, Selection::Inactive, &b).unwrap();
            assert!(out.doc.same_semantics(&plain.doc));
        } else {
            assert!(out.doc.same_semantics(&doc));
        }
    }
}

#[test]
fn replacement_masks_use_all_8bit_coverages_and_replay_one_atomic_edit() {
    let original = Rgba([211, 123, 71, 239]);
    for target in [Rgba::TRANSPARENT, Rgba([17, 45, 219, 87])] {
        let mut doc = new_document(260, 3, Some(original), 1, POLICY).unwrap();
        let id = doc.roots[0];
        doc.layers.get_mut(&id).unwrap().origin = (2, 1);
        let spans = (1..=255)
            .map(|coverage| MaskSpan {
                y: 1,
                x0: i64::from(coverage) + 2,
                x1: i64::from(coverage) + 2,
                coverage,
            })
            .collect();
        let b = batch(
            &doc,
            vec![Op::MaskSpans {
                layer: LayerRef::Id(id),
                color: target,
                mode: paintcraft_commands::MaskMode::Replace,
                spans,
            }],
        );
        let before = doc.clone();
        let mut session = Session::new(doc);
        let prepared = session.preparation().unwrap().apply(&b, || Ok(())).unwrap();
        let preview = prepared.document_snapshot();
        assert_eq!(session.undo_depth(), 0);
        assert!(session.snapshot().same_semantics(&before));
        session.publish_prepared(prepared, || Ok(())).unwrap();
        assert_eq!(session.undo_depth(), 1);
        assert!(session.snapshot().same_semantics(&preview));
        for x in 0..256 {
            let mut expected = original;
            for (out, (&src, &dst)) in expected
                .0
                .iter_mut()
                .zip(target.0.iter().zip(original.0.iter()))
            {
                *out = ((u32::from(src) * x + u32::from(dst) * (255 - x) + 127) / 255) as u8;
            }
            assert_eq!(
                preview.layers[&id].raster.get(i64::from(x), 0),
                expected,
                "coverage {x}"
            );
            assert_eq!(preview.layers[&id].raster.get(i64::from(x), 1), original);
        }
        session.undo().unwrap().unwrap();
        assert!(session.snapshot().same_semantics(&before));
        session.redo().unwrap().unwrap();
        assert!(session.snapshot().same_semantics(&preview));
    }
}

#[test]
fn destination_out_preserves_straight_rgb_and_has_no_dark_eraser_fringe() {
    use paintcraft_commands::MaskMode;
    for original in [
        Rgba([255; 4]),
        Rgba([31, 73, 191, 127]),
        Rgba([19, 29, 83, 0]),
    ] {
        for source_alpha in [0, 128, 255] {
            let doc = new_document(256, 1, Some(original), 1, POLICY).unwrap();
            let id = doc.roots[0];
            let spans = (1..=255)
                .map(|coverage| MaskSpan {
                    y: 0,
                    x0: i64::from(coverage),
                    x1: i64::from(coverage),
                    coverage,
                })
                .collect();
            let b = batch(
                &doc,
                vec![Op::MaskSpans {
                    layer: LayerRef::Id(id),
                    color: Rgba([0, 0, 0, source_alpha]),
                    mode: MaskMode::DestinationOut,
                    spans,
                }],
            );
            let out = execute::execute(&doc, Selection::Inactive, &b).unwrap();
            for x in 0..256 {
                let coverage = (u32::from(source_alpha) * x + 127) / 255;
                let mut expected = original;
                expected.0[3] = ((u32::from(original.a()) * (255 - coverage) + 127) / 255) as u8;
                assert_eq!(
                    out.doc.layers[&id].raster.get(i64::from(x), 0),
                    expected,
                    "coverage {x}, source alpha {source_alpha}"
                );
            }
        }
    }
}
