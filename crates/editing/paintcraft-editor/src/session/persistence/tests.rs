use super::*;
use paintcraft_commands::{LayerRef, Mode, Op};
use paintcraft_core::types::{Rect, Rgba};
use std::cell::Cell;

thread_local! {
    static CALLS: Cell<usize> = const { Cell::new(0) };
    static IN_VALIDATOR: Cell<bool> = const { Cell::new(false) };
    static SEEN_STATE: Cell<Option<Uuid>> = const { Cell::new(None) };
}
fn session() -> Session {
    CALLS.set(0);
    SEEN_STATE.set(None);
    Session::new(paintcraft_document::new_document(8, 8, Some(Rgba([255; 4])), 0, POLICY).unwrap())
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
fn paint(s: &Session, color: Rgba) -> Op {
    Op::FillRect {
        layer: LayerRef::Id(s.document().roots[0]),
        rect: Rect {
            x0: 0,
            y0: 0,
            x1: 1,
            y1: 1,
        },
        color,
        mode: Mode::Replace,
    }
}
fn reject(_: &Document, _: &dyn Fn() -> Result<()>) -> Result<()> {
    CALLS.set(CALLS.get() + 1);
    Err(Error::new(
        ErrorCode::ResourceLimit,
        "test persistent capacity",
    ))
}
fn accept(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<()> {
    CALLS.set(CALLS.get() + 1);
    SEEN_STATE.set(Some(doc.state_id));
    check()
}
fn checkpoints(_: &Document, check: &dyn Fn() -> Result<()>) -> Result<()> {
    IN_VALIDATOR.set(true);
    let result = (|| {
        check()?;
        check()
    })();
    IN_VALIDATOR.set(false);
    result
}
fn assert_unchanged(s: &Session, before: &SessionView) {
    assert!(Arc::ptr_eq(&s.snapshot(), &before.doc));
    assert_eq!(s.ticket(), before.ticket);
    assert_eq!(s.selection(), before.selection);
    assert_eq!(s.history_view(), before.history);
    assert_eq!(s.is_dirty(), before.dirty);
    assert_eq!(s.saved_state, before.saved);
}
struct SessionView {
    doc: Arc<Document>,
    ticket: Ticket,
    selection: Selection,
    history: HistoryView,
    dirty: bool,
    saved: Option<Uuid>,
}
impl SessionView {
    fn capture(s: &Session) -> Self {
        Self {
            doc: s.snapshot(),
            ticket: s.ticket(),
            selection: s.selection(),
            history: s.history_view(),
            dirty: s.is_dirty(),
            saved: s.saved_state,
        }
    }
}

#[test]
fn synchronous_growth_refusal_preserves_document_selection_history_and_redo() {
    for kind in 0..3 {
        let mut s = session();
        s.apply(&batch(&s, vec![paint(&s, Rgba([7, 8, 9, 255]))]))
            .unwrap();
        s.undo().unwrap().unwrap();
        s.mark_unsaved();
        s.set_persistent_validator(Some(reject));
        let before = SessionView::capture(&s);
        let operation = match kind {
            0 => Op::LayerAdd {
                alias: "new".into(),
                name: "growth".into(),
                width: 8,
                height: 8,
                origin: (0, 0),
                default: Rgba::TRANSPARENT,
            },
            1 => Op::LayerSetProperties {
                layer: LayerRef::Id(s.document().roots[0]),
                name: Some("larger name".into()),
                visible: None,
                opacity: None,
            },
            _ => paint(&s, Rgba([255, 0, 0, 255])),
        };
        let error = s
            .apply(&batch(
                &s,
                vec![
                    operation,
                    Op::SelectionSetRect {
                        rect: Rect {
                            x0: 0,
                            y0: 0,
                            x1: 2,
                            y1: 2,
                        },
                    },
                ],
            ))
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_unchanged(&s, &before);
        assert_eq!(s.redo_depth(), 1);
    }
}

#[test]
fn accepted_content_is_checked_with_final_id_and_records_one_undo_step() {
    let mut s = session().with_persistent_validator(Some(accept));
    let initial = s.snapshot();
    let result = s
        .apply(&batch(&s, vec![paint(&s, Rgba([4, 5, 6, 7]))]))
        .unwrap();
    assert!(result.committed);
    assert_ne!(result.state_id, initial.state_id);
    assert_eq!(SEEN_STATE.get(), Some(result.state_id));
    assert_eq!(CALLS.get(), 1);
    assert_eq!(s.undo_depth(), 1);
    s.undo().unwrap().unwrap();
    assert!(Arc::ptr_eq(&s.snapshot(), &initial));
    s.redo().unwrap().unwrap();
    assert_eq!(s.document().state_id, result.state_id);
    assert_eq!(CALLS.get(), 1);
}

#[test]
fn legacy_unfit_noops_selection_and_history_stay_usable() {
    let mut s = session();
    s.apply(&batch(&s, vec![paint(&s, Rgba([4, 5, 6, 7]))]))
        .unwrap();
    let painted = s.snapshot();
    s.set_persistent_validator(Some(reject));
    let unchanged = batch(&s, vec![paint(&s, Rgba([4, 5, 6, 7]))]);
    assert!(!s.apply(&unchanged).unwrap().committed);
    s.mark_saved();
    let select = batch(
        &s,
        vec![Op::SelectionSetRect {
            rect: Rect {
                x0: 0,
                y0: 0,
                x1: 2,
                y1: 2,
            },
        }],
    );
    let prepared = s.preparation().unwrap().apply(&select, || Ok(())).unwrap();
    s.publish_prepared(prepared, || Ok(())).unwrap();
    assert!(!s.is_dirty());
    let prepared = s.preparation().unwrap().no_change(|| Ok(())).unwrap();
    assert!(!s.publish_prepared(prepared, || Ok(())).unwrap().committed);
    s.undo().unwrap().unwrap();
    s.undo().unwrap().unwrap();
    let navigation = s.preparation().unwrap().redo(|| Ok(())).unwrap();
    s.publish_navigation(navigation, || Ok(()))
        .unwrap()
        .unwrap();
    assert!(s.document().same_semantics(&painted));
    assert_eq!(CALLS.get(), 0);
}

#[test]
fn detached_export_derivation_succeeds_but_owner_rechecks_before_publication() {
    let mut s = session().with_persistent_validator(Some(reject));
    let before = SessionView::capture(&s);
    let edit = batch(&s, vec![paint(&s, Rgba([19, 29, 39, 49]))]);
    let work = s.preparation().unwrap();
    let prepared = std::thread::spawn(move || work.apply(&edit, || Ok(())).unwrap())
        .join()
        .unwrap();
    assert_eq!(
        prepared.document_snapshot().layers[&s.document().roots[0]]
            .raster
            .get(0, 0),
        Rgba([19, 29, 39, 49])
    );
    assert_eq!(CALLS.get(), 0);
    let error = s.publish_prepared(prepared, || Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(CALLS.get(), 1);
    assert_unchanged(&s, &before);
}

#[test]
fn policy_replacement_removal_and_foreign_results_refuse_before_validator() {
    for replacement in [Some(accept as PersistentValidator), None] {
        let mut s = session().with_persistent_validator(Some(accept));
        let prepared = s
            .preparation()
            .unwrap()
            .apply(&batch(&s, vec![paint(&s, Rgba([1; 4]))]), || Ok(()))
            .unwrap();
        s.set_persistent_validator(replacement);
        let before = SessionView::capture(&s);
        assert_eq!(
            s.publish_prepared(prepared, || Ok(())).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        assert_eq!(CALLS.get(), 0);
        assert_unchanged(&s, &before);
    }
    let source = session();
    let prepared = source
        .preparation()
        .unwrap()
        .apply(&batch(&source, vec![paint(&source, Rgba([1; 4]))]), || {
            Ok(())
        })
        .unwrap();
    let mut other = Session::new(source.document().clone()).with_persistent_validator(Some(reject));
    assert_eq!(
        other
            .publish_prepared(prepared, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(CALLS.get(), 0);
}

#[test]
fn prepared_acceptance_preserves_save_acknowledged_after_capture() {
    let mut s = session().with_persistent_validator(Some(accept));
    s.mark_unsaved();
    let initial = s.document().state_id;
    let prepared = s
        .preparation()
        .unwrap()
        .apply(&batch(&s, vec![paint(&s, Rgba([6; 4]))]), || Ok(()))
        .unwrap();
    s.mark_saved();
    s.publish_prepared(prepared, || Ok(())).unwrap();
    assert_eq!(s.saved_state, Some(initial));
    assert!(s.is_dirty());
    assert_eq!(CALLS.get(), 1);
    s.undo().unwrap().unwrap();
    assert!(!s.is_dirty());
}

#[test]
fn one_shot_and_sticky_validator_cancellation_publish_nothing_on_both_routes() {
    for detached in [false, true] {
        for sticky in [false, true] {
            for stop in 1..=2 {
                let mut s = session().with_persistent_validator(Some(checkpoints));
                let before = SessionView::capture(&s);
                let visits = Cell::new(0);
                let check = || {
                    if IN_VALIDATOR.get() {
                        visits.set(visits.get() + 1);
                        if visits.get() == stop || sticky && visits.get() > stop {
                            return Err(Error::new(
                                ErrorCode::OperationCancelled,
                                "validator sentinel",
                            ));
                        }
                    }
                    Ok(())
                };
                let edit = batch(&s, vec![paint(&s, Rgba([1, 2, 3, 4]))]);
                let error = if detached {
                    let prepared = s.preparation().unwrap().apply(&edit, || Ok(())).unwrap();
                    s.publish_prepared(prepared, check).unwrap_err()
                } else {
                    s.apply_checked(&edit, check).unwrap_err()
                };
                assert_eq!(error.code, ErrorCode::OperationCancelled);
                assert_eq!(visits.get(), stop);
                assert_unchanged(&s, &before);
            }
        }
    }
}

#[test]
fn swallowed_one_shot_cancellation_is_latched_on_both_publication_routes() {
    fn swallowing(_: &Document, check: &dyn Fn() -> Result<()>) -> Result<()> {
        IN_VALIDATOR.set(true);
        assert_eq!(check().unwrap_err().code, ErrorCode::OperationCancelled);
        // Subsequent calls see the first failure, not an apparently recovered caller.
        assert_eq!(check().unwrap_err().code, ErrorCode::OperationCancelled);
        IN_VALIDATOR.set(false);
        Ok(())
    }
    for detached in [false, true] {
        let mut s = session().with_persistent_validator(Some(swallowing));
        let before = SessionView::capture(&s);
        let edit = batch(&s, vec![paint(&s, Rgba([9; 4]))]);
        let visits = Cell::new(0);
        let check = || {
            if IN_VALIDATOR.get() {
                visits.set(visits.get() + 1);
                if visits.get() == 1 {
                    return Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "first caller failure",
                    ));
                }
            }
            Ok(())
        };
        let error = if detached {
            let prepared = s.preparation().unwrap().apply(&edit, || Ok(())).unwrap();
            s.publish_prepared(prepared, check).unwrap_err()
        } else {
            s.apply_checked(&edit, check).unwrap_err()
        };
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.message, "first caller failure");
        assert_eq!(visits.get(), 1);
        assert_unchanged(&s, &before);
    }
}

#[test]
fn installing_or_removing_validator_revokes_prepared_navigation() {
    for replacement in [Some(reject as PersistentValidator), None] {
        let mut s = session();
        s.apply(&batch(&s, vec![paint(&s, Rgba([8; 4]))])).unwrap();
        s.set_persistent_validator(Some(accept));
        let navigation = s.preparation().unwrap().undo(|| Ok(())).unwrap();
        s.set_persistent_validator(replacement);
        let before = SessionView::capture(&s);
        assert_eq!(
            s.publish_navigation(navigation, || Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
        assert_eq!(CALLS.get(), 0);
        assert_unchanged(&s, &before);
    }
}
