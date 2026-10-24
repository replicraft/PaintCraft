use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba, resources::SharedBudget};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::{
    execute::{self, Selection},
    session::{HistoryRetention, Session},
};
use paintcraft_raster::Admission;
use std::{cell::Cell, sync::Arc};
fn session() -> Session {
    Session::new(new_document(8, 6, Some(Rgba([255; 4])), 0, POLICY).unwrap())
}
fn polygon() -> Op {
    Op::SelectionSetPolygon {
        points: vec![(1, 1), (6, 1), (1, 5)],
    }
}
fn batch(s: &Session, operations: Vec<Op>) -> Batch {
    let ticket = s.ticket();
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: ticket.state_id,
        base_epoch: ticket.publication_epoch,
        seed: None,
        digest: format!("{:064x}", ticket.publication_epoch + 1),
        operations,
    }
}
fn apply(s: &mut Session, ops: Vec<Op>) {
    s.apply(&batch(s, ops)).unwrap();
}
fn mask(selection: Selection) -> paintcraft_raster::selection::BinaryMask {
    let Selection::Mask(mask) = selection else {
        panic!("expected mask")
    };
    mask
}
fn cancel() -> Result<()> {
    Err(Error::new(
        ErrorCode::OperationCancelled,
        "test cancellation",
    ))
}
#[test]
fn selection_only_history_keeps_content_clean_and_rejects_stale_preparations() {
    let mut s = session();
    let original = s.snapshot();
    let state = s.ticket().state_id;
    let initial = s.history_ticket();
    let work = s.preparation().unwrap();
    let op = batch(&s, vec![polygon()]);
    let old_result = work.apply(&op, || Ok(())).unwrap();
    apply(&mut s, vec![polygon()]);
    let a = mask(s.selection());
    for y in -1..8 {
        for x in -1..10 {
            // Independent triangle half-plane decision at pixel centers.
            let expected = x >= 1 && y >= 1 && 4 * (2 * x + 1 - 2) + 5 * (2 * y + 1 - 2) < 40;
            assert_eq!(a.coverage(x, y) == 255, expected, "{x},{y}");
        }
    }
    assert_eq!(s.ticket().state_id, state);
    assert!(!s.is_dirty());
    assert!(s.document().same_semantics(&original));
    let current = s.ticket();
    assert_eq!(
        s.publish_prepared(old_result, || Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(s.ticket(), current);
    let selected = s
        .history_view()
        .entries
        .iter()
        .find(|entry| entry.current)
        .unwrap()
        .history_id;
    apply(&mut s, vec![Op::SelectionInvert]);
    let b = mask(s.selection());
    assert!(a.shares_storage(&b));
    assert_ne!(a, b);
    for y in 0..6 {
        for x in 0..8 {
            assert_eq!(b.coverage(x, y), 255 - a.coverage(x, y));
        }
    }
    assert_eq!(b.coverage(-1, 0), 0);
    assert_eq!(b.coverage(8, 0), 0);
    s.undo().unwrap().unwrap();
    assert_eq!(s.selection(), Selection::Mask(a.clone()));
    s.redo().unwrap().unwrap();
    assert_eq!(s.selection(), Selection::Mask(b));
    s.navigate_history(s.history_ticket(), selected)
        .unwrap()
        .unwrap();
    assert_eq!(s.selection(), Selection::Mask(a));
    // A pre-selection view is stale even though the document StateId never changed.
    assert_eq!(
        s.navigate_history(initial, None).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert!(!s.is_dirty());
}
#[test]
fn construction_occurs_once_and_replay_keeps_the_exact_admitted_storage() {
    let mut s = session();
    let budget = SharedBudget::new(1 << 20);
    let prepared = s
        .preparation()
        .unwrap()
        .apply_with_admission(
            &batch(&s, vec![polygon()]),
            Admission::new(0).with_selection_budget(budget.clone()),
            || Ok(()),
        )
        .unwrap();
    let reserved = budget.usage().reserved;
    assert!(reserved > 65_536);
    assert!(
        budget.usage().peak < reserved * 2,
        "replay must not build a second mask; polygon scratch is temporary"
    );
    s.publish_prepared(prepared, || Ok(())).unwrap();
    assert_eq!(s.history_memory().selection_bytes, reserved);
    // Independent equal construction is a semantic no-op and releases its copy.
    let result = s
        .preparation()
        .unwrap()
        .apply_with_admission(
            &batch(&s, vec![polygon()]),
            Admission::new(0).with_selection_budget(budget.clone()),
            || Ok(()),
        )
        .unwrap();
    let ticket = s.ticket();
    let result = s.publish_prepared(result, || Ok(())).unwrap();
    assert!(!result.committed);
    assert_eq!(s.ticket(), ticket);
    assert_eq!(budget.usage().reserved, reserved);
    drop(s);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn failed_mixed_batches_and_every_cancellation_leave_published_state_untouched() {
    let s = session();
    let source = s.snapshot();
    let ticket = s.ticket();
    let history = s.history_view();
    let budget = SharedBudget::new(1 << 20);
    let calls = Cell::new(0);
    let b = batch(&s, vec![polygon(), Op::SelectionInvert]);
    execute::execute_with_admission_checked(
        &source,
        s.selection(),
        &b,
        Admission::new(0).with_selection_budget(budget.clone()),
        &|| {
            calls.set(calls.get() + 1);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(budget.usage().reserved, 0);
    for at in 1..=calls.get() {
        let count = Cell::new(0);
        let err = execute::execute_with_admission_checked(
            &source,
            s.selection(),
            &b,
            Admission::new(0).with_selection_budget(budget.clone()),
            &|| {
                count.set(count.get() + 1);
                if count.get() == at { cancel() } else { Ok(()) }
            },
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::OperationCancelled);
        assert_eq!(budget.usage().reserved, 0);
    }
    for paint in [
        Op::CanvasTransform {
            transform: paintcraft_core::types::orthogonal::OrthogonalTransform::RotateRight,
        },
        Op::CanvasCrop {
            rect: Rect::new(1, 1, 3, 3).unwrap(),
        },
        Op::TransformSelection {
            layer: LayerRef::Id(s.document().roots[0]),
            transform: paintcraft_core::types::orthogonal::OrthogonalTransform::FlipHorizontal,
            clear: Rgba([0; 4]),
        },
    ] {
        let err = s
            .preparation()
            .unwrap()
            .apply(&batch(&s, vec![polygon(), paint]), || Ok(()))
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InputSchema);
        assert_eq!(err.operation_index, Some(1));
    }
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.history_view(), history);
    assert_eq!(s.selection(), Selection::Inactive);
    assert!(Arc::ptr_eq(&source, &s.snapshot()));
    let deny = SharedBudget::new(0);
    assert!(
        s.preparation()
            .unwrap()
            .apply_with_admission(
                &b,
                Admission::new(0).with_selection_budget(deny.clone()),
                || Ok(())
            )
            .is_err()
    );
    assert_eq!(deny.usage().reserved, 0);
}
#[test]
fn clear_reenables_paint_and_resize_translates_clips_without_resurrecting_coverage() {
    let mut s = session();
    apply(&mut s, vec![polygon()]);
    let before = mask(s.selection());
    apply(
        &mut s,
        vec![Op::CanvasResize {
            width: 4,
            height: 3,
            anchor: paintcraft_commands::Anchor::TopLeft,
        }],
    );
    let clipped = mask(s.selection());
    assert!(before.shares_storage(&clipped));
    assert_eq!(clipped.domain(), Rect::new(0, 0, 4, 3).unwrap());
    apply(
        &mut s,
        vec![
            Op::CanvasResize {
                width: 8,
                height: 6,
                anchor: paintcraft_commands::Anchor::TopLeft,
            },
            Op::SelectionInvert,
        ],
    );
    let expanded = mask(s.selection());
    for y in 0..6 {
        for x in 0..8 {
            assert_eq!(expanded.coverage(x, y), 255 - clipped.coverage(x, y));
        }
    }
    let id = s.document().roots[0];
    apply(
        &mut s,
        vec![
            Op::SelectionClear,
            Op::FillRect {
                layer: LayerRef::Id(id),
                rect: Rect::new(0, 0, 8, 6).unwrap(),
                color: Rgba([0; 4]),
                mode: Mode::Replace,
            },
        ],
    );
    assert!(s.is_dirty());
    assert_eq!(s.selection(), Selection::Inactive);
}
#[test]
fn failed_history_record_and_cancelled_publication_release_private_masks() {
    let mut s = session();
    s.set_history_retention(HistoryRetention::Preserve);
    s.set_history_budget(1);
    let ticket = s.ticket();
    let budget = SharedBudget::new(1 << 20);
    assert!(
        s.preparation()
            .unwrap()
            .apply_with_admission(
                &batch(&s, vec![polygon()]),
                Admission::new(0).with_selection_budget(budget.clone()),
                || Ok(())
            )
            .is_err()
    );
    assert_eq!(s.ticket(), ticket);
    assert_eq!(budget.usage().reserved, 0);
    s.set_history_budget(1 << 20);
    let prepared = s
        .preparation()
        .unwrap()
        .apply_with_admission(
            &batch(&s, vec![polygon()]),
            Admission::new(0).with_selection_budget(budget.clone()),
            || Ok(()),
        )
        .unwrap();
    assert_eq!(
        s.publish_prepared(prepared, cancel).unwrap_err().code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(s.ticket(), ticket);
    assert_eq!(budget.usage().reserved, 0);
}
