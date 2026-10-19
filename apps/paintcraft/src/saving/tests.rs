use super::*;
use std::{sync::mpsc, time::Instant};
pub(super) fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!("paintcraft-ordered-save-{}", Uuid::new_v4()));
    std::fs::create_dir(&path).unwrap();
    std::fs::canonicalize(path).unwrap()
}
pub(super) fn app() -> App {
    let mut app = App::new(None);
    app.saving = Some(NativeSaving::new(egui::Context::default()).unwrap());
    app
}
pub(super) fn recovery_app(directory: PathBuf) -> App {
    let mut app = app().with_recovery(directory);
    drain(&mut app);
    app
}
pub(super) fn drain(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.saving.as_ref().unwrap().pending() {
        app.poll_native_saves(&egui::Context::default());
        assert!(Instant::now() < deadline, "save queue did not finish");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn submit(app: &mut App, path: PathBuf) {
    app.saving
        .as_mut()
        .unwrap()
        .submit(
            &app.editor,
            path,
            app.bound.as_deref(),
            egui::Context::default(),
        )
        .unwrap();
}
#[test]
fn snapshot_a_saves_while_b_stays_dirty_and_undo_a_is_clean() {
    let dir = scratch();
    let path = dir.join("a.pcnd");
    let mut app = app();
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let saved = app.editor.document_snapshot();
    let (release, hold) = mpsc::channel();
    app.saving
        .as_mut()
        .unwrap()
        .io
        .submit(
            1,
            move || {
                hold.recv().unwrap();
                Ok(Completion::File(save::save(
                    &dir.join("barrier"),
                    b"barrier",
                    false,
                )))
            },
            || {},
        )
        .unwrap();
    // This barrier is a coordinator test job, so consume it separately before
    // normal App completion handling (which owns only admitted user-save tickets).
    submit(&mut app, path.clone());
    app.editor.request(2, Action::AddLayer);
    app.editor.run_pending();
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if app.saving.as_mut().unwrap().io.poll().is_some() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    drain(&mut app);
    assert!(app.editor.is_dirty());
    let stored = Editor::open(&path).unwrap();
    assert!(stored.document().same_semantics(&saved));
    app.editor.request(3, Action::Undo);
    app.editor.run_pending();
    assert!(!app.editor.is_dirty());
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
#[test]
fn later_save_to_same_destination_wins_and_receipts_keep_real_sequence() {
    let dir = scratch();
    let path = dir.join("a.pcnd");
    let mut app = app();
    submit(&mut app, path.clone());
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    submit(&mut app, path.clone());
    drain(&mut app);
    assert!(
        Editor::open(&path)
            .unwrap()
            .document()
            .same_semantics(app.editor.document())
    );
    assert!(!app.editor.is_dirty());
    let receipt = app.saving.as_ref().unwrap().last_receipt.as_ref().unwrap();
    assert_eq!(
        receipt.get("save_sequence").and_then(Value::as_str),
        Some("2")
    );
    assert_eq!(
        receipt.get("binding_generation").and_then(Value::as_str),
        Some("2")
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn stale_save_as_completion_cannot_retarget_later_failed_choice() {
    let dir = scratch();
    let original = dir.join("original.pcnd");
    let mut app = app();
    app.bound = Some(original.clone());
    app.editor.set_path(original.clone());
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    submit(&mut app, dir.join("older-choice.pcnd"));
    submit(&mut app, dir.join("missing/latest-choice.pcnd"));
    drain(&mut app);
    assert_eq!(app.bound, Some(original.clone()));
    assert_eq!(app.editor.path(), Some(original.as_path()));
    assert!(app.editor.is_dirty());
    assert!(
        dir.join("older-choice.pcnd").exists(),
        "external successful write is retained despite stale association"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn completion_from_replaced_editor_cannot_clean_or_bind_the_new_editor() {
    let dir = scratch();
    let mut app = app();
    submit(&mut app, dir.join("old-session.pcnd"));
    app.install(Editor::new_document(64, 64).unwrap()).unwrap();
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    drain(&mut app);
    assert!(app.bound.is_none());
    assert!(app.editor.path().is_none());
    assert!(app.editor.is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn closing_waits_for_accepted_saves_and_failure_keeps_the_window_open() {
    let dir = scratch();
    let mut app = app();
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    submit(&mut app, dir.join("missing/fails.pcnd"));
    app.allow_close = true;
    app.close_after_requested_saves(&egui::Context::default());
    assert!(
        app.close_after_saves,
        "accepted save must drain even after quit confirmation"
    );
    drain(&mut app);
    assert!(!app.close_after_saves);
    assert!(
        !app.allow_close,
        "failed save needs a new explicit decision"
    );
    assert!(app.editor.is_dirty());
    assert!(
        app.editor
            .status()
            .contains("failed or remains unconfirmed")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

pub(super) fn pause_queue(app: &mut App) -> mpsc::Sender<()> {
    let (release, hold) = mpsc::channel();
    let saving = app.saving.as_mut().unwrap();
    let ticket = saving
        .io
        .submit(
            0,
            move || {
                hold.recv().unwrap();
                Ok(Completion::Cleanup)
            },
            || {},
        )
        .unwrap();
    saving.pending.push(Pending {
        ticket,
        kind: PendingKind::Cleanup,
    });
    release
}
fn checkpoint(app: &mut App) -> bool {
    app.saving
        .as_mut()
        .unwrap()
        .checkpoint(&app.editor)
        .unwrap()
}
#[test]
fn background_checkpoint_captures_a_without_blocking_edits_to_b() {
    let dir = scratch();
    let mut app = recovery_app(dir.clone());
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let a = app.editor.document_snapshot();
    let release = pause_queue(&mut app);
    assert!(checkpoint(&mut app));
    assert!(
        !app.editor.has_checkpoint(),
        "submission is not a durable checkpoint"
    );
    app.editor.request(2, Action::AddLayer);
    app.editor.run_pending();
    assert!(
        !checkpoint(&mut app),
        "only one current-owner checkpoint is admitted at once"
    );
    release.send(()).unwrap();
    drain(&mut app);
    assert_eq!(app.editor.checkpoint_info().unwrap().state_id, a.state_id);
    assert_ne!(app.editor.document().state_id, a.state_id);
    let b = app.editor.document_snapshot();
    assert!(checkpoint(&mut app));
    drain(&mut app);
    assert_eq!(app.editor.checkpoint_info().unwrap().state_id, b.state_id);
    drop(app);
    let found = Recovery::scan(&dir);
    assert_eq!(found.len(), 1);
    assert!(
        Recovery::read_checkpoint(&found[0].checkpoint)
            .unwrap()
            .same_semantics(&b)
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn checkpoint_clear_and_new_checkpoint_are_fenced_and_ordered() {
    let dir = scratch();
    let mut app = recovery_app(dir.clone());
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let release = pause_queue(&mut app);
    assert!(checkpoint(&mut app));
    app.saving
        .as_mut()
        .unwrap()
        .clear_recovery(&mut app.editor)
        .unwrap();
    app.editor.request(2, Action::AddLayer);
    app.editor.run_pending();
    let expected = app.editor.document_snapshot();
    assert!(checkpoint(&mut app));
    release.send(()).unwrap();
    drain(&mut app);
    assert_eq!(
        app.editor.checkpoint_info().unwrap().sequence,
        1,
        "old queued checkpoint was superseded before publication"
    );
    assert_eq!(
        app.editor.checkpoint_info().unwrap().state_id,
        expected.state_id
    );
    drop(app);
    let found = Recovery::scan(&dir);
    assert_eq!(found.len(), 1);
    assert!(
        Recovery::read_checkpoint(&found[0].checkpoint)
            .unwrap()
            .same_semantics(&expected)
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn replacing_editor_does_not_wait_for_old_checkpoint_or_resurrect_it() {
    let dir = scratch();
    let mut app = recovery_app(dir.clone());
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let release = pause_queue(&mut app);
    assert!(checkpoint(&mut app));
    let installed = app.install(Editor::new_document(64, 64).unwrap());
    release.send(()).unwrap();
    installed.unwrap();
    assert_eq!(app.editor.document().canvas, (64, 64));
    drain(&mut app);
    assert!(app.editor.checkpoint_info().is_none());
    drop(app);
    assert!(Recovery::scan(&dir).is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn full_queue_refuses_document_replacement_before_invalidating_recovery() {
    let dir = scratch();
    let mut app = recovery_app(dir.clone());
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let token = app.editor.recovery_handle().unwrap().token();
    let key = app.editor.render_key();
    let release = pause_queue(&mut app);
    for _ in 0..3 {
        let saving = app.saving.as_mut().unwrap();
        let ticket = saving
            .io
            .submit(1, || Ok(Completion::Cleanup), || {})
            .unwrap();
        saving.pending.push(Pending {
            ticket,
            kind: PendingKind::Cleanup,
        });
    }
    let refused = app.install(Editor::new_document(64, 64).unwrap());
    release.send(()).unwrap();
    assert_eq!(refused.unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(app.editor.render_key(), key);
    assert!(app.editor.recovery_handle().unwrap().is_current(token));
    drain(&mut app);
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn clean_save_drains_recovery_cleanup_without_adopting_a_stale_checkpoint() {
    let dir = scratch();
    let recovery_dir = dir.join("recovery");
    let path = dir.join("saved.pcnd");
    let mut app = recovery_app(recovery_dir.clone());
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let expected = app.editor.document_snapshot();
    let release = pause_queue(&mut app);
    submit(&mut app, path.clone());
    assert!(checkpoint(&mut app));
    release.send(()).unwrap();
    drain(&mut app);
    assert!(!app.editor.is_dirty());
    assert!(app.editor.checkpoint_info().is_none());
    assert!(
        Editor::open(&path)
            .unwrap()
            .document()
            .same_semantics(&expected)
    );
    drop(app);
    assert!(Recovery::scan(&recovery_dir).is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn undo_to_saved_state_closes_after_clearing_a_newer_recovery_snapshot() {
    let dir = scratch();
    let mut app = recovery_app(dir.clone());
    // The initial state is clean. B is checkpointed, then undo returns to A.
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    assert!(checkpoint(&mut app));
    drain(&mut app);
    assert!(app.editor.checkpoint_info().is_some());
    app.editor.request(2, Action::Undo);
    app.editor.run_pending();
    assert!(!app.editor.is_dirty());
    app.close_after_requested_saves(&egui::Context::default());
    assert!(app.close_after_saves);
    drain(&mut app);
    assert!(app.editor.checkpoint_info().is_none());
    drop(app);
    assert!(
        Recovery::scan(&dir).is_empty(),
        "undone edits must not reappear after clean exit"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn startup_discovery_and_initialization_leave_the_editor_responsive() {
    let dir = scratch();
    let mut app = app();
    let release = pause_queue(&mut app);
    app = app.with_recovery(dir.clone());
    assert!(app.editor.recovery_handle().is_none());
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "configuration must not create files on the UI path"
    );
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let key = app.editor.render_key();
    release.send(()).unwrap();
    drain(&mut app);
    assert!(app.editor.recovery_handle().is_some());
    assert_eq!(
        app.editor.render_key(),
        key,
        "initialization must not replace edits made while scanning"
    );
    assert!(!app.recovery_needs_scan && !app.recovery_needs_init);
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn stale_startup_owner_is_retired_and_the_new_editor_gets_its_own_owner() {
    let dir = scratch();
    let mut app = app();
    let release = pause_queue(&mut app);
    app = app.with_recovery(dir.clone());
    app.install(Editor::new_document(64, 64).unwrap()).unwrap();
    assert!(
        app.recovery_needs_init,
        "exclusive discovery retains its admission until consumed"
    );
    release.send(()).unwrap();
    drain(&mut app);
    assert!(app.editor.recovery_handle().is_some());
    assert_eq!(app.editor.document().canvas, (64, 64));
    assert!(!app.recovery_needs_init);
    drop(app);
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "stale empty owners must be cleaned up"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
fn make_orphan(directory: &Path) -> Uuid {
    let recovery = Recovery::start(directory).unwrap();
    recovery
        .checkpoint_document(Editor::new_document(4, 4).unwrap().document(), None)
        .unwrap();
    recovery.session()
}
#[test]
fn discovery_results_follow_the_current_directory_configuration() {
    let a = scratch();
    let b = scratch();
    let a_id = make_orphan(&a);
    let b_id = make_orphan(&b);
    let mut app = app();
    let release = pause_queue(&mut app);
    app = app.with_recovery(a.clone()).with_recovery(b.clone());
    release.send(()).unwrap();
    drain(&mut app);
    assert_eq!(app.recoverable.len(), 1);
    assert_eq!(app.recoverable[0].session, b_id);
    assert_ne!(app.recoverable[0].session, a_id);
    assert_eq!(
        Recovery::scan(&a).len(),
        1,
        "other directory's orphan is preserved"
    );
    drop(app);
    std::fs::remove_dir_all(a).unwrap();
    std::fs::remove_dir_all(b).unwrap();
}
#[test]
fn recovery_initialization_failure_does_not_stop_the_editor_or_invent_protection() {
    let dir = scratch();
    let blocked = dir.join("not-a-directory");
    std::fs::write(&blocked, b"blocked").unwrap();
    let mut app = app().with_recovery(blocked);
    drain(&mut app);
    assert!(app.editor.recovery_handle().is_none());
    assert!(!app.recovery_needs_init);
    assert!(app.editor.status().contains("Crash recovery unavailable"));
    app.editor.request(1, Action::AddLayer);
    assert_eq!(app.editor.run_pending(), 1);
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn startup_configuration_does_not_silently_migrate_an_active_store() {
    let a = scratch();
    let b = scratch();
    let mut app = recovery_app(a.clone());
    let token = app.editor.recovery_handle().unwrap().token();
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    assert!(checkpoint(&mut app));
    drain(&mut app);
    app = app.with_recovery(b.clone());
    assert_eq!(app.recovery_dir.as_deref(), Some(a.as_path()));
    assert!(app.editor.recovery_handle().unwrap().is_current(token));
    assert!(app.editor.has_checkpoint());
    assert!(
        app.editor
            .status()
            .contains("Existing recovery data has been retained")
    );
    assert_eq!(std::fs::read_dir(&b).unwrap().count(), 0);
    drop(app);
    assert_eq!(Recovery::scan(&a).len(), 1);
    std::fs::remove_dir_all(a).unwrap();
    std::fs::remove_dir_all(b).unwrap();
}

#[test]
fn repeated_layout_passes_submit_one_save() {
    let dir = scratch();
    let mut app = app();
    let ctx = egui::Context::default();
    app.do_save(&ctx, dir.join("a.pcnd"));
    app.do_save(&ctx, dir.join("a.pcnd"));
    assert_eq!(app.saving.as_ref().unwrap().pending.len(), 1);
    drain(&mut app);
    std::fs::remove_dir_all(dir).unwrap();
}
