use super::super::tests::{drain, pause_queue, recovery_app, scratch};
use super::*;
use std::{sync::mpsc, time::Instant};

fn orphan(directory: &Path) -> Found {
    let owner = Recovery::start(directory).unwrap();
    let mut editor = Editor::new_document(13, 7).unwrap();
    editor.request(1, Action::AddLayer);
    editor.run_pending();
    editor.color = Rgba([23, 45, 67, 255]);
    editor.pointer_down((2, 3));
    editor.pointer_move((9, 3));
    editor.pointer_up();
    editor.run_pending();
    owner.checkpoint_document(editor.document(), None).unwrap();
    drop(owner);
    Recovery::scan(directory).into_iter().next().unwrap()
}
fn take_restore(app: &mut App) -> (Request, Result<Completion>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some((_, kind, result)) = app.saving.as_mut().unwrap().poll_completion() {
            if let PendingKind::Restore(request) = kind {
                return (*request, result);
            }
            panic!("expected the sole restore job");
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn background_restore_keeps_its_source_until_a_new_checkpoint_is_confirmed() {
    let dir = scratch();
    let offered = orphan(&dir);
    let expected = Recovery::read_checkpoint(&offered.checkpoint).unwrap();
    let mut app = recovery_app(dir.clone());
    app.do_recover(offered.clone());
    let (request, result) = take_restore(&mut app);
    let release = pause_queue(&mut app);
    app.apply_claimed_recovery(request, result);
    assert!(app.editor.document().same_semantics(&expected));
    assert!(app.editor.is_dirty());
    assert!(app.editor.checkpoint_info().is_none());
    assert!(offered.checkpoint.exists());
    assert!(
        Recovery::discard(&dir, offered.session).is_err(),
        "source lock survives adoption and initialization delay"
    );
    release.send(()).unwrap();
    drain(&mut app);
    assert!(app.editor.has_checkpoint());
    assert!(app.recovery_source.is_none());
    assert!(!offered.checkpoint.exists());
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
fn edits_made_during_read_cannot_be_replaced_by_the_restore_completion() {
    let dir = scratch();
    let offered = orphan(&dir);
    let mut app = recovery_app(dir.clone());
    let release = pause_queue(&mut app);
    app.do_recover(offered.clone());
    app.editor.request(3, Action::AddLayer);
    app.editor.run_pending();
    let key = app.editor.render_key();
    release.send(()).unwrap();
    drain(&mut app);
    assert_eq!(app.editor.render_key(), key);
    assert!(offered.checkpoint.exists());
    assert!(app.recovery_source.is_none());
    assert!(
        app.recoverable
            .iter()
            .any(|source| source.session == offered.session)
    );
    assert!(
        Recovery::claim(&offered).is_ok(),
        "stale decoded data releases the source claim on the worker"
    );
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn ask_later_can_cancel_an_already_decoded_restore_without_deleting_its_source() {
    let dir = scratch();
    let offered = orphan(&dir);
    let mut app = recovery_app(dir.clone());
    let key = app.editor.render_key();
    app.do_recover(offered.clone());
    let (request, result) = take_restore(&mut app);
    assert!(Recovery::discard(&dir, offered.session).is_err());
    app.saving.as_mut().unwrap().cancel_restore();
    app.recoverable.clear();
    app.apply_claimed_recovery(request, result);
    drain(&mut app);
    assert_eq!(app.editor.render_key(), key);
    assert!(app.recoverable.is_empty());
    assert!(offered.checkpoint.exists());
    assert!(Recovery::claim(&offered).is_ok());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn a_restore_waits_for_admission_then_starts_without_another_click() {
    let dir = scratch();
    let offered = orphan(&dir);
    let mut app = recovery_app(dir.clone());
    let (release, hold) = mpsc::channel();
    let saving = app.saving.as_mut().unwrap();
    let ticket = saving
        .io
        .submit(
            1,
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
    app.do_recover(offered.clone());
    assert!(app.saving.as_ref().unwrap().restore_intent.is_some());
    release.send(()).unwrap();
    drain(&mut app);
    assert_eq!(app.editor.document().state_id, offered.state_id);
    assert!(app.editor.has_checkpoint());
    assert!(!offered.checkpoint.exists());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn failed_new_store_preserves_the_claim_and_an_explicit_save_can_finish_the_handoff() {
    let dir = scratch();
    let offered = orphan(&dir);
    let expected = Recovery::read_checkpoint(&offered.checkpoint).unwrap();
    let mut app = recovery_app(dir.clone());
    let blocked = dir.join("not-a-directory");
    std::fs::write(&blocked, b"blocked").unwrap();
    app.recovery_dir = Some(blocked); // inject failure creating the replacement owner
    app.do_recover(offered.clone());
    drain(&mut app);
    assert!(app.editor.document().same_semantics(&expected));
    assert!(app.editor.is_dirty());
    assert!(app.recovery_source.is_some());
    assert!(app.editor.checkpoint_info().is_none());
    assert!(offered.checkpoint.exists());
    assert!(Recovery::discard(&dir, offered.session).is_err());
    let output = dir.join("explicit-save.pcnd");
    app.do_save(&egui::Context::default(), output.clone());
    drain(&mut app);
    assert!(!app.editor.is_dirty());
    assert!(app.recovery_source.is_none());
    assert!(!offered.checkpoint.exists());
    assert!(
        Editor::open(&output)
            .unwrap()
            .document()
            .same_semantics(&expected)
    );
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn deleting_the_source_after_decode_does_not_change_the_frozen_restore() {
    let dir = scratch();
    let offered = orphan(&dir);
    let expected = Recovery::read_checkpoint(&offered.checkpoint).unwrap();
    let mut app = recovery_app(dir.clone());
    app.do_recover(offered.clone());
    let (request, result) = take_restore(&mut app);
    std::fs::remove_file(&offered.checkpoint).unwrap();
    app.apply_claimed_recovery(request, result);
    drain(&mut app);
    assert!(app.editor.document().same_semantics(&expected));
    assert!(app.editor.has_checkpoint());
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
fn user_discard_of_an_orphan_runs_on_the_io_service() {
    let dir = scratch();
    let offered = orphan(&dir);
    let mut app = recovery_app(dir.clone());
    let release = pause_queue(&mut app);
    app.saving
        .as_mut()
        .unwrap()
        .discard_offer(app.recovery_scope, &offered)
        .unwrap();
    assert!(offered.checkpoint.exists());
    assert!(
        app.recoverable
            .iter()
            .any(|source| source.session == offered.session)
    );
    release.send(()).unwrap();
    drain(&mut app);
    assert!(!offered.checkpoint.exists());
    assert!(app.recoverable.is_empty());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_new_save_as_choice_supersedes_a_restore_waiting_for_capacity() {
    let dir = scratch();
    let offered = orphan(&dir);
    let mut app = recovery_app(dir.clone());
    let expected = app.editor.document_snapshot();
    let (release, hold) = mpsc::channel();
    let saving = app.saving.as_mut().unwrap();
    let ticket = saving
        .io
        .submit(
            1,
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
    app.do_recover(offered.clone());
    let output = dir.join("new-save-choice.pcnd");
    app.saving
        .as_mut()
        .unwrap()
        .submit(&app.editor, output.clone(), None, egui::Context::default())
        .unwrap();
    release.send(()).unwrap();
    drain(&mut app);
    assert!(app.editor.document().same_semantics(&expected));
    assert_eq!(app.bound.as_deref(), Some(output.as_path()));
    assert!(!app.saving.as_ref().unwrap().restoring());
    assert!(offered.checkpoint.exists());
    assert!(app.recovery_source.is_none());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn a_changed_source_is_rejected_without_replacing_the_current_document() {
    let dir = scratch();
    let offered = orphan(&dir);
    let mut app = recovery_app(dir.clone());
    let key = app.editor.render_key();
    let release = pause_queue(&mut app);
    app.do_recover(offered.clone());
    let changed = Editor::new_document(6, 9).unwrap();
    std::fs::write(
        &offered.checkpoint,
        paintcraft_formats::pcnd::write(changed.document()).unwrap(),
    )
    .unwrap();
    release.send(()).unwrap();
    drain(&mut app);
    assert_eq!(app.editor.render_key(), key);
    assert!(app.recovery_source.is_none());
    assert!(app.editor.status().contains("changed after it was offered"));
    assert!(offered.checkpoint.exists());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn closing_during_preparation_cancels_the_restore_without_consuming_its_source() {
    let dir = scratch();
    let offered = orphan(&dir);
    let mut app = recovery_app(dir.clone());
    let key = app.editor.render_key();
    let release = pause_queue(&mut app);
    app.do_recover(offered.clone());
    app.close_after_requested_saves(&egui::Context::default());
    release.send(()).unwrap();
    drain(&mut app);
    assert_eq!(app.editor.render_key(), key);
    assert!(offered.checkpoint.exists());
    assert!(app.recovery_source.is_none());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn undo_back_to_the_same_state_does_not_revive_an_old_restore_authorization() {
    let dir = scratch();
    let offered = orphan(&dir);
    let mut app = recovery_app(dir.clone());
    let base = app.editor.render_key();
    let release = pause_queue(&mut app);
    app.do_recover(offered.clone());
    app.editor.request(4, Action::AddLayer);
    app.editor.run_pending();
    app.editor.request(5, Action::Undo);
    app.editor.run_pending();
    let current = app.editor.render_key();
    assert_eq!(base.0, current.0);
    assert_ne!(base.1, current.1);
    release.send(()).unwrap();
    drain(&mut app);
    assert_eq!(app.editor.render_key(), current);
    assert!(offered.checkpoint.exists());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn text_started_during_preparation_is_kept_even_before_it_becomes_a_transaction() {
    let dir = scratch();
    let offered = orphan(&dir);
    let mut app = recovery_app(dir.clone());
    let base = app.editor.render_key();
    let release = pause_queue(&mut app);
    app.do_recover(offered.clone());
    app.editor.tool = Tool::Text;
    app.editor.pointer_down((3, 2));
    app.editor.type_text("keep this draft");
    assert_eq!(app.editor.render_key(), base);
    assert!(app.editor.is_editing_text());
    release.send(()).unwrap();
    drain(&mut app);
    assert_eq!(app.editor.render_key(), base);
    assert!(app.editor.is_editing_text());
    assert!(offered.checkpoint.exists());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn layer_name_and_opacity_drafts_are_not_lost_to_a_prepared_restore() {
    for editing_name in [true, false] {
        let dir = scratch();
        let offered = orphan(&dir);
        let mut app = recovery_app(dir.clone());
        let base = app.editor.render_key();
        let layer = app.editor.target().unwrap();
        let release = pause_queue(&mut app);
        app.do_recover(offered.clone());
        if editing_name {
            app.name_edit = Some((layer, "keep this name draft".into(), Default::default()));
        } else {
            app.opacity_edit = Some((layer, 71));
        }
        assert_eq!(app.editor.render_key(), base);
        release.send(()).unwrap();
        drain(&mut app);
        assert_eq!(app.editor.render_key(), base);
        if editing_name {
            assert_eq!(app.name_edit.as_ref().unwrap().1, "keep this name draft");
        } else {
            assert_eq!(app.opacity_edit, Some((layer, 71)));
        }
        assert!(offered.checkpoint.exists());
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn a_recovered_zero_state_uuid_is_unsaved_and_gets_a_protective_checkpoint() {
    let dir = scratch();
    let owner = Recovery::start(&dir).unwrap();
    let mut document = Editor::new_document(7, 5).unwrap().document().clone();
    document.state_id = Uuid::nil(); // Valid on-disk UUID, not an absence marker.
    owner.checkpoint_document(&document, None).unwrap();
    drop(owner);
    let offered = Recovery::scan(&dir).into_iter().next().unwrap();
    let mut app = recovery_app(dir.clone());
    app.do_recover(offered.clone());
    drain(&mut app);
    assert_eq!(app.editor.document().state_id, Uuid::nil());
    assert!(
        app.editor.is_dirty(),
        "recovered work has no native save regardless of UUID value"
    );
    assert!(app.editor.has_checkpoint());
    assert!(
        !offered.checkpoint.exists(),
        "handoff retires the old source only after protection"
    );
    drop(app);
    let found = Recovery::scan(&dir);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].state_id, Uuid::nil());
    std::fs::remove_dir_all(dir).unwrap();
}
