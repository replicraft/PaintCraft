use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_raster::RasterLayer;
use std::time::{Duration, Instant};
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
#[test]
fn metadata_denial_does_not_enter_decoded_history_pressure_retry() {
    let (mut editor, _, _) = setup(2);
    let base = editor.session.preparation().unwrap().history_ticket();
    let budget = SharedBudget::new(1);
    let blocker = budget.reserve(1).unwrap();
    let mut account = budget.account(AllocationClass::PaintMetadata);
    let error = account.resize(1).unwrap_err();
    let result = defer_failure(
        base,
        Request::Navigation(background::NavigationRequest::Undo),
        Err(error),
        Realms::capture(&Admission::new(0).with_paint_metadata_budget(budget.clone())),
        || Ok(()),
    );
    let Completion::Deferred(deferred) = result.unwrap() else {
        panic!("metadata request must wait");
    };
    assert_eq!(deferred.denial.class(), AllocationClass::PaintMetadata);
    editor.waiting_edit = Some(Waiting {
        text: None,
        filter: None,
        native_activation: None,
        deferred: *deferred,
        done: String::new(),
        empty: "",
        last_probe: Instant::now(),
        target_alias: None,
    });
    assert!(editor.allocation_wait().is_none());
    assert!(editor.metadata_wait().is_some());
    drop(blocker);
}
pub(super) fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "edit retry stalled");
        std::thread::yield_now();
    }
}
pub(super) fn setup(tiles: u64) -> (Editor, background::Service, SharedBudget) {
    let budget = SharedBudget::new(tiles * BYTES);
    let mut service = background::Service::new(eframe::egui::Context::default()).unwrap();
    service.residency = budget.clone();
    let mut editor = Editor::new_document(257, 1).unwrap();
    editor.connect_editing(service.clone());
    (editor, service, budget)
}
pub(super) fn stroke(editor: &mut Editor) {
    editor.pointer_down((0, 0));
    editor.pointer_move((256, 0));
    editor.pointer_up();
}
fn wait_for_capacity(editor: &mut Editor) {
    until(|| {
        editor.run_pending();
        editor.allocation_wait().is_some()
    });
}
#[test]
fn blocked_stroke_keeps_preview_and_fifo_save_boundary_then_publishes_once() {
    let (mut editor, _, budget) = setup(2);
    let blocker = budget.reserve(BYTES).unwrap();
    let original = editor.document_snapshot();
    stroke(&mut editor);
    let preview = editor.preview();
    editor.run_pending();
    let boundary = editor.queue_file_barrier();
    editor.request(
        1,
        Action::ClearCanvas {
            color: Rgba([255, 0, 0, 255]),
        },
    );
    wait_for_capacity(&mut editor);
    assert_eq!(
        editor.allocation_wait().unwrap().required_bytes(),
        2 * BYTES
    );
    assert!(editor.is_dirty());
    assert!(editor.has_pending_edits());
    assert!(!editor.can_capture_file());
    assert_eq!(editor.preview(), preview);
    assert!(Arc::ptr_eq(&original, &editor.document_snapshot()));
    assert_eq!(editor.undo_depth(), 0);
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
    assert_eq!(editor.publications(), 1);
    assert_eq!(editor.undo_depth(), 1);
    let raster = &editor.document().layers[&editor.target().unwrap()].raster;
    for x in 0..257 {
        assert_eq!(raster.get(x, 0), Rgba([0, 0, 0, 255]));
    }
    assert!(editor.can_capture_file());
    assert_eq!(budget.usage().reserved, 2 * BYTES);
    editor.release_file_barrier(boundary);
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.publications(), 2);
    assert_eq!(
        editor.document().layers[&editor.target().unwrap()]
            .raster
            .get(0, 0),
        Rgba([255, 0, 0, 255])
    );
}
#[test]
fn escape_discards_waiting_and_later_edits_without_crossing_the_save_barrier() {
    let (mut editor, _, budget) = setup(2);
    let blocker = budget.reserve(BYTES).unwrap();
    let original = editor.document_snapshot();
    stroke(&mut editor);
    editor.run_pending();
    let boundary = editor.queue_file_barrier();
    editor.request(1, Action::AddLayer);
    wait_for_capacity(&mut editor);
    editor.cancel_pending_edits();
    assert!(editor.allocation_wait().is_none());
    assert_eq!(editor.status(), "Edit cancelled");
    editor.run_pending();
    assert!(editor.file_barrier_ready(boundary));
    assert!(Arc::ptr_eq(&original, &editor.document_snapshot()));
    drop(blocker);
    editor.release_file_barrier(boundary);
    for _ in 0..4 {
        editor.run_pending();
    }
    assert!(!editor.has_pending_edits());
    assert_eq!(editor.publications(), 0);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn waiting_batch_is_not_rebased_after_a_competing_publication() {
    let (mut editor, _, budget) = setup(2);
    let blocker = budget.reserve(BYTES).unwrap();
    stroke(&mut editor);
    wait_for_capacity(&mut editor);
    let t = editor.session.ticket();
    let batch = Batch::from_json(format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"canvas.clear","ref":"background","color":"#FF0000FF"}}]}}"##,t.state_id,t.publication_epoch).as_bytes()).unwrap();
    editor.session.apply(&batch).unwrap();
    let newer = editor.document_snapshot();
    drop(blocker);
    editor.run_pending();
    assert!(editor.allocation_wait().is_none());
    assert!(!editor.has_pending_edits());
    assert!(Arc::ptr_eq(&newer, &editor.document_snapshot()));
    assert!(editor.status().contains("base changed"));
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn impossible_and_fixed_staged_limits_are_terminal_and_preserve_history() {
    for staged in [false, true] {
        let (mut editor, _, budget) = setup(if staged { 2 } else { 1 });
        if staged {
            editor.session.set_staged_pixel_budget(65_536);
        }
        let original = editor.document_snapshot();
        stroke(&mut editor);
        until(|| {
            editor.run_pending();
            !editor.has_pending_edits()
        });
        assert!(editor.allocation_wait().is_none());
        assert!(editor.status().contains("resource.limit"));
        assert!(Arc::ptr_eq(&original, &editor.document_snapshot()));
        assert_eq!(editor.undo_depth(), 0);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn waiting_edit_can_reclaim_own_history_with_a_queued_file_barrier() {
    let (mut editor, _, budget) = setup(3);
    for x in 0..3 {
        editor.pointer_down((x, 0));
        editor.pointer_up();
        until(|| {
            editor.run_pending();
            !editor.has_pending_edits()
        });
    }
    assert_eq!(budget.usage().reserved, 3 * BYTES);
    let history = editor.history_view();
    let ticket = editor.render_key();
    stroke(&mut editor);
    editor.run_pending();
    let boundary = editor.queue_file_barrier();
    wait_for_capacity(&mut editor);
    assert!(editor.start_residency_maintenance());
    until(|| {
        editor.poll_residency_maintenance();
        !editor.has_pending_maintenance()
    });
    assert_eq!(editor.history_view(), history);
    assert_eq!(editor.render_key(), ticket);
    assert!(budget.usage().reserved < 3 * BYTES);
    until(|| {
        editor.run_pending();
        editor.file_barrier_ready(boundary)
    });
    assert_eq!(editor.publications(), 4);
    assert_eq!(editor.undo_depth(), 4);
}
#[test]
fn worker_interference_keeps_waiting_command_without_holding_unused_tile_capacity() {
    let (mut editor, service, budget) = setup(2);
    let blocker = budget.reserve(BYTES).unwrap();
    stroke(&mut editor);
    wait_for_capacity(&mut editor);
    let release = service.pause();
    drop(blocker);
    for _ in 0..4 {
        editor.run_pending();
    }
    assert!(editor.allocation_wait().is_some());
    assert_eq!(budget.usage().reserved, 0);
    release.send(()).unwrap();
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.publications(), 1);
    assert_eq!(budget.usage().reserved, 2 * BYTES);
}
