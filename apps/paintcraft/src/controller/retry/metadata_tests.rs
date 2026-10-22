use super::tests::{setup, stroke, until};
use super::*;
use paintcraft_core::types::resources::SharedBudget;
use std::time::{Duration, Instant};
fn fixture(limit: u64) -> (Editor, SharedBudget, SharedBudget) {
    let (mut editor, service, tiles) = setup(2);
    let metadata = SharedBudget::new(limit);
    editor.connect_editing(service.with_metadata(metadata.clone()));
    (editor, tiles, metadata)
}
fn wait(editor: &mut Editor) {
    until(|| {
        editor.run_pending();
        editor.metadata_wait().is_some()
    });
}
fn finish(editor: &mut Editor) {
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
}

#[test]
fn metadata_wait_keeps_preview_fifo_and_file_barriers_then_publishes_once() {
    let (mut editor, tiles, metadata) = fixture(512);
    let blocker = metadata.reserve(400).unwrap();
    let before = editor.document_snapshot();
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
    wait(&mut editor);
    assert!(editor.allocation_wait().is_none());
    assert!(!editor.can_capture_file());
    assert_eq!(editor.preview(), preview);
    assert!(Arc::ptr_eq(&before, &editor.document_snapshot()));
    assert_eq!(tiles.usage().reserved, 0);
    assert_eq!(metadata.usage().reserved, 400);
    let activity = metadata.activity();
    let probe = editor.waiting_edit.as_ref().unwrap().last_probe;
    for _ in 0..4 {
        editor.run_pending();
        assert!(editor.pending_edit.is_none());
    }
    assert_eq!(metadata.activity(), activity);
    assert_eq!(editor.waiting_edit.as_ref().unwrap().last_probe, probe);
    assert!(
        !editor.start_residency_maintenance(),
        "metadata wait cannot bypass a queued file barrier for tile cleanup"
    );
    drop(blocker);
    until(|| {
        editor.run_pending();
        editor.file_barrier_ready(boundary)
    });
    assert_eq!(editor.publications(), 1);
    assert_eq!(editor.undo_depth(), 1);
    assert!(editor.metadata_wait().is_none());
    let raster = &editor.document().layers[&editor.target().unwrap()].raster;
    assert_eq!(raster.get(256, 0), Rgba([0, 0, 0, 255]));
    editor.release_file_barrier(boundary);
    finish(&mut editor);
    assert_eq!(editor.publications(), 2);
    assert_eq!(
        editor.document().layers[&editor.target().unwrap()]
            .raster
            .get(0, 0),
        Rgba([255, 0, 0, 255])
    );
    assert_eq!(metadata.usage().reserved, 0);
}
#[test]
fn escape_retires_metadata_wait_and_later_edits_without_crossing_the_file_boundary() {
    let (mut editor, _, metadata) = fixture(512);
    let blocker = metadata.reserve(400).unwrap();
    let before = editor.document_snapshot();
    stroke(&mut editor);
    editor.run_pending();
    let boundary = editor.queue_file_barrier();
    editor.request(
        1,
        Action::ClearCanvas {
            color: Rgba([255, 0, 0, 255]),
        },
    );
    wait(&mut editor);
    editor.cancel_pending_edits();
    until(|| {
        editor.run_pending();
        editor.file_barrier_ready(boundary)
    });
    drop(blocker);
    for _ in 0..4 {
        editor.run_pending();
    }
    assert!(editor.metadata_wait().is_none());
    assert_eq!(editor.publications(), 0);
    assert!(Arc::ptr_eq(&before, &editor.document_snapshot()));
}
#[test]
fn stale_metadata_request_is_never_rebased_onto_a_competing_publication() {
    let (mut editor, _, metadata) = fixture(512);
    let blocker = metadata.reserve(400).unwrap();
    stroke(&mut editor);
    wait(&mut editor);
    let t = editor.session.ticket();
    let batch=Batch::from_json(format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"canvas.clear","ref":"background","color":"#0000FFFF"}}]}}"##,t.state_id,t.publication_epoch).as_bytes()).unwrap();
    editor.session.apply(&batch).unwrap();
    let before = editor.document_snapshot();
    drop(blocker);
    editor.run_pending();
    assert!(editor.metadata_wait().is_none());
    assert!(editor.status().contains("base changed"));
    assert!(Arc::ptr_eq(&before, &editor.document_snapshot()));
}
#[test]
fn impossible_metadata_demand_is_terminal_and_never_registers_a_wait() {
    let (mut editor, _, _) = fixture(128);
    let before = editor.document_snapshot();
    stroke(&mut editor);
    finish(&mut editor);
    assert!(editor.metadata_wait().is_none());
    assert!(editor.allocation_wait().is_none());
    assert!(editor.status().contains("resource.limit"));
    assert_eq!(editor.publications(), 0);
    assert!(Arc::ptr_eq(&before, &editor.document_snapshot()));
}
#[test]
fn periodic_metadata_repricing_does_not_spin_on_its_own_allocation_cleanup() {
    let (mut editor, _, metadata) = fixture(512);
    let blocker = metadata.reserve(400).unwrap();
    stroke(&mut editor);
    wait(&mut editor);
    editor.waiting_edit.as_mut().unwrap().last_probe = Instant::now() - Duration::from_secs(6);
    editor.run_pending();
    assert!(editor.pending_edit.is_some());
    wait(&mut editor);
    let activity = metadata.activity();
    let probe = editor.waiting_edit.as_ref().unwrap().last_probe;
    for _ in 0..8 {
        editor.run_pending();
        assert!(editor.pending_edit.is_none());
    }
    assert_eq!(activity, metadata.activity());
    assert_eq!(probe, editor.waiting_edit.as_ref().unwrap().last_probe);
    drop(blocker);
    finish(&mut editor);
    assert_eq!(editor.publications(), 1);
}
#[test]
fn retry_preserves_both_original_realms_across_worker_service_replacement() {
    let (mut editor, tiles, metadata) = fixture(512);
    let blocker = metadata.reserve(400).unwrap();
    stroke(&mut editor);
    wait(&mut editor);
    let foreign = SharedBudget::new(0);
    let service =
        background::Service::with_residency(eframe::egui::Context::default(), foreign.clone())
            .unwrap()
            .with_metadata(foreign.clone());
    editor.connect_editing(service);
    drop(blocker);
    finish(&mut editor);
    assert_eq!(editor.publications(), 1);
    assert_eq!(foreign.usage().reserved, 0);
    assert_ne!(tiles.usage().reserved, 0);
    assert_eq!(metadata.usage().reserved, 0);
}
