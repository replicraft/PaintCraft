use super::*;
use crate::saving::tests::{drain, recovery_app, scratch};

#[test]
fn confirmed_native_saves_retire_their_write_ahead_snapshots_on_the_fifo() {
    let root = scratch();
    let mut app = recovery_app(root.clone());
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    app.do_save(&egui::Context::default(), root.join("saved.pcnd"));
    drain(&mut app);
    assert!(!app.editor.is_dirty());
    assert!(app.saving.as_ref().unwrap().retired_saves.is_empty());
    drop(app);
    assert!(Recovery::scan(&root).is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_save_retains_the_exact_snapshot_and_receipt_after_the_window_is_dropped() {
    let root = scratch();
    let mut app = recovery_app(root.clone());
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let captured = app.editor.document_snapshot();
    app.do_save(&egui::Context::default(), root.join("missing/a.pcnd"));
    drain(&mut app);
    assert!(app.editor.is_dirty());
    let receipt = app.saving.as_ref().unwrap().last_receipt.clone().unwrap();
    // Replacing the current document must not retire the failed save's snapshot.
    app.install(Editor::new_document(7, 9).unwrap()).unwrap();
    drain(&mut app);
    drop(app);
    let found = Recovery::scan(&root);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].state_id, captured.state_id);
    let record = paintcraft_formats::save_recovery::read_record(&found[0]).unwrap();
    assert_eq!(record.get("receipt"), Some(&receipt));
    let claim = Recovery::claim(&found[0]).unwrap();
    assert_eq!(claim.document.state_id, captured.state_id);
    assert_eq!(claim.document.layers.len(), captured.layers.len());
    claim.lease.discard().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unavailable_write_ahead_storage_refuses_save_before_touching_the_destination() {
    let root = scratch();
    let mut app = recovery_app(root.clone());
    std::fs::write(root.join("saved-attempts"), b"not a directory").unwrap();
    let destination = root.join("existing.pcnd");
    std::fs::write(&destination, b"unchanged").unwrap();
    app.do_save(&egui::Context::default(), destination.clone());
    drain(&mut app);
    assert_eq!(std::fs::read(&destination).unwrap(), b"unchanged");
    assert!(app.bound.is_none());
    assert!(app.saving.as_ref().unwrap().failed());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_fresh_native_coordinator_offers_and_restores_a_retained_failed_save_as_unsaved_work() {
    let root = scratch();
    let mut first = recovery_app(root.clone());
    first.editor.request(1, Action::AddLayer);
    first.editor.run_pending();
    let captured = first.editor.document_snapshot();
    first.do_save(&egui::Context::default(), root.join("missing/a.pcnd"));
    drain(&mut first);
    drop(first);
    let mut restarted = recovery_app(root.clone());
    assert_eq!(restarted.recoverable.len(), 1);
    let found = restarted.recoverable[0].clone();
    assert_eq!(found.state_id, captured.state_id);
    restarted.do_recover(found.clone());
    drain(&mut restarted);
    assert_eq!(restarted.editor.document().state_id, captured.state_id);
    assert!(restarted.editor.is_dirty());
    assert!(restarted.bound.is_none());
    assert!(restarted.editor.path().is_none());
    assert!(restarted.editor.has_checkpoint());
    assert!(
        !found.checkpoint.exists(),
        "new protection precedes old-source retirement"
    );
    assert!(restarted.recovery_source.is_none());
    drop(restarted);
    std::fs::remove_dir_all(root).unwrap();
}

fn retained_incomplete(app: &mut App, destination: PathBuf) -> u64 {
    let snapshot = app.editor.document_snapshot();
    let charge = snapshot_charge(&snapshot).unwrap();
    let saving = app.saving.as_mut().unwrap();
    let directory = saving.retention_directory.clone();
    let session = saving.session;
    let generation = saving.generation + 1;
    let document = snapshot.clone();
    let path = destination.clone();
    let ticket = saving
        .submit_io_with_ticket(
            charge,
            move |ticket| {
                let Completion::RetainedFile(mut file) = save_document(
                    &document,
                    &path,
                    directory.as_deref(),
                    Stamp {
                        session,
                        sequence: ticket.sequence(),
                        generation,
                        operation: "save-as",
                    },
                )?
                else {
                    panic!("configured retention is available")
                };
                // Fault-model the destination barrier uncertainty at coordinator delivery;
                // platform tests separately inject actual barrier/identity failures.
                file.outcome.durable = false;
                file.retention.as_ref().unwrap().finish(&file.outcome)?;
                Ok(Completion::RetainedFile(file))
            },
            || {},
        )
        .unwrap();
    saving.generation = generation;
    saving.pending.push(Pending {
        ticket,
        kind: PendingKind::Save(SaveRequest {
            session,
            generation,
            source: snapshot.state_id,
            path: destination,
            operation: "save-as",
            snapshot,
            charge,
        }),
    });
    ticket.sequence()
}

#[test]
fn explicit_forget_retires_the_persisted_snapshot_and_keeps_the_user_file() {
    let root = scratch();
    let destination = root.join("a.pcnd");
    let mut app = recovery_app(root.clone());
    let sequence = retained_incomplete(&mut app, destination.clone());
    drain(&mut app);
    let before = std::fs::read(&destination).unwrap();
    app.forget_incomplete_save(sequence);
    app.poll_native_saves(&egui::Context::default());
    drain(&mut app);
    drop(app);
    assert_eq!(std::fs::read(&destination).unwrap(), before);
    assert!(Recovery::scan(&root).is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn confirmed_reconciliation_retires_the_persisted_snapshot_after_acknowledgement() {
    let root = scratch();
    let destination = root.join("a.pcnd");
    let mut app = recovery_app(root.clone());
    let sequence = retained_incomplete(&mut app, destination.clone());
    drain(&mut app);
    app.saving
        .as_mut()
        .unwrap()
        .request_reconciliation(sequence, save::ReconcileMode::ConfirmDurability)
        .unwrap();
    drain(&mut app);
    assert_eq!(app.bound.as_deref(), Some(destination.as_path()));
    assert!(!app.saving.as_ref().unwrap().failed());
    drop(app);
    assert!(Recovery::scan(&root).is_empty());
    std::fs::remove_dir_all(root).unwrap();
}
