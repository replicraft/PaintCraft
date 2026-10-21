use super::*;
use paintcraft_core::types::resources::SharedBudget;
use std::time::{Duration, Instant};
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "dispatch stalled");
        std::thread::yield_now();
    }
}
fn setup() -> (Editor, background::Service, SharedBudget) {
    let budget = SharedBudget::new(3 << 30);
    let service =
        background::Service::with_budget(eframe::egui::Context::default(), budget.clone()).unwrap();
    let mut editor = Editor::new_document(8, 8).unwrap();
    editor.connect_editing(service.clone());
    (editor, service, budget)
}
#[test]
fn first_submission_race_retains_alias_publication_and_fifo_file_boundary() {
    let (mut editor, service, budget) = setup();
    let original = editor.document_snapshot();
    let target = editor.target();
    assert!(service.available());
    let blocker = budget.reserve(2 << 30).unwrap(); // Admission changes after the readiness hint.
    editor.add_layer(); // The actual initial batch dispatch encounters the race.
    assert!(editor.waiting_dispatch.is_some());
    let boundary = editor.queue_file_barrier();
    editor.request(
        1,
        Action::ClearCanvas {
            color: Rgba([17, 88, 156, 255]),
        },
    );
    for _ in 0..4 {
        editor.run_pending();
    }
    assert!(editor.is_dirty());
    assert!(!editor.can_capture_file());
    assert!(Arc::ptr_eq(&original, &editor.document_snapshot()));
    assert_eq!(editor.target(), target);
    assert_eq!(budget.usage().reserved, 2 << 30);
    assert!(!editor.start_residency_maintenance()); // Do not invalidate the captured preparation.
    drop(blocker);
    until(|| {
        editor.run_pending();
        editor.file_barrier_ready(boundary)
    });
    assert_eq!(editor.publications(), 1);
    assert_eq!(editor.document().layers.len(), 2);
    assert_ne!(editor.target(), target);
    editor.release_file_barrier(boundary);
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.publications(), 2);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn tool_plan_waits_without_losing_its_captured_preview_or_retargeting() {
    let (mut editor, service, budget) = setup();
    let target = editor.target().unwrap();
    editor.pointer_down((1, 1));
    editor.pointer_move((6, 6));
    let preview = editor.preview().unwrap();
    editor.cancel_gesture();
    assert!(service.available());
    let blocker = budget.reserve(2 << 30).unwrap();
    editor.publication_preview = Some(preview.clone());
    editor.start_tool_plan(preview.clone(), "Stroke");
    assert!(editor.waiting_dispatch.is_some());
    assert_eq!(editor.preview(), Some(preview));
    assert!(editor.has_pending_edits());
    drop(blocker);
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.publications(), 1);
    let raster = &editor.document().layers[&target].raster;
    for x in 1..=6 {
        assert_eq!(raster.get(x, x), Rgba([0, 0, 0, 255]));
    }
    assert!(editor.preview().is_none());
}
#[test]
fn escape_and_controller_drop_release_an_unsubmitted_task_without_executing_it() {
    let (mut editor, _, budget) = setup();
    let blocker = budget.reserve(2 << 30).unwrap();
    let original = editor.document_snapshot();
    editor.clear_canvas(Rgba([0, 0, 0, 255]));
    assert!(editor.waiting_dispatch.is_some());
    let boundary = editor.queue_file_barrier();
    editor.request(1, Action::AddLayer);
    editor.cancel_pending_edits();
    assert!(editor.waiting_dispatch.is_none());
    assert_eq!(editor.status(), "Edit cancelled");
    editor.run_pending();
    assert!(editor.file_barrier_ready(boundary));
    assert!(Arc::ptr_eq(&original, &editor.document_snapshot()));
    drop(blocker);
    editor.release_file_barrier(boundary);
    for _ in 0..4 {
        editor.run_pending();
    }
    assert_eq!(editor.publications(), 0);
    let blocker = budget.reserve(2 << 30).unwrap();
    editor.add_layer();
    assert!(editor.waiting_dispatch.is_some());
    drop(editor);
    assert_eq!(budget.usage().reserved, 2 << 30);
    drop(blocker);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn waiting_navigation_stays_before_later_commands_and_never_runs_again_after_cancel() {
    let (mut editor, service, budget) = setup();
    editor.add_layer();
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.document().layers.len(), 2);
    assert!(service.available());
    let blocker = budget.reserve(2 << 30).unwrap();
    assert!(editor.start_navigation(background::NavigationRequest::Undo));
    assert!(editor.waiting_dispatch.is_some());
    let boundary = editor.queue_file_barrier();
    drop(blocker);
    until(|| {
        editor.run_pending();
        editor.file_barrier_ready(boundary)
    });
    assert_eq!(editor.document().layers.len(), 1);
    assert_eq!(editor.redo_depth(), 1);
    editor.release_file_barrier(boundary);
    let blocker = budget.reserve(2 << 30).unwrap();
    editor.start_navigation(background::NavigationRequest::Redo);
    assert!(editor.waiting_dispatch.is_some());
    editor.cancel_pending_edits();
    drop(blocker);
    for _ in 0..4 {
        editor.run_pending();
    }
    assert_eq!(editor.document().layers.len(), 1);
    assert_eq!(editor.redo_depth(), 1);
}
#[test]
fn competing_publication_drops_the_captured_task_instead_of_rebasing_it() {
    let (mut editor, _, budget) = setup();
    let blocker = budget.reserve(2 << 30).unwrap();
    editor.add_layer();
    assert!(editor.waiting_dispatch.is_some());
    let service = editor.editing.take();
    editor.clear_canvas(Rgba([255, 0, 0, 255]));
    editor.editing = service;
    let newer = editor.document_snapshot();
    drop(blocker);
    editor.run_pending();
    assert!(editor.waiting_dispatch.is_none());
    assert!(!editor.has_pending_edits());
    assert!(Arc::ptr_eq(&newer, &editor.document_snapshot()));
    assert_eq!(editor.document().layers.len(), 1);
    assert!(editor.status().contains("base changed"));
    assert_eq!(budget.usage().reserved, 0);
}
