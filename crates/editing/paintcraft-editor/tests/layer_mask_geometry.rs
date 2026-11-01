//! Independent coordinate literals, same admitted compiler/replay path.
use paintcraft_commands::{Anchor, Batch, FilterOp, FilterRef, LayerRef, MaskOp, Op};
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba, filters::Descriptor, orthogonal::OrthogonalTransform as T,
    resources::SharedBudget,
};
use paintcraft_document::{Document, FilterInstance, FilterMask, FilterStack, LayerMask, POLICY};
use paintcraft_editor::{execute, session::Session};
use paintcraft_raster::{
    Admission, Selection,
    coverage_mask::{A8Mask, Band, Run},
};
use std::cell::Cell;
use uuid::Uuid;
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn field(budget: &SharedBudget) -> A8Mask {
    A8Mask::from_bands(
        rect(-1, -1, 5, 4),
        &[
            Band {
                y0: 0,
                y1: 1,
                runs: &[
                    Run {
                        x0: 0,
                        x1: 1,
                        coverage: 127,
                    },
                    Run {
                        x0: 1,
                        x1: 3,
                        coverage: 255,
                    },
                ],
            },
            Band {
                y0: 1,
                y1: 2,
                runs: &[Run {
                    x0: 1,
                    x1: 2,
                    coverage: 64,
                }],
            },
        ],
        budget,
        Default::default(),
        &|| Ok(()),
    )
    .unwrap()
}
fn scene(enabled: bool, both: bool) -> Document {
    let mut d = paintcraft_document::new_document(8, 6, None, 0, POLICY).unwrap();
    let imported =
        paintcraft_document::import_image(3, 2, &[13, 71, 201, 0].repeat(6), &"34".repeat(32))
            .unwrap();
    d.roots = imported.roots;
    d.layers = imported.layers;
    let id = d.roots[0];
    let l = d.layers.get_mut(&id).unwrap();
    let coverage = field(&SharedBudget::new(8192));
    l.mask = Some(LayerMask::new(enabled, coverage.clone()));
    if both {
        l.filters = FilterStack::new(vec![
            FilterInstance::new(Uuid::from_u128(7), true, Descriptor::InvertRgbV1)
                .with_mask(Some(FilterMask::new(false, coverage))),
        ])
        .unwrap();
    }
    d
}
fn batch(d: &Document, ops: Vec<Op>) -> Batch {
    Batch {
        version: 3,
        policy: POLICY.into(),
        base_state: d.state_id,
        base_epoch: 0,
        seed: None,
        operations: ops,
        digest: "4".repeat(64),
    }
}
fn staged(d: &Document, ops: Vec<Op>) -> execute::Staged {
    execute::execute_with_admission_checked(
        d,
        Selection::Inactive,
        &batch(d, ops),
        Admission::new(1 << 20).with_coverage_budget(SharedBudget::new(64 << 20)),
        &|| Ok(()),
    )
    .unwrap()
}
fn point(t: T, x: i64, y: i64) -> (i64, i64) {
    match t {
        T::RotateRight => (1 - y, x),
        T::RotateLeft => (y, 2 - x),
        T::Rotate180 => (2 - x, 1 - y),
        T::FlipHorizontal => (2 - x, y),
        T::FlipVertical => (x, 1 - y),
    }
}
#[test]
fn filterless_and_mixed_masks_crop_and_empty_replace_with_correct_enabled_and_domain() {
    for enabled in [false, true] {
        for both in [false, true] {
            let d = scene(enabled, both);
            let id = d.roots[0];
            let original = d.layers[&id].mask.as_ref().unwrap().clone();
            let out = staged(
                &d,
                vec![Op::CanvasCrop {
                    rect: rect(1, 0, 2, 2),
                }],
            );
            let l = &out.doc.layers[&id];
            let m = l.mask.as_ref().unwrap();
            assert_eq!(m.enabled(), enabled);
            assert_eq!(m.coverage().domain(), rect(0, 0, 2, 2));
            for y in 0..2 {
                for x in 0..2 {
                    assert_eq!(
                        m.coverage().coverage(x, y),
                        original.coverage().coverage(x + 1, y)
                    );
                }
            }
            assert_eq!(m.coverage().coverage(-1, 0), 0);
            if both {
                assert_eq!(
                    l.filters.as_slice()[0].mask().unwrap().coverage(),
                    m.coverage()
                );
            }
            let empty = staged(
                &d,
                vec![Op::CanvasCrop {
                    rect: rect(6, 4, 1, 1),
                }],
            );
            let mask = empty.doc.layers[&id].mask.as_ref().unwrap();
            assert_eq!(mask.enabled(), enabled);
            assert_eq!(mask.coverage().domain(), rect(0, 0, 1, 1));
            assert_eq!(mask.coverage().coverage(0, 0), 0);
        }
    }
}
#[test]
fn whole_source_orthogonal_mapping_clips_latent_field_and_replay_reuses_both_families() {
    for t in [
        T::RotateRight,
        T::RotateLeft,
        T::Rotate180,
        T::FlipHorizontal,
        T::FlipVertical,
    ] {
        let d = scene(false, true);
        let id = d.roots[0];
        let old = d.layers[&id].mask.as_ref().unwrap();
        let out = staged(&d, vec![Op::CanvasTransform { transform: t }]);
        let l = &out.doc.layers[&id];
        let m = l.mask.as_ref().unwrap();
        assert!(!m.enabled());
        let (w, h) = if matches!(t, T::RotateRight | T::RotateLeft) {
            (2, 3)
        } else {
            (3, 2)
        };
        assert_eq!(m.coverage().domain(), rect(0, 0, w, h));
        assert_eq!(m.coverage().coverage(-1, 0), 0);
        for y in 0..2 {
            for x in 0..3 {
                let (a, b) = point(t, x, y);
                assert_eq!(m.coverage().coverage(a, b), old.coverage().coverage(x, y));
            }
        }
        assert_eq!(
            l.filters.as_slice()[0].mask().unwrap().coverage(),
            m.coverage()
        );
        assert_eq!(d.layers[&id].mask.as_ref().unwrap(), old);
    }
}
fn move_op(id: Uuid, x: i64) -> Op {
    Op::MoveSelection {
        layer: LayerRef::Id(id),
        offset: (x, 0),
        clear: Rgba::TRANSPARENT,
        transparent_key: None,
    }
}
#[test]
fn selected_move_rebases_latent_domain_but_selection_rotation_does_not_rotate_layer_field() {
    let d = scene(true, true);
    let id = d.roots[0];
    let old = d.layers[&id].mask.as_ref().unwrap();
    let out = staged(
        &d,
        vec![
            Op::SelectionSetRect {
                rect: rect(0, 0, 1, 1),
            },
            move_op(id, -2),
        ],
    );
    let l = &out.doc.layers[&id];
    let m = l.mask.as_ref().unwrap();
    assert_eq!(l.origin, (-2, 0));
    assert_eq!(m.coverage().domain(), rect(1, -1, 5, 4));
    assert_eq!(m.coverage().storage_usage(), old.coverage().storage_usage());
    for y in -1..3 {
        for x in -1..4 {
            assert_eq!(
                m.coverage().coverage(x + 2, y),
                old.coverage().coverage(x, y)
            );
        }
    }
    let out = staged(
        &d,
        vec![
            Op::SelectionSetRect {
                rect: rect(0, 0, 2, 1),
            },
            Op::TransformSelection {
                layer: LayerRef::Id(id),
                transform: T::RotateRight,
                clear: Rgba::TRANSPARENT,
            },
        ],
    );
    assert_eq!(
        out.doc.layers[&id].mask, d.layers[&id].mask,
        "No origin change means fixed mask, not selected-pixel rotation"
    );
}
#[test]
fn filter_only_mask_edits_keep_layer_slot_and_safe_canvas_resize_growth_need_no_coverage_bytes() {
    let d = scene(false, true);
    let id = d.roots[0];
    let old = d.layers[&id].mask.as_ref().unwrap();
    let out = staged(
        &d,
        vec![Op::Filter(FilterOp::Mask {
            layer: LayerRef::Id(id),
            filter: FilterRef::Id(Uuid::from_u128(7)),
            operation: MaskOp::Remove,
        })],
    );
    assert_eq!(out.doc.layers[&id].mask.as_ref(), Some(old));
    assert!(out.doc.layers[&id].filters.as_slice()[0].mask().is_none());
    let out = execute::execute_with_admission_checked(
        &d,
        Selection::Inactive,
        &batch(
            &d,
            vec![
                Op::CanvasResize {
                    width: 9,
                    height: 7,
                    anchor: Anchor::TopLeft,
                },
                Op::LayerExpand {
                    layer: LayerRef::Id(id),
                    width: 5,
                    height: 3,
                },
            ],
        ),
        Admission::new(1 << 20).with_coverage_budget(SharedBudget::new(0)),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(out.doc.layers[&id].mask.as_ref(), Some(old));
    assert_eq!(
        out.doc.layers[&id]
            .mask
            .as_ref()
            .unwrap()
            .coverage()
            .coverage(4, 2),
        0
    );
}
#[test]
fn geometric_stage_cancel_and_admission_denial_leave_source_and_all_new_charges_untouched() {
    let d = scene(true, true);
    let id = d.roots[0];
    let old = d.layers[&id].mask.as_ref().unwrap().digest();
    let op = vec![Op::CanvasTransform {
        transform: T::RotateRight,
    }];
    let calls = Cell::new(0);
    let output = execute::execute_with_admission_checked(
        &d,
        Selection::Inactive,
        &batch(&d, op.clone()),
        Admission::new(1 << 20).with_coverage_budget(SharedBudget::new(1 << 20)),
        &|| {
            calls.set(calls.get() + 1);
            Ok(())
        },
    )
    .unwrap();
    drop(output);
    for stop in [
        1,
        calls.get() / 3,
        calls.get() / 2,
        calls.get() - 1,
        calls.get(),
    ] {
        let budget = SharedBudget::new(1 << 20);
        let seen = Cell::new(0);
        let err = execute::execute_with_admission_checked(
            &d,
            Selection::Inactive,
            &batch(&d, op.clone()),
            Admission::new(1 << 20).with_coverage_budget(budget.clone()),
            &|| {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel mapped mask",
                    ))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::OperationCancelled);
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(d.layers[&id].mask.as_ref().unwrap().digest(), old);
    }
    let err = execute::execute_with_admission_checked(
        &d,
        Selection::Inactive,
        &batch(&d, op),
        Admission::new(1 << 20).with_coverage_budget(SharedBudget::new(0)),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourceLimit);
    assert_eq!(d.layers[&id].mask.as_ref().unwrap().digest(), old);
}
fn many(count: usize, budget: &SharedBudget) -> A8Mask {
    let runs = (0..count)
        .map(|i| Run {
            x0: 2 * i as i64,
            x1: 2 * i as i64 + 1,
            coverage: 127,
        })
        .collect::<Vec<_>>();
    A8Mask::from_bands(
        rect(0, 0, 2 * count as i64, 1),
        &[Band {
            y0: 0,
            y1: 1,
            runs: &runs,
        }],
        budget,
        Default::default(),
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn per_result_counts_allow_successive_large_rebases_and_deleted_layers_free_inventory() {
    let budget = SharedBudget::new(64 << 20);
    let mut d = scene(false, false);
    let id = d.roots[0];
    d.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(false, many(131073, &budget)));
    d.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(7),
        true,
        Descriptor::InvertRgbV1,
    )])
    .unwrap();
    let other = Uuid::from_u128(71);
    let mut second = d.layers[&id].clone();
    second.id = other;
    second.mask = Some(LayerMask::new(true, many(131071, &budget)));
    d.roots.push(other);
    d.layers.insert(other, second);
    let add_mask = Op::Filter(FilterOp::Mask {
        layer: LayerRef::Id(id),
        filter: FilterRef::Id(Uuid::from_u128(7)),
        operation: MaskOp::FromSelection { enabled: false },
    });
    // Initial logical inventory is exactly262144 runs. Adding one without
    // deletion must fail, although the new field is physically tiny.
    let refused = execute::execute_with_admission_checked(
        &d,
        Selection::Inactive,
        &batch(
            &d,
            vec![
                Op::SelectionSetRect {
                    rect: rect(0, 0, 1, 1),
                },
                add_mask.clone(),
            ],
        ),
        Admission::new(1 << 20).with_coverage_budget(SharedBudget::new(64 << 20)),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(refused.code, ErrorCode::ResourceLimit);
    let out = staged(
        &d,
        vec![
            Op::LayerDelete {
                layer: LayerRef::Id(other),
            },
            Op::SelectionSetRect {
                rect: rect(0, 0, 1, 1),
            },
            add_mask,
            move_op(id, -1),
            Op::SelectionSetRect {
                rect: rect(0, 0, 1, 1),
            },
            move_op(id, -2),
        ],
    );
    assert_eq!(out.doc.layers.len(), 1);
    assert_eq!(out.doc.layers[&id].origin, (-2, 0));
    let result = out.doc.layers[&id].mask.as_ref().unwrap().coverage();
    assert_eq!(result.stats().runs, 131073);
    assert_eq!(result.domain(), rect(2, 0, 262146, 1));
    assert_eq!(
        result.storage_usage(),
        d.layers[&id]
            .mask
            .as_ref()
            .unwrap()
            .coverage()
            .storage_usage()
    );
}
#[test]
fn both_families_count_per_use_at_exact_run_cap_and_not_by_backing_alias() {
    let budget = SharedBudget::new(64 << 20);
    for (count, succeed) in [(131072, true), (131073, false)] {
        let mut d = scene(false, false);
        let id = d.roots[0];
        let coverage = many(count, &budget);
        d.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(false, coverage.clone()));
        d.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![
            FilterInstance::new(Uuid::from_u128(7), true, Descriptor::InvertRgbV1)
                .with_mask(Some(FilterMask::new(false, coverage))),
        ])
        .unwrap();
        let ops = vec![
            Op::SelectionSetRect {
                rect: rect(0, 0, 1, 1),
            },
            move_op(id, -1),
        ];
        let result = execute::execute_with_admission_checked(
            &d,
            Selection::Inactive,
            &batch(&d, ops),
            Admission::new(1 << 20).with_coverage_budget(SharedBudget::new(64 << 20)),
            &|| Ok(()),
        );
        if succeed {
            assert_eq!(
                result.unwrap().doc.layers[&id]
                    .mask
                    .as_ref()
                    .unwrap()
                    .coverage()
                    .stats()
                    .runs,
                count
            );
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
        }
    }
}
#[test]
fn actual_session_crop_undo_redo_restores_layer_mask_and_raw_source_identity() {
    let d = scene(false, true);
    let id = d.roots[0];
    let mut s = Session::new(d);
    let before = s.snapshot();
    let mut b = batch(
        s.document(),
        vec![Op::CanvasCrop {
            rect: rect(1, 0, 2, 2),
        }],
    );
    b.base_epoch = s.ticket().publication_epoch;
    let olddepth = s.undo_depth();
    s.apply(&b).unwrap();
    assert_eq!(s.undo_depth(), olddepth + 1);
    let after = s.snapshot();
    assert!(s.undo().unwrap().is_some());
    assert!(s.document().same_semantics(&before));
    assert_eq!(s.document().layers[&id].mask, before.layers[&id].mask);
    assert!(s.redo().unwrap().is_some());
    assert!(s.document().same_semantics(&after));
}
