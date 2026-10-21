use super::*;
use background::NavigationRequest;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_raster::RasterLayer;
use std::time::{Duration, Instant};
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "navigation stalled");
        std::thread::yield_now();
    }
}
fn finish(editor: &mut Editor) {
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
}
fn wait(editor: &mut Editor) {
    until(|| {
        editor.run_pending();
        editor.allocation_wait().is_some()
    });
}
fn fixture() -> (Editor, SharedBudget, Option<Uuid>) {
    let budget = SharedBudget::new(4 * BYTES);
    let service =
        background::Service::with_residency(eframe::egui::Context::default(), budget.clone())
            .unwrap();
    let mut editor = Editor::new_document(257, 1).unwrap();
    editor.connect_editing(service);
    editor.pointer_down((0, 0));
    editor.pointer_move((256, 0));
    editor.pointer_up();
    finish(&mut editor);
    let target = editor.history_view().entries.last().unwrap().history_id;
    for (x, color) in [(0, Rgba([255, 0, 0, 255])), (256, Rgba([0, 0, 255, 255]))] {
        editor.color = color;
        editor.pointer_down((x, 0));
        editor.pointer_up();
        finish(&mut editor);
    }
    assert_eq!(budget.usage().reserved, 4 * BYTES);
    editor
        .session
        .compact_history(1, 4 << 20, || Ok(()))
        .unwrap();
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    editor.acknowledge_saved_state(editor.document().state_id);
    (editor, budget, target)
}
fn seek(editor: &mut Editor, target: Option<Uuid>) {
    editor.start_navigation(NavigationRequest::Seek {
        ticket: editor.session.history_ticket(),
        history_id: target,
    });
}
#[test]
fn cold_history_seek_waits_before_save_then_restores_recorded_pixels_once() {
    let (mut editor, budget, target) = fixture();
    let before = editor.document_snapshot();
    let view = editor.history_view();
    let blocker = budget.reserve(BYTES).unwrap();
    seek(&mut editor, target);
    let boundary = editor.queue_file_barrier();
    editor.request(
        1,
        Action::ClearCanvas {
            color: Rgba([17, 88, 156, 255]),
        },
    );
    wait(&mut editor);
    assert_eq!(
        editor.allocation_wait().unwrap().required_bytes(),
        2 * BYTES
    );
    assert!(Arc::ptr_eq(&before, &editor.document_snapshot()));
    assert_eq!(editor.history_view(), view);
    assert!(!editor.can_capture_file());
    let usage = budget.usage();
    for _ in 0..4 {
        editor.run_pending();
    }
    assert_eq!(budget.usage(), usage);
    drop(blocker);
    until(|| {
        editor.run_pending();
        editor.file_barrier_ready(boundary)
    });
    assert_eq!(editor.publications(), 4);
    assert_eq!(editor.undo_depth(), 1);
    assert_eq!(editor.redo_depth(), 2);
    let raster = &editor.document().layers[&editor.target().unwrap()].raster;
    for x in 0..257 {
        assert_eq!(raster.get(x, 0), Rgba([0, 0, 0, 255]));
    }
    editor.release_file_barrier(boundary);
    finish(&mut editor);
    assert_eq!(editor.publications(), 5);
}
#[test]
fn escape_discards_a_waiting_seek_and_later_edits_but_keeps_its_file_boundary() {
    let (mut editor, budget, target) = fixture();
    let before = editor.document_snapshot();
    let view = editor.history_view();
    let blocker = budget.reserve(BYTES).unwrap();
    seek(&mut editor, target);
    let boundary = editor.queue_file_barrier();
    editor.request(1, Action::AddLayer);
    wait(&mut editor);
    editor.cancel_pending_edits();
    assert!(editor.allocation_wait().is_none());
    editor.run_pending();
    assert!(editor.file_barrier_ready(boundary));
    drop(blocker);
    editor.release_file_barrier(boundary);
    finish(&mut editor);
    assert!(Arc::ptr_eq(&before, &editor.document_snapshot()));
    assert_eq!(editor.history_view(), view);
    assert_eq!(editor.publications(), 3);
    assert!(!editor.is_dirty());
    assert_eq!(budget.usage().reserved, 2 * BYTES);
}
#[test]
fn stale_waiting_navigation_does_not_seek_into_a_competing_timeline() {
    let (mut editor, budget, target) = fixture();
    let blocker = budget.reserve(BYTES).unwrap();
    seek(&mut editor, target);
    wait(&mut editor);
    let service = editor.editing.take();
    editor.clear_canvas(Rgba([17, 88, 156, 255]));
    editor.editing = service;
    let newer = editor.document_snapshot();
    let view = editor.history_view();
    drop(blocker);
    finish(&mut editor);
    assert!(Arc::ptr_eq(&newer, &editor.document_snapshot()));
    assert_eq!(editor.history_view(), view);
    assert!(editor.status().contains("base changed"));
}
#[test]
fn cold_undo_and_redo_both_retry_without_replaying_commands_or_losing_redo() {
    let (mut editor, budget, _) = fixture();
    let blocker = budget.reserve(2 * BYTES).unwrap();
    editor.start_navigation(NavigationRequest::Undo);
    wait(&mut editor);
    assert_eq!(editor.allocation_wait().unwrap().required_bytes(), BYTES);
    drop(blocker);
    finish(&mut editor);
    assert_eq!(editor.publications(), 4);
    assert_eq!(editor.undo_depth(), 2);
    assert_eq!(editor.redo_depth(), 1);
    seek(&mut editor, None);
    finish(&mut editor);
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    let blocker = budget.reserve(BYTES).unwrap();
    editor.start_navigation(NavigationRequest::Redo);
    wait(&mut editor);
    assert_eq!(
        editor.allocation_wait().unwrap().required_bytes(),
        2 * BYTES
    );
    drop(blocker);
    finish(&mut editor);
    assert_eq!(editor.publications(), 6);
    assert_eq!(editor.undo_depth(), 1);
    assert_eq!(editor.redo_depth(), 2);
    let raster = &editor.document().layers[&editor.target().unwrap()].raster;
    for x in 0..257 {
        assert_eq!(raster.get(x, 0), Rgba([0, 0, 0, 255]));
    }
}
#[test]
fn impossible_decoded_and_fixed_restore_limits_fail_without_registering_a_wait() {
    for fixed in [false, true] {
        let (mut editor, _, target) = fixture();
        if fixed {
            editor.session.set_history_restore_budget(65_536);
        } else {
            editor.connect_editing(
                background::Service::with_residency(
                    eframe::egui::Context::default(),
                    SharedBudget::new(BYTES),
                )
                .unwrap(),
            );
        }
        let before = editor.document_snapshot();
        let view = editor.history_view();
        seek(&mut editor, target);
        finish(&mut editor);
        assert!(editor.allocation_wait().is_none());
        assert!(editor.status().contains("resource.limit"));
        assert!(Arc::ptr_eq(&before, &editor.document_snapshot()));
        assert_eq!(editor.history_view(), view);
    }
}
