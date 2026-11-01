//! Actual compiler/replay and Session publication; byte expectations are literal.
use paintcraft_commands::{Batch, LayerMaskOp, LayerRef, Op};
use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba, resources::SharedBudget};
use paintcraft_document::{Document, LayerMask, POLICY};
use paintcraft_editor::{execute, session::Session};
use paintcraft_raster::{
    Admission, Selection,
    coverage_mask::{A8Mask, Band, Run},
};
use std::{cell::Cell, sync::Arc};
use uuid::Uuid;
fn r(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn document() -> Document {
    paintcraft_document::new_document(4, 3, Some(Rgba([13, 71, 201, 0])), 0, POLICY).unwrap()
}
fn b(d: &Document, ops: Vec<Op>) -> Batch {
    Batch {
        version: 7,
        policy: POLICY.into(),
        base_state: d.state_id,
        base_epoch: 0,
        seed: None,
        operations: ops,
        digest: "4".repeat(64),
    }
}
fn apply(s: &mut Session, ops: Vec<Op>) -> paintcraft_editor::session::Receipt {
    let mut batch = b(s.document(), ops);
    batch.base_epoch = s.ticket().publication_epoch;
    s.apply(&batch).unwrap()
}
fn capture(id: Uuid, enabled: bool) -> Op {
    Op::LayerMask(LayerMaskOp::FromSelection {
        layer: LayerRef::Id(id),
        enabled,
    })
}
fn invert(id: Uuid) -> Op {
    Op::LayerMask(LayerMaskOp::Invert {
        layer: LayerRef::Id(id),
    })
}
fn remove(id: Uuid) -> Op {
    Op::LayerMask(LayerMaskOp::Remove {
        layer: LayerRef::Id(id),
    })
}
fn toggle(id: Uuid, enabled: bool) -> Op {
    Op::LayerMask(LayerMaskOp::SetEnabled {
        layer: LayerRef::Id(id),
        enabled,
    })
}
fn field() -> A8Mask {
    A8Mask::from_bands(
        r(-1, -1, 4, 3),
        &[Band {
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
                    x1: 2,
                    coverage: 255,
                },
            ],
        }],
        &SharedBudget::new(8192),
        Default::default(),
        &|| Ok(()),
    )
    .unwrap()
}
fn with_field() -> Document {
    let mut d = document();
    let id = d.roots[0];
    d.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(false, field()));
    d
}
fn run(d: &Document, selection: Selection, ops: Vec<Op>, budget: &SharedBudget) -> execute::Staged {
    execute::execute_with_admission_checked(
        d,
        selection,
        &b(d, ops),
        Admission::new(1 << 20).with_coverage_budget(budget.clone()),
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn four_actions_freeze_selection_and_preserve_source_with_one_undo_each() {
    let mut s = Session::new(document());
    let id = s.document().roots[0];
    let source = s.snapshot();
    let first = apply(
        &mut s,
        vec![
            Op::SelectionSetRect {
                rect: r(1, 0, 2, 1),
            },
            capture(id, false),
            Op::SelectionClear,
        ],
    );
    assert!(first.committed);
    assert_eq!(s.undo_depth(), 1);
    assert_eq!(s.selection(), Selection::Inactive);
    let saved = s.document().layers[&id].mask.as_ref().unwrap().clone();
    assert!(!saved.enabled());
    assert_eq!(saved.coverage().domain(), r(0, 0, 4, 3));
    assert_eq!(
        [
            saved.coverage().coverage(0, 0),
            saved.coverage().coverage(1, 0),
            saved.coverage().coverage(2, 0),
            saved.coverage().coverage(3, 0)
        ],
        [0, 255, 255, 0]
    );
    let captured = s.snapshot();
    let before_depth = s.undo_depth();
    assert!(apply(&mut s, vec![toggle(id, true)]).committed);
    assert_eq!(s.undo_depth(), before_depth + 1);
    assert_eq!(
        s.document().layers[&id]
            .mask
            .as_ref()
            .unwrap()
            .coverage()
            .storage_usage(),
        saved.coverage().storage_usage()
    );
    assert!(apply(&mut s, vec![invert(id)]).committed);
    let mask = s.document().layers[&id].mask.as_ref().unwrap();
    assert_eq!(
        [
            mask.coverage().coverage(0, 0),
            mask.coverage().coverage(1, 0),
            mask.coverage().coverage(2, 0),
            mask.coverage().coverage(3, 0)
        ],
        [255, 0, 0, 255]
    );
    let before_remove = s.snapshot();
    assert!(apply(&mut s, vec![remove(id)]).committed);
    assert!(s.document().layers[&id].mask.is_none());
    assert_eq!(s.document().layers[&id].raster, source.layers[&id].raster);
    s.undo().unwrap().unwrap();
    assert!(s.document().same_semantics(&before_remove));
    s.redo().unwrap().unwrap();
    assert!(s.document().layers[&id].mask.is_none());
    assert!(
        captured.layers[&id]
            .mask
            .as_ref()
            .unwrap()
            .coverage()
            .same_coverage_checked(saved.coverage(), 1 << 20, &|| Ok(()))
            .unwrap()
    );
}
#[test]
fn inactive_refuses_but_active_empty_disjoint_and_offset_selections_stay_real_masks() {
    let mut d = document();
    let id = d.roots[0];
    d.layers.get_mut(&id).unwrap().origin = (-1, 1);
    let budget = SharedBudget::new(1 << 20);
    let error = execute::execute_with_admission_checked(
        &d,
        Selection::Inactive,
        &b(&d, vec![capture(id, true)]),
        Admission::new(1 << 20).with_coverage_budget(budget.clone()),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InputSchema);
    assert_eq!(budget.usage().reserved, 0);
    for selection in [Selection::Empty, Selection::Rect(r(0, 0, 1, 1))] {
        let out = run(&d, selection, vec![capture(id, true)], &budget);
        let m = out.doc.layers[&id].mask.as_ref().unwrap();
        assert!(m.enabled());
        assert_eq!(m.coverage().domain(), r(0, 0, 4, 3));
        assert_eq!(m.coverage().stats().runs, 0);
    }
    let out = run(
        &d,
        Selection::Rect(r(0, 1, 2, 1)),
        vec![capture(id, true)],
        &budget,
    );
    let m = out.doc.layers[&id].mask.as_ref().unwrap();
    assert_eq!(
        [
            m.coverage().coverage(0, 0),
            m.coverage().coverage(1, 0),
            m.coverage().coverage(2, 0)
        ],
        [0, 255, 255]
    );
}
#[test]
fn absent_and_equal_noops_need_no_new_coverage_and_bad_target_errors_are_typed() {
    for present in [false, true] {
        let d = if present { with_field() } else { document() };
        let id = d.roots[0];
        let op = if present {
            toggle(id, false)
        } else {
            remove(id)
        };
        let out = run(&d, Selection::Inactive, vec![op], &SharedBudget::new(0));
        assert!(out.doc.same_semantics(&d));
    }
    let d = document();
    let id = d.roots[0];
    for op in [toggle(id, true), invert(id)] {
        let error = execute::execute_with_admission_checked(
            &d,
            Selection::Inactive,
            &b(&d, vec![op]),
            Admission::new(1 << 20).with_coverage_budget(SharedBudget::new(0)),
            &|| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::TargetInvalid);
    }
    let mut s = Session::new(with_field());
    let id = s.document().roots[0];
    let before = s.snapshot();
    let ticket = s.ticket();
    assert!(!apply(&mut s, vec![toggle(id, false)]).committed);
    assert!(Arc::ptr_eq(&before, &s.snapshot()));
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.undo_depth(), 0);
}
#[test]
fn fractional_gaps_invert_twice_preserves_latent_domain_disabled_intent_and_semantics() {
    let d = with_field();
    let id = d.roots[0];
    let old = d.layers[&id].mask.as_ref().unwrap();
    let budget = SharedBudget::new(1 << 20);
    let out = run(&d, Selection::Inactive, vec![invert(id)], &budget);
    let m = out.doc.layers[&id].mask.as_ref().unwrap();
    assert!(!m.enabled());
    assert_eq!(m.coverage().domain(), old.coverage().domain());
    assert_eq!(m.coverage().coverage(-2, 0), 0);
    assert_eq!(
        [
            m.coverage().coverage(-1, 0),
            m.coverage().coverage(0, 0),
            m.coverage().coverage(1, 0),
            m.coverage().coverage(2, 0)
        ],
        [255, 128, 0, 255]
    );
    let mut s = Session::new(d);
    let before = s.snapshot();
    assert!(!apply(&mut s, vec![invert(id), invert(id)]).committed);
    assert!(Arc::ptr_eq(&before, &s.snapshot()));
    assert_eq!(s.undo_depth(), 0);
}
#[test]
fn replay_owns_admitted_field_until_output_drop_and_equal_replacement_reuses_backing() {
    let d = document();
    let id = d.roots[0];
    let budget = SharedBudget::new(1 << 20);
    let out = run(
        &d,
        Selection::Rect(r(1, 0, 2, 1)),
        vec![capture(id, true)],
        &budget,
    );
    let usage = budget.usage().reserved;
    assert!(usage > 0);
    let clone = out.doc.clone();
    drop(out);
    assert_eq!(budget.usage().reserved, usage);
    let old = clone.layers[&id].mask.as_ref().unwrap();
    let new = run(
        &clone,
        Selection::Rect(r(1, 0, 2, 1)),
        vec![capture(id, true)],
        &budget,
    );
    assert_eq!(
        new.doc.layers[&id]
            .mask
            .as_ref()
            .unwrap()
            .coverage()
            .storage_usage(),
        old.coverage().storage_usage()
    );
    drop(new);
    drop(clone);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn cancellation_in_compiler_and_replay_or_byte_denial_leaves_source_and_new_charges_intact() {
    let d = with_field();
    let id = d.roots[0];
    let budget = SharedBudget::new(1 << 20);
    let calls = Cell::new(0);
    let out = execute::execute_with_admission_checked(
        &d,
        Selection::Inactive,
        &b(&d, vec![invert(id)]),
        Admission::new(1 << 20).with_coverage_budget(budget.clone()),
        &|| {
            calls.set(calls.get() + 1);
            Ok(())
        },
    )
    .unwrap();
    drop(out);
    assert_eq!(budget.usage().reserved, 0);
    for stop in [
        1,
        calls.get() / 3,
        calls.get() / 2,
        calls.get() - 1,
        calls.get(),
    ] {
        let seen = Cell::new(0);
        let err = execute::execute_with_admission_checked(
            &d,
            Selection::Inactive,
            &b(&d, vec![invert(id)]),
            Admission::new(1 << 20).with_coverage_budget(budget.clone()),
            &|| {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel layer mask",
                    ))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::OperationCancelled);
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(d.layers[&id].mask.as_ref().unwrap().coverage(), &field());
    }
    let err = execute::execute_with_admission_checked(
        &d,
        Selection::Inactive,
        &b(&d, vec![invert(id)]),
        Admission::new(1 << 20).with_coverage_budget(SharedBudget::new(0)),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourceLimit);
}
fn refuse(
    _: &Document,
    check: &dyn Fn() -> paintcraft_core::types::Result<()>,
) -> paintcraft_core::types::Result<()> {
    check()?;
    Err(Error::new(
        ErrorCode::ResourceLimit,
        "injected persistent result ceiling",
    ))
}
#[test]
fn persistent_validator_and_stale_preparation_refusal_preserve_selection_history_and_source() {
    let mut s = Session::new(document());
    let id = s.document().roots[0];
    let before = s.snapshot();
    let ticket = s.ticket();
    let selection = s.selection();
    let history = s.history_view();
    s.set_persistent_validator(Some(refuse));
    let mut batch = b(
        s.document(),
        vec![
            Op::SelectionSetRect {
                rect: r(0, 0, 1, 1),
            },
            capture(id, true),
        ],
    );
    batch.base_epoch = s.ticket().publication_epoch;
    assert_eq!(s.apply(&batch).unwrap_err().code, ErrorCode::ResourceLimit);
    assert!(Arc::ptr_eq(&before, &s.snapshot()));
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.selection(), selection);
    assert_eq!(s.history_view(), history);
    s.set_persistent_validator(None);
    let prepared = s.preparation().unwrap().apply(&batch, || Ok(())).unwrap();
    apply(
        &mut s,
        vec![Op::SelectionSetRect {
            rect: r(2, 0, 1, 1),
        }],
    );
    let current = s.snapshot();
    assert_eq!(
        s.publish_prepared(prepared, || Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert!(Arc::ptr_eq(&current, &s.snapshot()));
    assert!(s.document().layers[&id].mask.is_none());
}
#[test]
fn per_document_counts_include_other_layers_and_deletion_frees_new_capture_inventory() {
    let budget = SharedBudget::new(64 << 20);
    let mut d = document();
    let id = d.roots[0];
    let runs = (0..262144)
        .map(|i| Run {
            x0: 2 * i,
            x1: 2 * i + 1,
            coverage: 127,
        })
        .collect::<Vec<_>>();
    let field = A8Mask::from_bands(
        r(0, 0, 524288, 1),
        &[Band {
            y0: 0,
            y1: 1,
            runs: &runs,
        }],
        &budget,
        Default::default(),
        &|| Ok(()),
    )
    .unwrap();
    let other = Uuid::from_u128(77);
    let mut layer = d.layers[&id].clone();
    layer.id = other;
    layer.mask = Some(LayerMask::new(false, field));
    d.roots.push(other);
    d.layers.insert(other, layer);
    let selected = Selection::Rect(r(0, 0, 1, 1));
    let action = vec![capture(id, true)];
    let err = execute::execute_with_admission_checked(
        &d,
        selected.clone(),
        &b(&d, action),
        Admission::new(1 << 20).with_coverage_budget(SharedBudget::new(64 << 20)),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourceLimit);
    assert!(d.layers[&id].mask.is_none());
    let out = run(
        &d,
        selected,
        vec![
            Op::LayerDelete {
                layer: LayerRef::Id(other),
            },
            capture(id, true),
            toggle(id, false),
            invert(id),
            invert(id),
        ],
        &SharedBudget::new(64 << 20),
    );
    assert_eq!(out.doc.layers.len(), 1);
    let mask = out.doc.layers[&id].mask.as_ref().unwrap();
    assert!(!mask.enabled());
    assert_eq!(mask.coverage().coverage(0, 0), 255);
    assert_eq!(mask.coverage().stats().runs, 1);
}
#[test]
fn arbitrary_binary_and_inverted_selections_capture_exact_source_local_values() {
    use paintcraft_core::types::text::MaskSpan;
    use paintcraft_raster::selection::BinaryMask;
    let d = document();
    let id = d.roots[0];
    let budget = SharedBudget::new(1 << 20);
    let selection = BinaryMask::from_spans(
        r(0, 0, 4, 3),
        false,
        &[
            MaskSpan {
                y: 0,
                x0: 0,
                x1: 0,
                coverage: 255,
            },
            MaskSpan {
                y: 1,
                x0: 2,
                x1: 3,
                coverage: 255,
            },
        ],
        &budget,
        Default::default(),
        &|| Ok(()),
    )
    .unwrap();
    for inverse in [false, true] {
        let selected = if inverse {
            selection.inverted()
        } else {
            selection.clone()
        };
        let out = run(
            &d,
            Selection::Mask(selected),
            vec![capture(id, true)],
            &budget,
        );
        let field = out.doc.layers[&id].mask.as_ref().unwrap().coverage();
        for y in 0..3 {
            for x in 0..4 {
                let on = (y == 0 && x == 0) || (y == 1 && x >= 2);
                assert_eq!(field.coverage(x, y), if on != inverse { 255 } else { 0 });
            }
        }
        assert_eq!(field.coverage(-1, 0), 0);
        assert_eq!(field.coverage(4, 0), 0);
    }
}
