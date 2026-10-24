use paintcraft_commands::Batch;
use paintcraft_core::types::{Error, ErrorCode, Rgba};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::session::Session;
use std::sync::Arc;

fn session() -> Session {
    Session::new(new_document(8, 8, Some(Rgba([255; 4])), 0, POLICY).unwrap())
}
fn edit(s: &Session, color: &str) -> Batch {
    let t = s.ticket();
    let id = s.document().roots[0];
    Batch::from_json(format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":0,"y":0,"width":8,"height":8}},"color":"{color}","mode":"replace"}}]}}"##,t.state_id,t.publication_epoch).as_bytes()).unwrap()
}
fn cancelled() -> paintcraft_core::types::Result<()> {
    Err(Error::new(
        ErrorCode::OperationCancelled,
        "injected cancellation",
    ))
}

#[test]
fn worker_preparation_does_not_publish_and_owner_publication_restores_exact_history() {
    let mut s = session();
    let initial = s.snapshot();
    let view = s.history_view();
    let batch = edit(&s, "#17589CCC");
    let work = s.preparation().unwrap();
    let prepared = std::thread::spawn(move || work.apply(&batch, || Ok(())).unwrap())
        .join()
        .unwrap();
    assert!(Arc::ptr_eq(&s.snapshot(), &initial));
    assert_eq!(s.history_view(), view);
    let receipt = s.publish_prepared(prepared, || Ok(())).unwrap();
    assert!(receipt.committed);
    assert_eq!(s.ticket().publication_epoch, 1);
    let edited = s.snapshot();
    assert_eq!(
        edited.layers[&edited.roots[0]].raster.get(3, 4),
        Rgba([23, 88, 156, 204])
    );
    s.undo().unwrap().unwrap();
    assert!(Arc::ptr_eq(&s.snapshot(), &initial));
    s.redo().unwrap().unwrap();
    assert!(Arc::ptr_eq(&s.snapshot(), &edited));
}

#[test]
fn foreign_session_cannot_publish_even_with_equal_content_and_ticket() {
    let source = session();
    let mut other = session();
    assert_eq!(source.ticket(), other.ticket());
    let prepared = source
        .preparation()
        .unwrap()
        .apply(&edit(&source, "#000000FF"), || Ok(()))
        .unwrap();
    let view = other.history_view();
    assert_eq!(
        other
            .publish_prepared(prepared, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(other.history_view(), view);
}

#[test]
fn edit_then_undo_back_to_same_state_still_invalidates_a_preparation() {
    let mut s = session();
    let prepared = s
        .preparation()
        .unwrap()
        .apply(&edit(&s, "#000000FF"), || Ok(()))
        .unwrap();
    let initial = s.ticket().state_id;
    s.apply(&edit(&s, "#FF0000FF")).unwrap();
    s.undo().unwrap().unwrap();
    assert_eq!(s.ticket().state_id, initial);
    let view = s.history_view();
    assert_eq!(
        s.publish_prepared(prepared, || Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(s.history_view(), view);
}

#[test]
fn resource_policy_changes_invalidate_already_prepared_history() {
    let mut s = session();
    let prepared = s
        .preparation()
        .unwrap()
        .apply(&edit(&s, "#000000FF"), || Ok(()))
        .unwrap();
    s.set_history_budget(0);
    let view = s.history_view();
    assert_eq!(
        s.publish_prepared(prepared, || Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(s.history_view(), view);
}

#[test]
fn cancellation_on_worker_or_at_publication_keeps_redo_and_document() {
    let mut s = session();
    s.apply(&edit(&s, "#000000FF")).unwrap();
    let edited = s.ticket().state_id;
    s.undo().unwrap().unwrap();
    let doc = s.snapshot();
    let view = s.history_view();
    assert_eq!(
        s.preparation()
            .unwrap()
            .apply(&edit(&s, "#FF0000FF"), cancelled)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    let prepared = s
        .preparation()
        .unwrap()
        .apply(&edit(&s, "#FF0000FF"), || Ok(()))
        .unwrap();
    assert_eq!(
        s.publish_prepared(prepared, cancelled).unwrap_err().code,
        ErrorCode::OperationCancelled
    );
    assert!(Arc::ptr_eq(&s.snapshot(), &doc));
    assert_eq!(s.history_view(), view);
    assert_eq!(s.redo().unwrap().unwrap().state_id, edited);
}

#[test]
fn saves_during_preparation_preserve_saved_state_acknowledgement() {
    let mut s = session();
    s.apply(&edit(&s, "#000000FF")).unwrap();
    let prepared = s
        .preparation()
        .unwrap()
        .apply(&edit(&s, "#FF0000FF"), || Ok(()))
        .unwrap();
    s.mark_saved();
    assert!(!s.is_dirty());
    s.publish_prepared(prepared, || Ok(())).unwrap();
    assert!(s.is_dirty());
    s.undo().unwrap().unwrap();
    assert!(!s.is_dirty());
}

#[test]
fn no_op_result_does_not_replace_history_or_advance_epoch() {
    let mut s = session();
    let doc = s.snapshot();
    let view = s.history_view();
    let prepared = s
        .preparation()
        .unwrap()
        .apply(&edit(&s, "#FFFFFFFF"), || Ok(()))
        .unwrap();
    assert!(!s.publish_prepared(prepared, || Ok(())).unwrap().committed);
    assert!(Arc::ptr_eq(&s.snapshot(), &doc));
    assert_eq!(s.history_view(), view);
}

#[path = "preparation/navigation.rs"]
mod navigation;

#[test]
fn explicit_empty_preparation_preserves_redo_and_later_save_acknowledgements() {
    let mut s = session();
    s.apply(&edit(&s, "#000000FF")).unwrap();
    s.undo().unwrap().unwrap();
    let original = s.snapshot();
    let view = s.history_view();
    let work = s.preparation().unwrap();
    s.mark_saved();
    let prepared = std::thread::spawn(move || work.no_change(|| Ok(())).unwrap())
        .join()
        .unwrap();
    let receipt = s.publish_prepared(prepared, || Ok(())).unwrap();
    assert!(!receipt.committed && receipt.history_id.is_none() && receipt.aliases.is_empty());
    assert!(Arc::ptr_eq(&s.snapshot(), &original));
    assert_eq!(s.history_view(), view);
    assert!(!s.is_dirty());
    assert_eq!(s.redo_depth(), 1);
    s.redo().unwrap().unwrap();
    assert!(s.is_dirty());
}

#[test]
fn explicit_empty_preparation_obeys_cancellation_and_stale_publication_guards() {
    let mut s = session();
    assert_eq!(
        s.preparation()
            .unwrap()
            .no_change(cancelled)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    let prepared = s.preparation().unwrap().no_change(|| Ok(())).unwrap();
    s.apply(&edit(&s, "#000000FF")).unwrap();
    let view = s.history_view();
    assert_eq!(
        s.publish_prepared(prepared, || Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(s.history_view(), view);
}

#[test]
fn exported_prepared_snapshot_has_final_identity_without_owner_publication() {
    let mut owner = session();
    let ticket = owner.ticket();
    let before = owner.history_view();
    let prepared = owner
        .preparation()
        .unwrap()
        .apply(&edit(&owner, "#123456FF"), || Ok(()))
        .unwrap();
    let snapshot = prepared.document_snapshot();
    assert_ne!(snapshot.state_id, ticket.state_id);
    assert_eq!(
        snapshot.layers[&snapshot.roots[0]].raster.get(1, 1),
        Rgba([18, 52, 86, 255])
    );
    assert_eq!(owner.ticket(), ticket);
    assert_eq!(owner.history_view(), before);
    assert!(!owner.is_dirty());
    owner.publish_prepared(prepared, || Ok(())).unwrap();
    assert!(Arc::ptr_eq(&snapshot, &owner.snapshot()));
    owner.undo().unwrap();
    assert_eq!(
        snapshot.layers[&snapshot.roots[0]].raster.get(1, 1),
        Rgba([18, 52, 86, 255])
    );
}
