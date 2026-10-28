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
fn constructed_shapes_keep_a_frozen_preview_while_waiting_for_native_worker_capacity() {
    for tool in [Tool::Polygon, Tool::Curve] {
        let service = Service::new(egui::Context::default()).unwrap();
        let release = service.pause();
        let mut editor = editor(&service);
        editor.tool = tool;
        editor.pointer_down((1, 1));
        editor.pointer_move((6, 1));
        editor.pointer_up();
        editor.pointer_down((1, 6));
        editor.pointer_up();
        if tool == Tool::Polygon {
            editor.pointer_down((1, 1));
        } else {
            editor.pointer_down((6, 6));
        }
        let before = editor.document_snapshot();
        let history = editor.history_view();
        let batch = editor.preview_batch().unwrap().unwrap();
        let expected = paintcraft_editor::execute::execute(&before, editor.selection(), &batch)
            .unwrap()
            .doc;
        editor.pointer_up();
        assert_eq!(editor.run_pending(), 0);
        assert!(editor.has_pending_edits());
        assert_eq!(editor.history_view(), history);
        assert!(matches!(editor.preview(), Some(Preview::Contour(_))));
        release.send(()).unwrap();
        until(|| {
            editor.run_pending();
            !editor.has_pending_edits()
        });
        assert_eq!(editor.publications(), 1);
        assert!(editor.document().same_semantics(&expected));
        assert!(editor.preview().is_none());
    }
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

#[test]
fn retained_font_text_keeps_its_captured_layer_after_text_entry_and_target_selection_change() {
    let service = Service::new(egui::Context::default()).unwrap();
    let renderer = Arc::new(Mutex::new(crate::text::bundled()));
    let mut editor = Editor::new_document(128, 80)
        .unwrap()
        .with_text_renderer(renderer);
    let background = editor.target().unwrap();
    editor.request(1, Action::AddLayer);
    editor.run_pending();
    let text_layer = editor.target().unwrap();
    editor.connect_editing(service.clone());
    let release = service.pause();
    editor.select_tool(Tool::Text);
    editor.color = Rgba([30, 90, 180, 128]);
    editor.pointer_down((8, 8));
    editor.type_text("Hello Ω");
    let batch = editor.preview_batch().unwrap().unwrap();
    let staged =
        paintcraft_editor::execute::execute(editor.document(), editor.selection(), &batch).unwrap();
    let expected = paintcraft_render::render_flat(&staged.doc).unwrap();
    assert!(
        expected
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| *pixel != [255; 4])
    );
    editor.commit_text();
    assert!(!editor.is_editing_text());
    assert!(editor.set_target(background));
    editor.run_pending();
    assert!(matches!(editor.preview(), Some(Preview::Text { layer, .. }) if layer == text_layer));
    let retained = editor
        .preview_batch()
        .expect("retained text still has a captured layer")
        .unwrap();
    let staged =
        paintcraft_editor::execute::execute(editor.document(), editor.selection(), &retained)
            .unwrap();
    assert_eq!(
        paintcraft_render::render_flat(&staged.doc).unwrap(),
        expected
    );
    release.send(()).unwrap();
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.target(), Some(background));
    assert_eq!(
        paintcraft_render::render_flat(editor.document()).unwrap(),
        expected
    );
    assert_eq!(editor.publications(), 2);
}

#[test]
fn styled_text_command_submission_does_not_wait_for_worker_font_rasterization() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let service = Service::new(egui::Context::default()).unwrap();
    let fonts = Arc::new(Mutex::new(crate::text::bundled()));
    let mut editor = Editor::new_document(128, 80)
        .unwrap()
        .with_text_renderer(fonts.clone());
    editor.connect_editing(service);
    editor.select_tool(Tool::Text);
    editor.pointer_down((8, 8));
    editor.type_text("Hello Ω");
    let (started, wait_started) = mpsc::sync_channel(1);
    let (release, wait_release) = mpsc::channel();
    let forced = Arc::new(AtomicBool::new(false));
    let timed_out = forced.clone();
    let holder = std::thread::spawn(move || {
        let _guard = fonts.lock().unwrap();
        started.send(()).unwrap();
        if wait_release.recv_timeout(Duration::from_secs(8)).is_err() {
            timed_out.store(true, Ordering::SeqCst);
        }
    });
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    editor.commit_text();
    assert_eq!(editor.run_pending(), 0);
    assert!(editor.pending_edit.is_some());
    assert!(
        !forced.load(Ordering::SeqCst),
        "command dispatch waited for font work"
    );
    release.send(()).unwrap();
    holder.join().unwrap();
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.publications(), 1);
    assert!(
        paintcraft_render::render_flat(editor.document())
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| *pixel != [255; 4])
    );
}

#[test]
fn whitespace_text_finishes_as_a_guarded_noop_and_preserves_the_redo_branch() {
    let service = Service::new(egui::Context::default()).unwrap();
    let fonts = Arc::new(Mutex::new(crate::text::bundled()));
    let mut editor = Editor::new_document(128, 80)
        .unwrap()
        .with_text_renderer(fonts);
    editor.connect_editing(service);
    editor.request(1, Action::AddLayer);
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    editor.request(2, Action::Undo);
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    let before = editor.history_view();
    editor.select_tool(Tool::Text);
    editor.pointer_down((8, 8));
    editor.type_text("   ");
    editor.commit_text();
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.history_view(), before);
    assert_eq!(editor.redo_depth(), 1);
    assert!(!editor.is_dirty());
    assert!(editor.status().contains("changed nothing"));
}

/// Tests can pause the actual successful preparation while its own Pending and
/// resource lease remain live, independently of admission contention.
#[derive(Debug)]
pub(super) struct CompletionGate {
    prepared: mpsc::SyncSender<()>,
    release: mpsc::Receiver<()>,
}
impl Service {
    pub(crate) fn pause_next_completion(&self) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (prepared, observe) = mpsc::sync_channel(1);
        let (release, wait) = mpsc::channel();
        let mut gate = self.completion_gate.lock().unwrap();
        assert!(gate.is_none());
        *gate = Some(CompletionGate {
            prepared,
            release: wait,
        });
        (observe, release)
    }
    pub(super) fn gate_completion(&self, task: dispatch::Task) -> dispatch::Task {
        let Some(gate) = self.completion_gate.lock().unwrap().take() else {
            return task;
        };
        Box::new(move |cancel| {
            let result = task(cancel);
            gate.prepared.send(()).unwrap();
            gate.release.recv_timeout(Duration::from_secs(10)).unwrap();
            result
        })
    }
}
