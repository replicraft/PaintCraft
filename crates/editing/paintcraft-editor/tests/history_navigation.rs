//! Direct navigation restores recorded outcomes, including session-only selection.
use paintcraft_commands::Batch;
use paintcraft_core::types::{ErrorCode, Rgba};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::execute::Selection;
use paintcraft_editor::session::{Receipt, Session};
use uuid::Uuid;

fn session() -> Session {
    Session::new(new_document(8, 8, None, 0, POLICY).unwrap())
}
fn apply(s: &mut Session, operations: &str) -> Receipt {
    let t = s.ticket();
    let json = format!(
        r#"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":{operations}}}"#,
        t.state_id, t.publication_epoch
    );
    s.apply_json(json.as_bytes()).unwrap()
}
fn add(s: &mut Session) -> Uuid {
    apply(s,r##"[{"op":"layer.add","ref":"l","name":"L","width":8,"height":8,"origin":{"x":0,"y":0},"default_pixel":"#12345600"}]"##).aliases["l"]
}
fn paint(s: &mut Session, id: Uuid, x: i64) {
    apply(
        s,
        &format!(
            r##"[{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":{x},"y":0,"width":1,"height":1}},"color":"#FF000080","mode":"replace"}}]"##
        ),
    );
}

#[test]
fn labels_and_direct_navigation_restore_pixels_selection_and_dirty_state() {
    let mut s = session();
    let id = add(&mut s);
    s.mark_saved();
    let saved = s.snapshot();
    paint(&mut s, id, 1);
    let painted = s.snapshot();
    apply(
        &mut s,
        r#"[{"op":"selection.set_rect","rect":{"x":1,"y":0,"width":2,"height":2}}]"#,
    );
    let selection = s.selection();
    let view = s.history_view();
    assert_eq!(
        view.entries
            .iter()
            .map(|e| e.label.text())
            .collect::<Vec<_>>(),
        ["Initial state", "Add layer", "Fill", "Selection"]
    );
    assert_eq!(view.entries[2].state_id, view.entries[3].state_id);
    assert_ne!(view.entries[2].history_id, view.entries[3].history_id);
    assert!(view.entries[3].current);
    let epoch = s.ticket().publication_epoch;
    s.navigate_history(view.ticket, view.entries[1].history_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        s.ticket().publication_epoch,
        epoch + 1,
        "one atomic publication for any jump distance"
    );
    assert!(s.document().same_semantics(&saved));
    assert_eq!(
        s.document().layers[&id].raster.get(1, 0),
        Rgba([0x12, 0x34, 0x56, 0])
    );
    assert_eq!(s.selection(), Selection::Inactive);
    assert!(!s.is_dirty());
    let forward = s.history_view();
    assert_eq!(forward.entries.iter().filter(|e| e.redo).count(), 2);
    s.navigate_history(forward.ticket, view.entries[3].history_id)
        .unwrap()
        .unwrap();
    assert!(s.document().same_semantics(&painted));
    assert_eq!(s.selection(), selection);
    assert!(s.is_dirty());
    let same = s.history_view();
    let before = s.ticket();
    assert_eq!(
        s.navigate_history(same.ticket, view.entries[3].history_id)
            .unwrap(),
        None
    );
    assert_eq!(s.ticket(), before, "clicking current row is a no-op");
}

#[test]
fn stale_and_foreign_views_cannot_move_the_cursor() {
    let mut s = session();
    let initial = s.history_view();
    let mut other = Session::new((*s.snapshot()).clone());
    assert_eq!(
        other.ticket(),
        initial.ticket.base,
        "even an identical document and epoch are not the same session"
    );
    assert_eq!(
        other
            .navigate_history(initial.ticket, None)
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    let id = add(&mut s);
    let view = s.history_view();
    paint(&mut s, id, 1);
    let before = s.ticket();
    assert_eq!(
        s.navigate_history(view.ticket, None).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(s.ticket(), before);
    assert!(s.history_view().entries.last().unwrap().current);
}

#[test]
fn trimmed_or_replaced_redo_targets_are_rejected_without_a_partial_navigation() {
    let mut s = session();
    let id = add(&mut s);
    paint(&mut s, id, 0);
    let old_tip = s.history_view().entries.last().unwrap().history_id;
    s.undo().unwrap().unwrap();
    paint(&mut s, id, 2);
    let current = s.history_view();
    let before = s.ticket();
    assert_eq!(
        s.navigate_history(current.ticket, old_tip)
            .unwrap_err()
            .code,
        ErrorCode::ReferenceUnknownId
    );
    assert_eq!(s.ticket(), before);
    s.set_history_budget(0);
    paint(&mut s, id, 3);
    let trimmed = s.history_view();
    assert!(trimmed.trimmed_total > 0);
    assert!(trimmed.entries[0].history_id.is_some());
    let before = s.ticket();
    assert_eq!(
        s.navigate_history(trimmed.ticket, None).unwrap_err().code,
        ErrorCode::ReferenceUnknownId
    );
    assert_eq!(s.ticket(), before);
    assert!(trimmed.entries.last().unwrap().current);
}

#[test]
fn noops_failures_and_viewing_add_no_history_or_tile_leases() {
    let mut s = session();
    let id = add(&mut s);
    let doc = s.snapshot();
    let leases = std::sync::Arc::strong_count(&doc);
    let view = s.history_view();
    assert_eq!(std::sync::Arc::strong_count(&doc), leases);
    apply(&mut s, r#"[{"op":"selection.clear"}]"#);
    assert_eq!(s.history_view().entries, view.entries);
    let t = s.ticket();
    let batch = format!(
        r#"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"layer.delete","layer_ref":{{"id":"{}"}}}}]}}"#,
        t.state_id,
        t.publication_epoch,
        Uuid::new_v4()
    );
    assert!(
        s.apply(&Batch::from_json(batch.as_bytes()).unwrap())
            .is_err()
    );
    assert_eq!(s.history_view().entries, view.entries);
    assert!(s.document().layers.contains_key(&id));
}

#[test]
fn seeded_navigation_restores_every_retained_outcome_in_arbitrary_order() {
    let mut s = session();
    let id = add(&mut s);
    let mut seed = 0x82a7_92b1_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut outcomes = std::collections::HashMap::new();
    let initial = s.history_view();
    outcomes.insert(
        initial.entries.last().unwrap().history_id,
        (s.snapshot(), s.selection()),
    );
    let mut saved_state = s.ticket().state_id;
    s.mark_saved();
    for i in 0..60 {
        let n = next();
        let receipt = if i % 3 == 0 {
            apply(
                &mut s,
                if i % 2 == 0 {
                    r#"[{"op":"selection.clear"}]"#
                } else {
                    r#"[{"op":"selection.set_rect","rect":{"x":1,"y":1,"width":5,"height":5}}]"#
                },
            )
        } else {
            let (x, y) = (n % 8, (n >> 4) % 8);
            let color = format!(
                "#{:06X}{:02X}",
                n & 0xff_ffff,
                if i % 5 == 0 { 0 } else { 128 }
            );
            apply(
                &mut s,
                &format!(
                    r#"[{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":{x},"y":{y},"width":1,"height":1}},"color":"{color}","mode":"replace"}}]"#
                ),
            )
        };
        if receipt.committed {
            outcomes.insert(receipt.history_id, (s.snapshot(), s.selection()));
        }
        if i == 30 {
            s.mark_saved();
            saved_state = s.ticket().state_id;
        }
    }
    // Exclude the original empty root: the retained Add-layer state is this test's baseline.
    let retained = s
        .history_view()
        .entries
        .into_iter()
        .skip(1)
        .collect::<Vec<_>>();
    for _ in 0..200 {
        let target = &retained[next() as usize % retained.len()];
        let view = s.history_view();
        let current = view.entries.iter().find(|e| e.current).unwrap().history_id;
        let epoch = s.ticket().publication_epoch;
        let result = s.navigate_history(view.ticket, target.history_id).unwrap();
        assert_eq!(result.is_some(), current != target.history_id);
        assert_eq!(
            s.ticket().publication_epoch,
            epoch + u64::from(current != target.history_id)
        );
        let (doc, selection) = &outcomes[&target.history_id];
        assert!(s.document().same_semantics(doc));
        assert_eq!(s.selection(), *selection);
        assert_eq!(s.is_dirty(), s.ticket().state_id != saved_state);
    }
}
