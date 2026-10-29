use super::super::tests::{app, drain, pause_queue, scratch};
use super::*;
use paintcraft_core::types::resources::SharedBudget;
use std::fs;
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        Self(scratch())
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn orphan(dir: &Path) -> Found {
    let document=paintcraft_formats::pcnd::read(include_bytes!("../../../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd")).unwrap();
    let recovery = Recovery::start(dir).unwrap();
    recovery.checkpoint_document(&document, None).unwrap();
    let id = recovery.session();
    drop(recovery);
    Recovery::scan(dir)
        .into_iter()
        .find(|f| f.session == id)
        .unwrap()
}
fn take_restore(app: &mut App) -> (Request, Result<Completion>) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some((_, kind, result)) = app.saving.as_mut().unwrap().poll_completion() {
            if let PendingKind::Restore(request) = kind {
                return (*request, result);
            }
            panic!("Expected the sole restore job");
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn discovery_denial_preserves_offers_and_current_autosave_without_busy_retry() {
    let folder = Folder::new();
    let offer = orphan(&folder.0);
    let payload = fs::read(&offer.checkpoint).unwrap();
    let budget = SharedBudget::new(0);
    let mut app = app();
    app.source_metadata_budget = budget.clone();
    app.recoverable.push(offer.clone());
    let before = app.editor.document_snapshot();
    let history = app.editor.history_view().ticket;
    let mut app = app.with_recovery(folder.0.clone());
    drain(&mut app);
    assert!(app.editor.recovery_handle().is_some());
    assert!(app.editor.document().same_semantics(&before));
    assert_eq!(app.editor.history_view().ticket, history);
    assert_eq!(app.recoverable, vec![offer.clone()]);
    assert!(app.editor.status().contains("resource limit"));
    assert!(app.recovery_needs_scan);
    for _ in 0..10 {
        app.poll_native_saves(&egui::Context::default());
        assert!(!app.saving.as_ref().unwrap().pending());
    }
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    assert!(
        app.saving
            .as_mut()
            .unwrap()
            .checkpoint(&app.editor)
            .unwrap()
    );
    drain(&mut app);
    assert!(app.editor.has_checkpoint());
    assert_eq!(fs::read(&offer.checkpoint).unwrap(), payload);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn pending_claims_share_host_realm_and_cancel_or_stale_release_without_adoption() {
    for mode in 0..4 {
        let folder = Folder::new();
        let offer = orphan(&folder.0);
        let bytes = fs::read(&offer.checkpoint).unwrap();
        let budget = SharedBudget::new(if mode == 0 { 0 } else { 1 << 20 });
        let mut app = app();
        app.source_metadata_budget = budget.clone();
        let mut app = app.with_recovery(folder.0.clone());
        drain(&mut app);
        app.recoverable = vec![offer.clone()];
        let before = app.editor.document_snapshot();
        let history = app.editor.history_view().ticket;
        app.do_recover(offer.clone());
        let (request, result) = take_restore(&mut app);
        if mode == 0 {
            assert_eq!(result.as_ref().unwrap_err().code, ErrorCode::ResourceLimit);
        } else {
            assert!(budget.usage().reserved > 0);
        }
        if mode == 1 {
            app.saving.as_mut().unwrap().cancel_restore();
        }
        if mode == 2 {
            app.editor.request(2, Action::AddLayer);
            app.editor.run_pending();
        }
        let current = app.editor.document_snapshot();
        let current_history = app.editor.history_view().ticket;
        app.apply_claimed_recovery(request, result);
        drain(&mut app);
        if mode == 3 {
            assert!(app.editor.document().source_metadata.is_some());
            assert!(budget.usage().reserved > 0);
            assert!(app.editor.is_dirty());
        } else {
            assert!(app.editor.document().same_semantics(&current));
            assert_eq!(app.editor.history_view().ticket, current_history);
            assert_eq!(budget.usage().reserved, 0);
            assert_eq!(app.recoverable, vec![offer.clone()]);
            assert_eq!(fs::read(&offer.checkpoint).unwrap(), bytes);
            if mode != 2 {
                assert!(app.editor.document().same_semantics(&before));
                assert_eq!(app.editor.history_view().ticket, history);
            }
            assert!(Recovery::claim(&offer).is_ok());
        }
        drop(app);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn cancelled_queued_initialization_does_not_scan_or_create_an_old_owner_store() {
    let folder = Folder::new();
    let offer = orphan(&folder.0);
    let mut app = app();
    let release = pause_queue(&mut app);
    app = app.with_recovery(folder.0.clone());
    app.saving.as_mut().unwrap().new_session();
    release.send(()).unwrap();
    drain(&mut app);
    assert!(app.editor.recovery_handle().is_none());
    assert_eq!(
        fs::read_dir(&folder.0)
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|e| e == "lock"))
            .count(),
        1
    );
    assert!(offer.checkpoint.exists());
}

#[test]
fn preexisting_private_text_survives_async_denial_cancel_stale_and_capacity_failure() {
    for mode in 0..4 {
        let folder = Folder::new();
        let offer = orphan(&folder.0);
        let payload = fs::read(&offer.checkpoint).unwrap();
        let budget = SharedBudget::new(if mode == 0 { 0 } else { 1 << 20 });
        let mut app = app();
        app.settings.experience = Experience::Studio;
        app.source_metadata_budget = budget.clone();
        let mut app = app.with_recovery(folder.0.clone());
        drain(&mut app);
        app.recoverable = vec![offer.clone()];
        app.editor.tool = Tool::Text;
        app.editor.pointer_down((3, 2));
        app.editor.type_text("keep this private draft");
        let base = app.editor.document_snapshot();
        let history = app.editor.history_view().ticket;
        app.do_recover(offer.clone());
        assert!(
            app.editor.is_editing_text(),
            "preparation does not finish text"
        );
        let (request, result) = take_restore(&mut app);
        match mode {
            0 => assert_eq!(result.as_ref().unwrap_err().code, ErrorCode::ResourceLimit),
            1 => app.saving.as_mut().unwrap().cancel_restore(),
            2 => {
                app.editor.type_text(" changed");
            }
            3 => app.next_tab_number = u64::MAX,
            _ => unreachable!(),
        };
        let text = app.editor.canvas_text_content().unwrap().to_owned();
        let stamp = app.editor.preview_stamp();
        app.apply_claimed_recovery(request, result);
        drain(&mut app);
        assert!(app.editor.is_editing_text());
        assert_eq!(app.editor.canvas_text_content(), Some(text.as_str()));
        assert_eq!(app.editor.preview_stamp(), stamp);
        assert!(app.editor.document().same_semantics(&base));
        assert_eq!(app.editor.history_view().ticket, history);
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(app.recoverable, vec![offer.clone()]);
        assert_eq!(fs::read(&offer.checkpoint).unwrap(), payload);
        assert!(Recovery::claim(&offer).is_ok());
    }
}

#[test]
fn successful_studio_recovery_finalizes_old_text_and_cancels_old_gesture_only_at_adoption() {
    for text in [false, true] {
        let folder = Folder::new();
        let offer = orphan(&folder.0);
        let mut app = app();
        app.settings.experience = Experience::Studio;
        let mut app = app.with_recovery(folder.0.clone());
        drain(&mut app);
        let original = app.active_tab_id();
        let base = app.editor.document_snapshot();
        let depth = app.editor.undo_depth();
        app.editor.tool = if text { Tool::Text } else { Tool::Brush };
        app.editor.pointer_down((3, 2));
        if text {
            app.editor.type_text("committed only on success");
        } else {
            app.editor.pointer_move((7, 2));
        }
        let stamp = app.editor.preview_stamp();
        app.do_recover(offer.clone());
        assert_eq!(app.editor.preview_stamp(), stamp);
        let (request, result) = take_restore(&mut app);
        assert!(result.is_ok());
        assert_eq!(app.editor.preview_stamp(), stamp);
        app.apply_claimed_recovery(request, result);
        drain(&mut app);
        assert_ne!(app.active_tab_id(), original);
        assert!(app.editor.document().source_metadata.is_some());
        assert!(app.swap_tab_state(original));
        assert!(!app.editor.is_editing_text());
        assert!(!app.editor.gesture_in_progress());
        if text {
            assert_eq!(app.editor.undo_depth(), depth + 1);
            assert!(!app.editor.document().same_semantics(&base));
            app.editor.request(99, Action::Undo);
            app.editor.run_pending();
        } else {
            assert_eq!(app.editor.undo_depth(), depth);
        }
        assert!(app.editor.document().same_semantics(&base));
    }
}
