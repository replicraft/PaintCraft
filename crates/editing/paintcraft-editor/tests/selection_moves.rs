use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba, resources::SharedBudget};
use paintcraft_document::{Layer, POLICY, new_document};
use paintcraft_editor::{execute::Selection, session::Session};
use paintcraft_raster::{Admission, RasterLayer};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use uuid::Uuid;

fn batch(s: &Session, ops: Vec<Op>) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: s.ticket().state_id,
        base_epoch: s.ticket().publication_epoch,
        seed: None,
        digest: format!("{:064x}", s.ticket().publication_epoch + 17),
        operations: ops,
    }
}
fn apply(s: &mut Session, ops: Vec<Op>) {
    s.apply(&batch(s, ops)).unwrap();
}
fn setup() -> (Session, Uuid) {
    let mut doc = new_document(8, 6, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    doc.title = Some("Move hidden pixels 日本語".into());
    let id = Uuid::new_v4();
    let mut raster = RasterLayer::new(5, 4, Rgba([11, 22, 33, 0]));
    for y in 0..4 {
        for x in 0..5 {
            if (x + y) % 3 != 0 {
                raster
                    .blend_pixel(
                        x,
                        y,
                        Rgba([
                            x as u8 * 39,
                            y as u8 * 51,
                            91,
                            if (x + y) % 2 == 0 { 0 } else { 128 },
                        ]),
                        Mode::Replace,
                        &mut Admission::new(65_536),
                    )
                    .unwrap();
            }
        }
    }
    doc.roots.push(id);
    doc.layers.insert(
        id,
        Layer {
            id,
            name: "Hidden target".into(),
            visible: false,
            opacity: 73,
            origin: (2, 1),
            filters: Default::default(),
            raster,
        },
    );
    (Session::new(doc), id)
}
fn selected(masked: bool) -> (Op, BTreeSet<(i64, i64)>) {
    let field = (1..5)
        .flat_map(|y| {
            (1..7)
                .filter(move |&x| !masked || !((3..5).contains(&x) && (2..4).contains(&y)))
                .map(move |x| (x, y))
        })
        .collect();
    let op = if masked {
        Op::SelectionSetPolygon {
            points: vec![
                (1, 1),
                (7, 1),
                (7, 5),
                (1, 5),
                (1, 1),
                (3, 2),
                (3, 4),
                (5, 4),
                (5, 2),
                (3, 2),
                (1, 1),
            ],
        }
    } else {
        Op::SelectionSetRect {
            rect: Rect::new(1, 1, 6, 4).unwrap(),
        }
    };
    (op, field)
}
fn move_op(id: Uuid, offset: (i64, i64), clear: Rgba, key: Option<Rgba>) -> Op {
    Op::MoveSelection {
        layer: LayerRef::Id(id),
        offset,
        clear,
        transparent_key: key,
    }
}
fn sample(layer: &Layer, (x, y): (i64, i64)) -> Rgba {
    if layer.extent().contains(x, y) {
        layer.raster.get(x - layer.origin.0, y - layer.origin.1)
    } else {
        Rgba::TRANSPARENT
    }
}
fn field(selection: &Selection) -> BTreeSet<(i64, i64)> {
    (0..6)
        .flat_map(|y| {
            (0..8)
                .filter(move |&x| match selection {
                    Selection::Inactive => panic!("active selection required"),
                    Selection::Empty => false,
                    Selection::Rect(r) => r.contains(x, y),
                    Selection::Mask(m) => m.coverage(x, y) == 255,
                })
                .map(move |x| (x, y))
        })
        .collect()
}

#[test]
fn rect_and_mask_moves_match_frozen_cut_paste_oracles_with_overlap_keys_and_offcanvas_pixels() {
    for masked in [false, true] {
        for offset in [(2, 1), (-2, -1), (0, 2), (7, 0), (-7, 0)] {
            for key in [
                None,
                Some(Rgba([11, 22, 33, 99])),
                Some(Rgba([0, 0, 0, 255])),
            ] {
                let (mut s, id) = setup();
                let (selection, selected) = selected(masked);
                apply(&mut s, vec![selection]);
                let before = s.snapshot();
                let old_selection = s.selection();
                let old = &before.layers[&id];
                let destination = Rect::new(1 + offset.0, 1 + offset.1, 6, 4).unwrap();
                let origin = (
                    old.origin.0.min(destination.x0),
                    old.origin.1.min(destination.y0),
                );
                let end = (
                    old.extent().x1.max(destination.x1),
                    old.extent().y1.max(destination.y1),
                );
                let mut expected: BTreeMap<_, _> = (origin.1..end.1)
                    .flat_map(|y| (origin.0..end.0).map(move |x| ((x, y), sample(old, (x, y)))))
                    .collect();
                let clear = Rgba([220, 230, 240, 255]);
                let capture: Vec<_> = selected
                    .iter()
                    .map(|&point| (point, sample(old, point)))
                    .collect();
                for &(point, _) in &capture {
                    if old.extent().contains(point.0, point.1) {
                        expected.insert(point, clear);
                    }
                }
                for &((x, y), pixel) in &capture {
                    if key.is_none_or(|key| pixel.0[..3] != key.0[..3]) {
                        expected.insert((x + offset.0, y + offset.1), pixel);
                    }
                }
                apply(&mut s, vec![move_op(id, offset, clear, key)]);
                let after = s.snapshot();
                let target = &after.layers[&id];
                assert_eq!(target.origin, origin);
                assert_eq!(
                    (target.raster.width(), target.raster.height()),
                    ((end.0 - origin.0) as u32, (end.1 - origin.1) as u32)
                );
                for (point, pixel) in expected {
                    assert_eq!(
                        sample(target, point),
                        pixel,
                        "masked={masked} offset={offset:?} key={key:?} at {point:?}"
                    );
                }
                assert_eq!(target.name, old.name);
                assert_eq!(target.opacity, old.opacity);
                assert_eq!(target.visible, old.visible);
                assert_eq!(target.raster.default_pixel(), old.raster.default_pixel());
                assert_eq!(after.canvas, before.canvas);
                assert_eq!(after.roots, before.roots);
                assert_eq!(after.title, before.title);
                assert_eq!(
                    after.layers[&before.roots[0]].raster,
                    before.layers[&before.roots[0]].raster
                );
                let moved: BTreeSet<_> = selected
                    .iter()
                    .map(|&(x, y)| (x + offset.0, y + offset.1))
                    .filter(|&(x, y)| (0..8).contains(&x) && (0..6).contains(&y))
                    .collect();
                assert_eq!(
                    field(&s.selection()),
                    moved,
                    "key never changes the moved selection field"
                );
                assert_eq!(
                    s.history_view().entries.last().unwrap().label,
                    paintcraft_history::Label::MoveSelection
                );
                s.undo().unwrap();
                assert!(s.document().same_semantics(&before));
                assert_eq!(s.selection(), old_selection);
                s.redo().unwrap();
                assert!(s.document().same_semantics(&after));
                assert_eq!(field(&s.selection()), moved);
            }
        }
    }
}

#[test]
fn zero_offset_empty_and_inactive_are_distinct_and_zero_needs_no_plans_or_pixel_admission() {
    let (mut s, id) = setup();
    assert_eq!(
        s.apply(&batch(
            &s,
            vec![move_op(id, (0, 0), Rgba::TRANSPARENT, None)]
        ))
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
    apply(&mut s, vec![selected(true).0]);
    let before = s.snapshot();
    let selection = s.selection();
    let ticket = s.ticket();
    let admission = Admission::with_residency(0, SharedBudget::new(0))
        .with_paint_metadata_limit(0)
        .with_paint_metadata_budget(SharedBudget::new(0))
        .with_selection_budget(SharedBudget::new(0));
    let prepared = s
        .preparation()
        .unwrap()
        .apply_with_admission(
            &batch(
                &s,
                vec![move_op(
                    id,
                    (0, 0),
                    Rgba([55; 4]),
                    Some(Rgba([11, 22, 33, 255])),
                )],
            ),
            admission,
            || Ok(()),
        )
        .unwrap();
    drop(prepared);
    let receipt = s
        .apply(&batch(
            &s,
            vec![move_op(
                id,
                (0, 0),
                Rgba([55; 4]),
                Some(Rgba([11, 22, 33, 255])),
            )],
        ))
        .unwrap();
    assert!(!receipt.committed);
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.selection(), selection);
    assert!(Arc::ptr_eq(&before, &s.snapshot()));
    apply(
        &mut s,
        vec![Op::SelectionSetRect {
            rect: Rect::new(90, 90, 1, 1).unwrap(),
        }],
    );
    let ticket = s.ticket();
    assert!(
        !s.apply(&batch(
            &s,
            vec![move_op(id, (1, 2), Rgba::TRANSPARENT, None)]
        ))
        .unwrap()
        .committed
    );
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.selection(), Selection::Empty);
    assert_eq!(
        s.apply(&batch(
            &s,
            vec![move_op(Uuid::new_v4(), (0, 0), Rgba::TRANSPARENT, None)]
        ))
        .unwrap_err()
        .code,
        ErrorCode::ReferenceUnknownId
    );
}

#[test]
fn compound_logical_order_history_and_failure_after_private_prefix_remain_atomic() {
    let (mut s, id) = setup();
    apply(&mut s, vec![selected(true).0]);
    let before = s.snapshot();
    let selection = s.selection();
    let ticket = s.ticket();
    let first = Op::FillRect {
        layer: LayerRef::Id(id),
        rect: Rect::new(2, 1, 1, 1).unwrap(),
        color: Rgba([200, 30, 40, 0]),
        mode: Mode::Replace,
    };
    let moves = vec![
        first.clone(),
        move_op(id, (1, 0), Rgba::TRANSPARENT, None),
        move_op(id, (-1, 0), Rgba([255; 4]), Some(Rgba([0, 0, 0, 73]))),
    ];
    let mut sequential = Session::new((*before).clone());
    apply(&mut sequential, vec![selected(true).0]);
    for op in &moves {
        apply(&mut sequential, vec![op.clone()]);
    }
    apply(&mut s, moves.clone());
    assert!(s.document().same_semantics(sequential.document()));
    assert_eq!(field(&s.selection()), field(&sequential.selection()));
    s.undo().unwrap();
    assert!(s.document().same_semantics(&before));
    assert_eq!(s.selection(), selection);
    // A late extent-policy failure must not publish its earlier logical paint.
    let bad = vec![first, move_op(id, (1_048_576, 0), Rgba::TRANSPARENT, None)];
    let checkpoint = s.ticket();
    let cursor = s.history_view();
    assert_eq!(
        s.apply(&batch(&s, bad)).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(s.ticket(), checkpoint);
    assert_eq!(s.history_view(), cursor);
    assert!(s.document().same_semantics(&before));
    assert_ne!(
        ticket.publication_epoch,
        s.ticket().publication_epoch,
        "undo uses recorded publication identity"
    );
}

#[test]
fn cancellation_and_resource_denials_release_all_private_movement_accounts() {
    let (mut s, id) = setup();
    apply(&mut s, vec![selected(true).0]);
    let before = s.snapshot();
    let ticket = s.ticket();
    let selection = s.selection();
    let ops = vec![
        move_op(id, (2, 1), Rgba::TRANSPARENT, Some(Rgba([11, 22, 33, 255]))),
        move_op(id, (-1, -1), Rgba([255; 4]), None),
    ];
    let b = batch(&s, ops);
    let metadata = SharedBudget::new(8 << 20);
    let decoded = SharedBudget::new(8 << 20);
    let masks = SharedBudget::new(8 << 20);
    let admission = Admission::with_residency(1_000_000, decoded.clone())
        .with_selection_budget(masks.clone())
        .with_paint_metadata_budget(metadata.clone());
    let calls = Cell::new(0);
    drop(
        s.preparation()
            .unwrap()
            .apply_with_admission(&b, admission.clone(), || {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .unwrap(),
    );
    for failure in 1..=calls.get() {
        let count = Cell::new(0);
        let result = s
            .preparation()
            .unwrap()
            .apply_with_admission(&b, admission.clone(), || {
                count.set(count.get() + 1);
                if count.get() == failure {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel move"))
                } else {
                    Ok(())
                }
            });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(
            (
                metadata.usage().reserved,
                decoded.usage().reserved,
                masks.usage().reserved
            ),
            (0, 0, 0)
        );
        assert_eq!(s.ticket(), ticket);
        assert_eq!(s.selection(), selection);
        assert!(Arc::ptr_eq(&before, &s.snapshot()));
    }
    let denied = Admission::with_residency(1_000_000, SharedBudget::new(0));
    assert_eq!(
        s.preparation()
            .unwrap()
            .apply_with_admission(&b, denied, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let denied = Admission::new(1_000_000).with_paint_metadata_budget(SharedBudget::new(0));
    assert_eq!(
        s.preparation()
            .unwrap()
            .apply_with_admission(&b, denied, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.selection(), selection);
}
