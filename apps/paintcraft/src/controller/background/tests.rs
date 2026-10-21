use super::*;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

impl Service {
    /// Hold the actual bounded worker, then abandon its test-only completion.
    pub(crate) fn pause(&self) -> mpsc::Sender<()> {
        let (started, wait_started) = mpsc::sync_channel(1);
        let (release, wait_release) = mpsc::channel();
        let handle = self
            .lock()
            .unwrap()
            .submit(
                PREPARATION_CHARGE,
                move |_| {
                    started.send(()).unwrap();
                    wait_release.recv_timeout(Duration::from_secs(10)).unwrap();
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "test worker gate",
                    ))
                },
                || {},
            )
            .unwrap();
        wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(self.lock().unwrap().abandon(handle));
        release
    }
}
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "editing worker stalled");
        std::thread::yield_now();
    }
}
fn editor(service: &Service) -> Editor {
    let mut editor = Editor::new_document(8, 8).unwrap();
    editor.connect_editing(service.clone());
    editor
}
fn color(editor: &Editor) -> Rgba {
    editor.document().layers[&editor.document().roots[0]]
        .raster
        .default_pixel()
}
#[test]
fn queued_activations_use_one_shared_worker_publish_once_and_retarget_aliases() {
    let service = Service::new(egui::Context::default()).unwrap();
    let release = service.pause();
    let mut first = editor(&service);
    let mut second = editor(&service);
    let initial_target = first.target();
    assert!(first.request(1, Action::AddLayer));
    assert!(!first.request(1, Action::AddLayer));
    let boundary = first.queue_file_barrier();
    first.request(
        2,
        Action::ClearCanvas {
            color: Rgba([17, 88, 156, 255]),
        },
    );
    first.request(3, Action::Undo);
    second.request(1, Action::AddLayer);
    assert_eq!(first.run_pending(), 0);
    assert_eq!(second.run_pending(), 0);
    assert_eq!(first.target(), initial_target);
    assert_eq!(service.lock().unwrap().pending(), 1);
    release.send(()).unwrap();
    until(|| {
        first.run_pending();
        second.run_pending();
        first.file_barrier_ready(boundary)
    });
    assert_eq!(first.publications(), 1);
    assert_ne!(first.target(), initial_target);
    let added = first.target().unwrap();
    first.release_file_barrier(boundary);
    until(|| {
        first.run_pending();
        second.run_pending();
        !first.has_pending_edits() && !second.has_pending_edits()
    });
    assert_eq!(first.publications(), 3);
    assert_eq!(first.target(), Some(added));
    assert_eq!(first.document().layers.len(), 2);
    assert_eq!(second.publications(), 1);
    assert_eq!(second.document().layers.len(), 2);
    assert_eq!(service.lock().unwrap().reserved_bytes(), 0);
}
#[test]
fn snapshot_barrier_stops_later_edits_until_the_file_consumer_captures() {
    let service = Service::new(egui::Context::default()).unwrap();
    let mut editor = editor(&service);
    editor.request(
        1,
        Action::ClearCanvas {
            color: Rgba([0, 0, 0, 255]),
        },
    );
    let boundary = editor.queue_file_barrier();
    editor.request(
        2,
        Action::ClearCanvas {
            color: Rgba([255, 0, 0, 255]),
        },
    );
    until(|| {
        editor.run_pending();
        editor.file_barrier_ready(boundary)
    });
    assert_eq!(editor.publications(), 1);
    assert_eq!(color(&editor), Rgba([0, 0, 0, 255]));
    assert!(editor.can_capture_file());
    for _ in 0..5 {
        assert_eq!(editor.run_pending(), 0);
    }
    editor.release_file_barrier(boundary);
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.publications(), 2);
    assert_eq!(color(&editor), Rgba([255, 0, 0, 255]));
}
fn blocked_preparation(editor: &mut Editor, service: &Service) -> mpsc::Sender<()> {
    let t = editor.session.ticket();
    let batch = Batch::from_json(format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"canvas.clear","ref":"background","color":"#000000FF"}}]}}"##,t.state_id,t.publication_epoch).as_bytes()).unwrap();
    let preparation = editor.session.preparation().unwrap();
    let (started, wait_started) = mpsc::sync_channel(1);
    let (release, wait_release) = mpsc::channel();
    let pending = service
        .submit(
            move |cancellation| {
                started.send(()).unwrap();
                wait_release.recv_timeout(Duration::from_secs(10)).unwrap();
                preparation
                    .apply(&batch, || cancellation.check())
                    .map(Completion::Edit)
            },
            "Clear Canvas".into(),
            "",
            Some("background"),
        )
        .unwrap();
    editor.install_pending(Ok(pending));
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    release
}
#[test]
fn cancellation_and_dropped_consumers_keep_admission_until_the_worker_finishes() {
    let service = Service::new(egui::Context::default()).unwrap();
    let mut editor = editor(&service);
    let initial = editor.document_snapshot();
    let release = blocked_preparation(&mut editor, &service);
    editor.request(1, Action::AddLayer);
    editor.cancel_pending_edits();
    assert!(!service.available());
    release.send(()).unwrap();
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert!(Arc::ptr_eq(&editor.document_snapshot(), &initial));
    assert_eq!(editor.publications(), 0);
    assert_eq!(editor.status(), "Edit cancelled");
    let release = blocked_preparation(&mut editor, &service);
    drop(editor);
    assert!(!service.available());
    release.send(()).unwrap();
    until(|| service.available());
    assert_eq!(service.lock().unwrap().reserved_bytes(), 0);
}
#[test]
fn stale_worker_result_cannot_replace_a_newer_publication() {
    let service = Service::new(egui::Context::default()).unwrap();
    let mut editor = editor(&service);
    let release = blocked_preparation(&mut editor, &service);
    // Simulate a competing publisher while this worker holds its private base.
    editor.editing = None;
    editor.clear_canvas(Rgba([255, 0, 0, 255]));
    editor.editing = Some(service.clone());
    let newer = editor.document_snapshot();
    release.send(()).unwrap();
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert!(Arc::ptr_eq(&newer, &editor.document_snapshot()));
    assert_eq!(color(&editor), Rgba([255, 0, 0, 255]));
    assert_eq!(editor.publications(), 1);
    assert!(editor.status().contains("no longer matches"));
}
#[test]
fn released_stroke_keeps_its_preview_until_publication() {
    let service = Service::new(egui::Context::default()).unwrap();
    let release = service.pause();
    let mut editor = editor(&service);
    editor.pointer_down((1, 1));
    editor.pointer_move((5, 5));
    let preview = editor.preview().unwrap();
    editor.pointer_up();
    editor.run_pending();
    assert_eq!(editor.preview(), Some(preview));
    assert_eq!(editor.publications(), 0);
    release.send(()).unwrap();
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.publications(), 1);
    assert!(editor.preview().is_none());
}

#[test]
fn deleting_the_target_retargets_after_background_publication() {
    let service = Service::new(egui::Context::default()).unwrap();
    let mut editor = editor(&service);
    let background = editor.target().unwrap();
    editor.request(1, Action::AddLayer);
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    let added = editor.target().unwrap();
    assert_ne!(added, background);
    editor.request(2, Action::DeleteLayer { layer: added });
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.target(), Some(background));
    assert!(!editor.document().layers.contains_key(&added));
    assert_eq!(editor.publications(), 2);
}

#[test]
fn queued_history_click_cannot_erase_an_edit_that_publishes_ahead_of_it() {
    let service = Service::new(egui::Context::default()).unwrap();
    let mut editor = editor(&service);
    editor.request(1, Action::AddLayer);
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    let ticket = editor.history_view().ticket;
    let release = service.pause();
    editor.request(
        2,
        Action::ClearCanvas {
            color: Rgba([0, 0, 0, 255]),
        },
    );
    editor.request(
        3,
        Action::NavigateHistory {
            owner: ticket.owner,
            state_id: ticket.base.state_id,
            epoch: ticket.base.publication_epoch,
            history_id: None,
        },
    );
    release.send(()).unwrap();
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.publications(), 2);
    assert_eq!(color(&editor), Rgba([0, 0, 0, 255]));
    assert!(editor.status().contains("no longer current"));
}

#[test]
fn synchronous_controllers_still_accept_two_gestures_before_draining_the_outbox() {
    let mut editor = Editor::new_document(8, 8).unwrap();
    editor.pointer_down((1, 1));
    editor.pointer_up();
    editor.pointer_down((6, 6));
    editor.pointer_up();
    assert_eq!(editor.run_pending(), 2);
    let layer = &editor.document().layers[&editor.target().unwrap()];
    assert_eq!(layer.raster.get(1, 1), Rgba([0, 0, 0, 255]));
    assert_eq!(layer.raster.get(6, 6), Rgba([0, 0, 0, 255]));
}
