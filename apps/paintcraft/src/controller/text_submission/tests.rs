use super::*;
use std::time::{Duration, Instant};
fn reject(_: &Document, _: &dyn Fn() -> Result<()>) -> Result<()> {
    Err(Error::new(
        ErrorCode::ResourceLimit,
        "persistent metadata test refusal",
    ))
}
fn text(editor: &mut Editor) {
    editor.select_tool(Tool::Text);
    editor.pointer_down((2, 2));
    editor.pointer_up();
    editor.type_text("Ink");
}
fn editor() -> Editor {
    let mut editor = Editor::new_document(128, 64).unwrap();
    text(&mut editor);
    editor
}
fn settled(editor: &mut Editor) {
    editor.run_pending();
    let until = Instant::now() + Duration::from_secs(10);
    while editor.has_processing_edits() {
        editor.run_pending();
        assert!(Instant::now() < until, "{}", editor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn sync_failure_retains_text_and_fifo_undo_until_explicit_cancel() {
    let mut e = editor();
    e.test_persistent_validator(Some(reject));
    let before = e.document_snapshot();
    let history = e.history_view();
    let stamp = e.tools.text_stamp().unwrap();
    e.request(1001, Action::Undo);
    e.run_pending();
    assert!(e.failed_text_retained());
    assert!(
        !e.session.is_dirty(),
        "refusal never changes the Session save acknowledgement"
    );
    assert!(
        e.is_dirty(),
        "retained authored intention remains unsaved aggregate work"
    );
    assert!(e.tools.text_stamp_current(stamp));
    assert_eq!(e.tools.text_content(), Some("Ink"));
    assert_eq!(e.history_view(), history);
    assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
    assert_eq!(e.actions.len(), 1);
    let attempt = e.text_submission.placement.unwrap().attempt;
    for _ in 0..8 {
        e.commit_text();
        e.run_pending();
    }
    assert_eq!(e.text_submission.placement.unwrap().attempt, attempt);
    assert_eq!(e.history_view(), history);
    assert!(!e.text_save_ready());
    assert!(e.can_capture_export());
    assert!(!e.export_snapshot().unwrap().same_semantics(&before));
    e.cancel_text();
    e.run_pending();
    assert!(e.actions.is_empty());
    assert!(e.tools.text_content().is_none());
    assert_eq!(e.history_view(), history);
    e.test_persistent_validator(None);
    e.run_pending();
    assert_eq!(e.history_view(), history, "no latent Undo");
}

#[test]
fn explicit_retry_is_fresh_but_queued_and_active_notifications_coalesce() {
    let mut e = editor();
    e.test_persistent_validator(Some(reject));
    e.place_text();
    let first = e.text_submission.placement.unwrap().attempt;
    for _ in 0..4 {
        e.place_text();
        e.commit_text();
    }
    assert_eq!(e.text_submission.placement.unwrap().attempt, first);
    e.run_pending();
    assert!(e.failed_text_retained());
    e.test_persistent_validator(None);
    e.place_text();
    let second = e.text_submission.placement.unwrap().attempt;
    assert_ne!(first.token, second.token);
    assert_eq!(first.stamp, second.stamp);
    e.run_pending();
    assert!(e.tools.text_content().is_none());
    assert_eq!(e.undo_depth(), 1);
    e.session.undo().unwrap().unwrap();
    e.session.redo().unwrap().unwrap();
    assert_eq!(e.undo_depth(), 1);
}

#[test]
fn failed_draft_blocks_pointer_replacement_and_keeps_captured_style_location() {
    let mut e = editor();
    e.test_persistent_validator(Some(reject));
    let stamp = e.tools.text_stamp().unwrap();
    let intention = e.tools.pending_text_action().unwrap();
    e.commit_text();
    e.run_pending();
    e.pointer_down((60, 30));
    e.pointer_up();
    e.select_tool(Tool::Brush);
    e.color = Rgba([255, 0, 0, 255]);
    e.pointer_down((60, 30));
    e.pointer_move((70, 35));
    e.pointer_up();
    assert!(e.tools.text_stamp_current(stamp));
    assert_eq!(e.tools.pending_text_action(), Some(intention));
    assert!(!e.tools.gesture_in_progress());
}

#[test]
fn async_rejection_retains_draft_and_old_worker_must_retire_before_export() {
    let service = background::Service::new(eframe::egui::Context::default()).unwrap();
    let (prepared, release) = service.pause_next_completion();
    let mut e = editor();
    e.connect_editing(service.clone());
    e.test_persistent_validator(Some(reject));
    let before = e.document_snapshot();
    e.place_text();
    e.run_pending();
    prepared.recv_timeout(Duration::from_secs(10)).unwrap();
    let attempt = e.text_submission.placement.unwrap().attempt;
    e.place_text();
    assert_eq!(e.text_submission.placement.unwrap().attempt, attempt);
    assert!(!e.can_capture_export());
    e.type_text("!"); // Revokes the exact older intention before owner publication.
    assert!(e.failed_text_retained());
    assert!(!e.can_capture_export());
    assert!(e.export_snapshot().is_err());
    release.send(()).unwrap();
    settled(&mut e);
    assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
    assert_eq!(e.tools.text_content(), Some("Ink!"));
    assert_eq!(e.undo_depth(), 0);
    assert!(e.can_capture_export());
    assert!(!e.export_snapshot().unwrap().same_semantics(&before));
    assert!(service.available());
    e.request(
        8821,
        Action::Stroke(paintcraft_tools::Stroke {
            layer: e.document().roots[0],
            points: vec![(50, 20)],
            point_scale: 1,
            color: Rgba([255, 0, 0, 255]),
            size: 1,
            round: false,
            media: Default::default(),
            mode: paintcraft_core::types::policy::BlendMode::Replace,
        }),
    );
    e.run_pending();
    assert!(
        matches!(e.preview(), Some(Preview::Text { .. })),
        "blocked stroke must not replace retained text preview"
    );
    assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
}

#[test]
fn newer_text_rejects_an_already_prepared_success_and_retries_without_double_ink() {
    let service = background::Service::new(eframe::egui::Context::default()).unwrap();
    let (prepared, release) = service.pause_next_completion();
    let mut e = editor();
    e.connect_editing(service);
    let before = e.document_snapshot();
    e.place_text();
    e.run_pending();
    prepared.recv_timeout(Duration::from_secs(10)).unwrap();
    e.backspace();
    e.type_text("k!");
    release.send(()).unwrap();
    settled(&mut e);
    assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
    assert_eq!(e.tools.text_content(), Some("Ink!"));
    let expected = e.export_snapshot().unwrap();
    e.place_text();
    settled(&mut e);
    assert!(e.document().same_semantics(&expected));
    assert!(e.tools.text_content().is_none());
    assert_eq!(e.undo_depth(), 1);
}

#[test]
fn capture_failure_stays_after_prior_fifo_work_and_empty_close_is_local() {
    let mut e = Editor::new_document(128, 64).unwrap();
    e.request(555, Action::AddLayer);
    e.tools.text_style = Some(paintcraft_core::types::text::TextOptions {
        points: 0,
        ..Default::default()
    });
    text(&mut e);
    e.commit_text();
    e.request(556, Action::Undo);
    e.run_pending();
    assert!(e.failed_text_retained());
    assert_eq!(
        e.undo_depth(),
        1,
        "prior AddLayer must finish before failure boundary"
    );
    assert_eq!(e.document().layers.len(), 2);
    e.cancel_text();
    e.run_pending();
    assert_eq!(e.undo_depth(), 1, "later Undo was discarded");
    e.tools.text_style = None;
    e.pointer_down((1, 1));
    e.pointer_up();
    assert_eq!(e.tools.text_content(), Some(""));
    e.place_text();
    assert!(e.tools.text_content().is_none());
    assert_eq!(e.undo_depth(), 1);
}

#[test]
fn async_fully_clipped_noop_acknowledges_draft_without_history() {
    let service = background::Service::new(eframe::egui::Context::default()).unwrap();
    let mut e = Editor::new_document(8, 8).unwrap();
    e.select_tool(Tool::Text);
    e.pointer_down((100, 100));
    e.pointer_up();
    e.type_text("Outside");
    e.connect_editing(service);
    let before = e.document_snapshot();
    e.place_text();
    settled(&mut e);
    assert!(e.tools.text_content().is_none());
    assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
    assert_eq!(e.undo_depth(), 0);
}

#[test]
fn successful_click_continuation_is_revoked_by_an_intervening_tool_change() {
    for change_tool in [false, true] {
        let service = background::Service::new(eframe::egui::Context::default()).unwrap();
        let (prepared, release) = service.pause_next_completion();
        let mut e = editor();
        e.connect_editing(service);
        e.pointer_down((40, 20));
        e.pointer_up();
        e.run_pending();
        prepared.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(e.text_submission.next_text.is_some());
        if change_tool {
            e.select_tool(Tool::Brush);
            e.select_tool(Tool::Text);
        }
        assert_eq!(e.text_submission.next_text.is_none(), change_tool);
        release.send(()).unwrap();
        settled(&mut e);
        if change_tool {
            assert!(e.tools.text_content().is_none());
        } else {
            assert_eq!(e.tools.text_content(), Some(""));
            e.type_text("Next");
            assert!(matches!(
                e.tools.pending_text_action(),
                Some(Action::Text { at: (40, 20), .. })
            ));
        }
        assert_eq!(e.undo_depth(), 1);
    }
}

#[test]
fn an_earlier_file_barrier_can_save_its_prefix_before_a_later_capture_failure() {
    let mut e = Editor::new_document(128, 64).unwrap();
    let before = e.document_snapshot();
    let epoch = e.text_file_epoch();
    let barrier = e.queue_file_barrier();
    e.tools.text_style = Some(paintcraft_core::types::text::TextOptions {
        points: 0,
        ..Default::default()
    });
    text(&mut e);
    e.commit_text();
    e.run_pending();
    assert!(e.file_barrier_ready(barrier));
    assert_eq!(e.text_file_epoch(), epoch);
    assert!(!e.failed_text_retained());
    assert!(e.text_save_ready());
    let path = std::env::temp_dir().join(format!(
        "paintcraft-retained-text-prefix-{}.pcnd",
        Uuid::new_v4()
    ));
    e.set_path(path.clone());
    e.save_captured();
    assert_eq!(e.take_save_outcome(), Some(true));
    assert!(
        paintcraft_formats::pcnd::read(&std::fs::read(&path).unwrap())
            .unwrap()
            .same_semantics(&before)
    );
    std::fs::remove_file(&path).unwrap();
    assert_eq!(e.tools.text_content(), Some("Ink"));
    e.release_file_barrier(barrier);
    e.run_pending();
    assert!(e.failed_text_retained());
    assert_ne!(e.text_file_epoch(), epoch);
    assert!(!e.text_save_ready());
    assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
}

#[test]
fn a_new_queued_revision_keeps_its_fifo_position_after_an_earlier_save_barrier() {
    for aba in [false, true] {
        let mut e = editor();
        e.test_persistent_validator(Some(reject));
        let before = e.document_snapshot();
        let epoch = e.text_file_epoch();
        let barrier = e.queue_file_barrier();
        e.commit_text();
        let first = e.text_submission.placement.unwrap().attempt;
        e.request(6001, Action::Undo);
        if aba {
            e.backspace();
            e.type_text("k");
        } else {
            e.type_text("!");
        }
        e.commit_text();
        let newer = e.text_submission.placement.unwrap().attempt;
        assert_ne!(newer.stamp, first.stamp);
        assert_ne!(newer.token, first.token);
        e.run_pending();
        assert!(e.file_barrier_ready(barrier));
        assert!(e.text_save_ready());
        assert_eq!(e.text_file_epoch(), epoch);
        assert_eq!(e.history_view().entries.len(), 1);
        e.release_file_barrier(barrier);
        e.run_pending();
        assert!(e.failed_text_retained());
        assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
        assert_eq!(e.undo_depth(), 0, "later Undo must remain blocked");
        assert_eq!(
            e.tools.text_content(),
            Some(if aba { "Ink" } else { "Ink!" })
        );
        e.cancel_text();
        e.run_pending();
        assert!(e.actions.is_empty());
    }
}

#[test]
fn cancelling_queued_text_discards_only_its_suffix_and_preserves_earlier_file_barrier() {
    let mut e = editor();
    let before = e.document_snapshot();
    let epoch = e.text_file_epoch();
    let earlier = e.queue_file_barrier();
    e.commit_text();
    e.request(7001, Action::Undo);
    let dependent = e.queue_file_barrier();
    e.cancel_text();
    e.run_pending();
    assert!(e.file_barrier_ready(earlier));
    assert!(e.file_barrier_live(earlier));
    assert!(!e.file_barrier_live(dependent));
    assert_eq!(e.text_file_epoch(), epoch);
    assert!(e.text_save_ready());
    e.release_file_barrier(earlier);
    e.run_pending();
    assert!(!e.has_pending_edits());
    assert!(e.actions.is_empty());
    assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
    assert_eq!(e.undo_depth(), 0);
}

#[test]
fn normal_native_factory_automatically_refuses_unsavable_text_without_test_policy_injection() {
    let document = crate::editing::text_submission_tests::boundary();
    let mut e = Editor::with_document(document, None);
    let before = e.document_snapshot();
    let history = e.history_view();
    let metadata = before.source_metadata.as_ref().unwrap().digest();
    text(&mut e);
    let stamp = e.tools.text_stamp().unwrap();
    e.place_text();
    e.run_pending();
    assert!(
        e.failed_text_retained(),
        "the normal factory must install persistent admission"
    );
    assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
    assert_eq!(e.history_view(), history);
    assert_eq!(
        e.document().source_metadata.as_ref().unwrap().digest(),
        metadata
    );
    assert!(e.tools.text_stamp_current(stamp));
    assert_eq!(e.tools.text_content(), Some("Ink"));
    assert!(!e.session.is_dirty());
    assert!(e.is_dirty());
    assert!(e.can_capture_export());
    assert!(!e.export_snapshot().unwrap().same_semantics(&before));
}
