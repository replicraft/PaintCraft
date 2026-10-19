use super::*;
use crate::saving::tests::{app, drain, scratch};

// Platform barrier fault injection has its own suite. Here a real replacement
// with an unconfirmed durability flag exercises native coordinator adoption.
fn incomplete(app: &mut App, path: &Path) -> u64 {
    let snapshot = app.editor.document_snapshot();
    let bytes = paintcraft_formats::pcnd::write(&snapshot).unwrap();
    let mut outcome = save::save(path, &bytes, true);
    assert!(outcome.is_success());
    outcome.durable = false;
    let saving = app.saving.as_mut().unwrap();
    let charge = snapshot_charge(&snapshot).unwrap();
    saving.admit_save(charge).unwrap();
    let ticket = saving
        .submit_io(charge, move || Ok(Completion::File(outcome)), || {})
        .unwrap();
    let generation = saving.generation + 1;
    saving.generation = generation;
    saving.pending.push(Pending {
        ticket,
        kind: PendingKind::Save(SaveRequest {
            session: saving.session,
            generation,
            source: snapshot.state_id,
            snapshot,
            charge,
            path: path.into(),
            operation: "save-as",
        }),
    });
    ticket.sequence()
}

#[test]
fn inspect_does_not_acknowledge_but_confirming_durability_can_adopt_the_captured_state() {
    let dir = scratch();
    let path = dir.join("a.pcnd");
    let mut app = app();
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let state_a = app.editor.document().state_id;
    let sequence = incomplete(&mut app, &path);
    drain(&mut app);
    assert!(app.editor.is_dirty());
    assert!(app.bound.is_none());
    assert_eq!(app.saving.as_ref().unwrap().attempts.len(), 1);
    let first = app.saving.as_ref().unwrap().last_receipt.clone().unwrap();
    app.saving
        .as_mut()
        .unwrap()
        .request_reconciliation(sequence, save::ReconcileMode::Inspect)
        .unwrap();
    drain(&mut app);
    assert!(app.editor.is_dirty());
    assert!(app.bound.is_none());
    assert_eq!(app.saving.as_ref().unwrap().attempts.len(), 1);
    app.editor.request(2, Action::AddLayer);
    app.editor.run_pending();
    app.saving
        .as_mut()
        .unwrap()
        .request_reconciliation(sequence, save::ReconcileMode::ConfirmDurability)
        .unwrap();
    drain(&mut app);
    assert!(app.editor.is_dirty(), "newer edits were not saved");
    assert_eq!(app.bound.as_deref(), Some(path.as_path()));
    let saving = app.saving.as_ref().unwrap();
    assert!(saving.attempts.is_empty());
    assert!(!saving.failed());
    assert_eq!(
        saving.last_receipt.as_ref().unwrap().get("operation_id"),
        first.get("operation_id")
    );
    assert_eq!(
        saving.last_receipt.as_ref().unwrap().get("save_sequence"),
        first.get("save_sequence")
    );
    assert_eq!(
        saving
            .last_receipt
            .as_ref()
            .unwrap()
            .get("acknowledged_state_id"),
        Some(&Value::str(state_a.to_string()))
    );
    app.editor.request(3, Action::Undo);
    app.editor.run_pending();
    assert!(
        !app.editor.is_dirty(),
        "undo to the captured state is clean"
    );
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_later_success_cannot_hide_an_incomplete_save_or_let_it_regress_the_association() {
    let dir = scratch();
    let old = dir.join("old.pcnd");
    let new = dir.join("new.pcnd");
    let mut app = app();
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let sequence = incomplete(&mut app, &old);
    drain(&mut app);
    let captured = app.saving.as_ref().unwrap().attempts[0]
        .request
        .snapshot
        .state_id;
    app.editor.request(2, Action::AddLayer);
    app.editor.run_pending();
    app.do_save(&egui::Context::default(), new.clone());
    drain(&mut app);
    assert!(!app.editor.is_dirty());
    assert_eq!(app.bound.as_deref(), Some(new.as_path()));
    assert!(
        app.saving.as_ref().unwrap().failed(),
        "the older incomplete save remains visible"
    );
    let latest = app.saving.as_ref().unwrap().last_receipt.clone();
    assert_eq!(
        app.saving.as_ref().unwrap().attempts[0]
            .request
            .snapshot
            .state_id,
        captured
    );
    app.saving
        .as_mut()
        .unwrap()
        .request_reconciliation(sequence, save::ReconcileMode::ConfirmDurability)
        .unwrap();
    drain(&mut app);
    assert_eq!(app.bound.as_deref(), Some(new.as_path()));
    assert!(!app.editor.is_dirty());
    assert_eq!(
        app.saving.as_ref().unwrap().last_receipt,
        latest,
        "an older receipt cannot displace a newer one"
    );
    assert!(app.saving.as_ref().unwrap().attempts.is_empty());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn older_same_generation_completion_cannot_regress_a_newer_saved_checkpoint() {
    let dir = scratch();
    let path = dir.join("a.pcnd");
    let mut app = app();
    let sequence = incomplete(&mut app, &path);
    drain(&mut app);
    // Acknowledged Save B is tested separately against real disk content. Here
    // replay a success for A after a later acknowledged sequence in the same
    // binding generation, so matching generation alone would be insufficient.
    let mut attempt = app.saving.as_mut().unwrap().attempts.remove(0);
    app.saving.as_mut().unwrap().adopted_sequence = sequence + 1;
    attempt.outcome.as_mut().unwrap().durable = true;
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let before = app.editor.document().state_id;
    app.adopt_saved_attempt(
        attempt.ticket,
        &attempt.request,
        attempt.outcome.as_ref().unwrap(),
    );
    assert_eq!(app.editor.document().state_id, before);
    assert!(app.bound.is_none());
    assert!(app.editor.is_dirty());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn replacement_of_the_editor_does_not_drop_or_adopt_a_previous_sessions_attempt() {
    let dir = scratch();
    let path = dir.join("a.pcnd");
    let mut app = app();
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let sequence = incomplete(&mut app, &path);
    drain(&mut app);
    app.install(Editor::new_document(7, 9).unwrap()).unwrap();
    let key = app.editor.render_key();
    app.saving
        .as_mut()
        .unwrap()
        .request_reconciliation(sequence, save::ReconcileMode::ConfirmDurability)
        .unwrap();
    drain(&mut app);
    assert_eq!(app.editor.render_key(), key);
    assert_eq!(app.editor.document().canvas, (7, 9));
    assert!(app.bound.is_none());
    assert!(app.saving.as_ref().unwrap().attempts.is_empty());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn retained_snapshots_charge_admission_and_a_full_ledger_does_not_change_binding_generation() {
    let dir = scratch();
    let mut app = app();
    for i in 0..ATTEMPTS_MAX {
        // Small empty snapshots avoid independent tile charges beyond the
        // conservative 64 MiB base estimate per captured document.
        app.install(Editor::new_document(1, 1).unwrap()).unwrap();
        let result = app.saving.as_ref().unwrap().admit(1, 64 << 20);
        if result.is_err() {
            break;
        }
        incomplete(&mut app, &dir.join(format!("{i}.pcnd")));
        drain(&mut app);
    }
    let saving = app.saving.as_ref().unwrap();
    assert!(!saving.attempts.is_empty());
    assert!(
        saving.admit(1, 1 << 30).is_err(),
        "retained snapshots are part of the I/O budget"
    );
    let generation = saving.generation;
    let result = app.saving.as_mut().unwrap().submit(
        &app.editor,
        dir.join("overflow.pcnd"),
        None,
        egui::Context::default(),
    );
    assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(app.saving.as_ref().unwrap().generation, generation);
    assert!(!dir.join("overflow.pcnd").exists());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn opening_a_retained_snapshot_recovers_captured_work_after_the_source_is_deleted() {
    let dir = scratch();
    let path = dir.join("a.pcnd");
    let mut app = app();
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let captured = app.editor.document_snapshot();
    let sequence = incomplete(&mut app, &path);
    drain(&mut app);
    app.install(Editor::new_document(7, 9).unwrap()).unwrap();
    std::fs::remove_file(&path).unwrap();
    app.open_saved_snapshot(sequence);
    assert_eq!(app.editor.document().state_id, captured.state_id);
    assert_eq!(app.editor.document().layers.len(), captured.layers.len());
    assert!(app.editor.is_dirty());
    assert!(app.bound.is_none());
    assert!(app.editor.path().is_none());
    assert_eq!(app.saving.as_ref().unwrap().attempts.len(), 1);
    app.close_after_requested_saves(&egui::Context::default());
    assert_eq!(app.confirm, Some(Confirm::QuitUnsaved));
    assert!(!app.allow_close);
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_worker_failure_retains_the_snapshot_without_inventing_replacement_evidence() {
    let dir = scratch();
    let mut app = app();
    let snapshot = app.editor.document_snapshot();
    let charge = snapshot_charge(&snapshot).unwrap();
    let saving = app.saving.as_mut().unwrap();
    let ticket = saving
        .submit_io(
            charge,
            || {
                Err(Error::new(
                    ErrorCode::StorageFailed,
                    "injected worker failure",
                ))
            },
            || {},
        )
        .unwrap();
    saving.pending.push(Pending {
        ticket,
        kind: PendingKind::Save(SaveRequest {
            session: saving.session,
            generation: saving.generation,
            source: snapshot.state_id,
            snapshot,
            charge,
            path: dir.join("a.pcnd"),
            operation: "save-as",
        }),
    });
    drain(&mut app);
    let saving = app.saving.as_mut().unwrap();
    assert!(saving.failed());
    assert_eq!(saving.attempts.len(), 1);
    assert!(saving.attempts[0].outcome.is_none());
    assert!(
        saving
            .request_reconciliation(ticket.sequence(), save::ReconcileMode::Inspect)
            .is_err()
    );
    app.open_saved_snapshot(ticket.sequence());
    assert!(app.editor.is_dirty());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn successful_reconciliation_binds_the_captured_namespace_after_a_dialog_alias_changes() {
    use std::os::unix::fs::symlink;
    let dir = scratch();
    let actual = dir.join("actual.pcnd");
    let alias = dir.join("alias.pcnd");
    let other = dir.join("other.pcnd");
    let mut app = app();
    // Create before linking so initial resolution captures the target.
    std::fs::write(&actual, b"old").unwrap();
    symlink(&actual, &alias).unwrap();
    let sequence = incomplete(&mut app, &alias);
    drain(&mut app);
    std::fs::write(&other, b"untouched").unwrap();
    std::fs::remove_file(&alias).unwrap();
    symlink(&other, &alias).unwrap();
    app.saving
        .as_mut()
        .unwrap()
        .request_reconciliation(sequence, save::ReconcileMode::ConfirmDurability)
        .unwrap();
    drain(&mut app);
    assert_eq!(app.bound.as_deref(), Some(actual.as_path()));
    assert_eq!(app.editor.path(), Some(actual.as_path()));
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    app.do_save(&egui::Context::default(), app.bound.clone().unwrap());
    drain(&mut app);
    assert_eq!(std::fs::read(&other).unwrap(), b"untouched");
    assert!(!app.editor.is_dirty());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn repeated_reconciliation_activation_admits_only_one_job_per_attempt() {
    let dir = scratch();
    let mut app = app();
    let sequence = incomplete(&mut app, &dir.join("a.pcnd"));
    drain(&mut app);
    let saving = app.saving.as_mut().unwrap();
    saving
        .request_reconciliation(sequence, save::ReconcileMode::Inspect)
        .unwrap();
    saving
        .request_reconciliation(sequence, save::ReconcileMode::ConfirmDurability)
        .unwrap();
    assert_eq!(saving.pending.len(), 1);
    drain(&mut app);
    assert!(
        app.bound.is_none(),
        "the duplicate activation did not turn inspection into a write barrier"
    );
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn user_controls_inspect_and_confirm_through_real_multipass_layout() {
    use egui_kittest::{Harness, kittest::Queryable};
    let dir = scratch();
    let path = dir.join("a.pcnd");
    let mut app = app();
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    incomplete(&mut app, &path);
    drain(&mut app);
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1100.0, 900.0))
        .build_ui_state(
            |ui, app: &mut App| {
                app.ui(ui);
                if ui.ctx().current_pass_index() == 0 {
                    ui.ctx()
                        .request_discard("test incomplete-save multipass activation");
                }
            },
            app,
        );
    harness.run();
    harness.get_by_label("Incomplete saves (1)").click();
    harness.run();
    harness.get_by_label("Inspect destination").click();
    harness.step();
    drain(harness.state_mut());
    assert!(harness.state().editor.is_dirty());
    assert_eq!(harness.state().saving.as_ref().unwrap().attempts.len(), 1);
    harness.run();
    harness.get_by_label("Confirm durability").click();
    harness.step();
    drain(harness.state_mut());
    assert!(!harness.state().editor.is_dirty());
    assert_eq!(harness.state().bound.as_deref(), Some(path.as_path()));
    assert!(harness.state().saving.as_ref().unwrap().attempts.is_empty());
    drop(harness);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn discarding_a_captured_snapshot_requires_an_explicit_ui_choice() {
    use egui_kittest::{Harness, kittest::Queryable};
    let dir = scratch();
    let path = dir.join("a.pcnd");
    let mut app = app();
    incomplete(&mut app, &path);
    drain(&mut app);
    let before = std::fs::read(&path).unwrap();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1100.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    harness.run();
    harness.get_by_label("Incomplete saves (1)").click();
    harness.run();
    harness.get_by_label("Forget captured snapshot").click();
    harness.run();
    assert!(matches!(
        harness.state().confirm,
        Some(Confirm::ForgetSaveSnapshot(_))
    ));
    assert_eq!(harness.state().saving.as_ref().unwrap().attempts.len(), 1);
    harness.get_by_label("Cancel").click();
    harness.run();
    assert_eq!(harness.state().saving.as_ref().unwrap().attempts.len(), 1);
    harness.get_by_label("Forget captured snapshot").click();
    harness.run();
    harness.get_by_label("Forget snapshot").click();
    harness.run();
    assert!(harness.state().saving.as_ref().unwrap().attempts.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    drop(harness);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_admission_preserves_capacity_for_reconciliation_controls() {
    let mut app = app();
    let saving = app.saving.as_mut().unwrap();
    assert_eq!(
        saving.admit_save(1 << 30).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(saving.admit_save((1 << 30) - CONTROL_RESERVE).is_ok());
    assert!(saving.pending.is_empty());
}

#[test]
fn opening_a_captured_snapshot_asks_before_discarding_newer_work() {
    use egui_kittest::{Harness, kittest::Queryable};
    let dir = scratch();
    let mut app = app();
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let captured = app.editor.document().state_id;
    incomplete(&mut app, &dir.join("a.pcnd"));
    drain(&mut app);
    app.editor.request(2, Action::AddLayer);
    app.editor.run_pending();
    let newer = app.editor.document().state_id;
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1100.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    harness.run();
    harness.get_by_label("Incomplete saves (1)").click();
    harness.run();
    harness.get_by_label("Open captured snapshot").click();
    harness.run();
    assert_eq!(harness.state().editor.document().state_id, newer);
    assert!(matches!(
        harness.state().confirm,
        Some(Confirm::DiscardForSaveSnapshot(_))
    ));
    harness.get_by_label("Cancel").click();
    harness.run();
    assert_eq!(harness.state().editor.document().state_id, newer);
    harness.get_by_label("Open captured snapshot").click();
    harness.run();
    harness.get_by_label("Continue").click();
    harness.run();
    assert_eq!(harness.state().editor.document().state_id, captured);
    assert!(harness.state().editor.is_dirty());
    assert!(harness.state().bound.is_none());
    drop(harness);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn clean_documents_with_uncommitted_drafts_also_require_discard_confirmation() {
    let dir = scratch();
    let mut app = app();
    let sequence = incomplete(&mut app, &dir.join("a.pcnd"));
    drain(&mut app);
    let target = app.editor.target().unwrap();
    assert!(!app.editor.is_dirty());
    app.name_edit = Some((target, "keep this draft".into()));
    app.saved_snapshot_requested(sequence);
    assert_eq!(app.confirm, Some(Confirm::DiscardForSaveSnapshot(sequence)));
    assert_eq!(app.name_edit.as_ref().unwrap().1, "keep this draft");
    app.confirm = None;
    app.name_edit = None;
    app.opacity_edit = Some((target, 100));
    app.saved_snapshot_requested(sequence);
    assert_eq!(app.confirm, Some(Confirm::DiscardForSaveSnapshot(sequence)));
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn known_failed_saves_retain_captured_work_but_do_not_allow_reconciliation() {
    let dir = scratch();
    let mut app = app();
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let captured = app.editor.document().state_id;
    app.do_save(&egui::Context::default(), dir.join("missing/a.pcnd"));
    drain(&mut app);
    let saving = app.saving.as_mut().unwrap();
    assert_eq!(saving.attempts.len(), 1);
    let attempt = &saving.attempts[0];
    assert_eq!(
        attempt.outcome.as_ref().unwrap().replacement,
        save::Replacement::NotReplaced
    );
    let sequence = attempt.ticket.sequence();
    assert!(
        saving
            .request_reconciliation(sequence, save::ReconcileMode::Inspect)
            .is_err()
    );
    app.install(Editor::new_document(7, 9).unwrap()).unwrap();
    app.open_saved_snapshot(sequence);
    assert_eq!(app.editor.document().state_id, captured);
    assert!(app.editor.is_dirty());
    app.forget_incomplete_save(sequence);
    assert!(app.saving.as_ref().unwrap().attempts.is_empty());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
