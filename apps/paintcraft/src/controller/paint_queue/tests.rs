use super::*;
use paintcraft_core::types::storage::{SpillBlob, SpillStore, SpillUsage};
use std::{
    sync::{Mutex, mpsc},
    time::{Duration, Instant},
};
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}
fn editor() -> (Editor, background::Service) {
    let service = background::Service::new(eframe::egui::Context::default()).unwrap();
    let mut editor = Editor::new_document(8, 8).unwrap();
    editor.connect_editing(service.clone());
    (editor, service)
}
fn stroke(editor: &mut Editor, color: Rgba, at: (i64, i64)) {
    editor.color = color;
    editor.pointer_down(at);
    editor.pointer_up();
}
fn pixel(editor: &Editor, at: (i64, i64)) -> Rgba {
    editor.document().layers[&editor.target().unwrap()]
        .raster
        .get(at.0, at.1)
}
fn preview_pixel(editor: &Editor, at: (i64, i64)) -> Rgba {
    let batch = editor.preview_batch().unwrap().unwrap();
    let staged =
        paintcraft_editor::execute::execute(editor.document(), editor.selection(), &batch).unwrap();
    staged.doc.layers[&editor.target().unwrap()]
        .raster
        .get(at.0, at.1)
}
#[test]
fn identical_translucent_strokes_in_one_queue_are_previewed_once_each_and_remain_separate_undo_steps()
 {
    let (mut editor, service) = editor();
    let release = service.pause();
    for _ in 0..3 {
        stroke(&mut editor, Rgba([0, 0, 0, 64]), (3, 3));
    }
    editor.run_pending();
    assert_eq!(editor.publications(), 0);
    assert_eq!(pixel(&editor, (3, 3)), Rgba([255; 4]));
    assert!(matches!(editor.preview(), Some(Preview::Paths(paths)) if paths.len()==3));
    let expected = Rgba([107, 107, 107, 255]);
    assert_eq!(preview_pixel(&editor, (3, 3)), expected);
    release.send(()).unwrap();
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.publications(), 3);
    assert_eq!(pixel(&editor, (3, 3)), expected);
    for (token, expected) in [(100, 143), (101, 191), (102, 255)] {
        editor.request(token, Action::Undo);
        until(|| {
            editor.run_pending();
            !editor.has_pending_edits()
        });
        assert_eq!(
            pixel(&editor, (3, 3)),
            Rgba([expected, expected, expected, 255])
        );
    }
    for token in 103..106 {
        editor.request(token, Action::Redo);
        until(|| {
            editor.run_pending();
            !editor.has_pending_edits()
        });
    }
    assert_eq!(pixel(&editor, (3, 3)), expected);
}
#[derive(Debug)]
struct Gate {
    native: paintcraft_platform::spill::NativeSpillStore,
    started: mpsc::SyncSender<()>,
    release: Mutex<Option<mpsc::Receiver<()>>>,
}
impl SpillStore for Gate {
    fn write(&self, bytes: &[u8]) -> Result<Arc<dyn SpillBlob>> {
        self.native.write(bytes)
    }
    fn flush(&self) -> Result<()> {
        self.native.flush()?;
        if self.native.usage().reserved_bytes > 0
            && let Some(release) = self.release.lock().unwrap().take()
        {
            self.started.send(()).unwrap();
            release.recv_timeout(Duration::from_secs(10)).unwrap();
        }
        Ok(())
    }
    fn collect_garbage(&self) -> Result<SpillUsage> {
        self.native.collect_garbage()
    }
    fn usage(&self) -> SpillUsage {
        self.native.usage()
    }
}
#[test]
fn a_live_stroke_survives_an_earlier_strokes_actual_spill_and_publication_without_double_blending()
{
    let (mut editor, _) = editor();
    for at in [(0, 0), (7, 7)] {
        stroke(&mut editor, Rgba([0, 0, 0, 255]), at);
        until(|| {
            editor.run_pending();
            !editor.has_pending_edits()
        });
    }
    let root = std::env::temp_dir().join(format!("paintcraft-paint-queue-{}", Uuid::new_v4()));
    let (started, wait_started) = mpsc::sync_channel(1);
    let (release, wait_release) = mpsc::channel();
    let gate = Arc::new(Gate {
        native: paintcraft_platform::spill::NativeSpillStore::new(&root, 8 << 20, 1 << 20).unwrap(),
        started,
        release: Mutex::new(Some(wait_release)),
    });
    editor.session.set_history_spill_store(gate.clone());
    editor
        .session
        .set_history_budget(2 * paintcraft_raster::RasterLayer::TILE_BYTES as u64);
    stroke(&mut editor, Rgba([255, 0, 0, 128]), (3, 3));
    editor.run_pending();
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(editor.pending_edit.is_some());
    assert!(
        gate.usage().reserved_bytes > 0,
        "the worker is held after a real native spill write"
    );
    assert_eq!(editor.publications(), 2);
    editor.color = Rgba([0, 0, 255, 128]);
    editor.pointer_down((3, 3));
    assert!(editor.gesture_in_progress());
    let expected = Rgba([127, 63, 191, 255]);
    assert_eq!(preview_pixel(&editor, (3, 3)), expected);
    release.send(()).unwrap();
    until(|| {
        editor.run_pending();
        editor.publications() == 3
    });
    assert!(editor.gesture_in_progress());
    assert_eq!(preview_pixel(&editor, (3, 3)), expected);
    editor.pointer_up();
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.publications(), 4);
    assert_eq!(pixel(&editor, (3, 3)), expected);
    drop(editor);
    assert_eq!(gate.collect_garbage().unwrap().reserved_bytes, 0);
    drop(gate);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn structural_and_file_boundaries_pause_new_gestures_and_exclude_later_paint_from_preview() {
    let (mut editor, service) = editor();
    let release = service.pause();
    stroke(&mut editor, Rgba([255, 0, 0, 128]), (3, 3));
    let barrier = editor.queue_file_barrier();
    assert!(editor.painting_blocked());
    editor.pointer_down((4, 4));
    assert!(!editor.gesture_in_progress());
    let layer = editor.target().unwrap();
    assert!(editor.request(
        20,
        Action::Stroke(Stroke {
            layer,
            points: vec![(3, 3)],
            point_scale: 1,
            color: Rgba([0, 0, 255, 128]),
            size: 1,
            round: false,
            mode: paintcraft_tools::BlendMode::SourceOver
        })
    ));
    assert_eq!(preview_pixel(&editor, (3, 3)), Rgba([255, 127, 127, 255]));
    release.send(()).unwrap();
    until(|| {
        editor.run_pending();
        editor.file_barrier_ready(barrier)
    });
    assert!(editor.preview_batch().is_none());
    assert_eq!(pixel(&editor, (3, 3)), Rgba([255, 127, 127, 255]));
    editor.release_file_barrier(barrier);
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(pixel(&editor, (3, 3)), Rgba([127, 63, 191, 255]));
    let release = service.pause();
    editor.request(
        21,
        Action::ClearCanvas {
            color: Rgba([255; 4]),
        },
    );
    assert!(editor.painting_blocked());
    release.send(()).unwrap();
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
}
#[test]
fn queue_limits_reject_excess_strokes_and_oversized_point_storage_without_losing_accepted_paths() {
    let (mut editor, service) = editor();
    let release = service.pause();
    for n in 0..MAX_STROKES {
        stroke(&mut editor, Rgba([0, 0, 0, 64]), (n as i64 % 8, 3));
    }
    assert!(editor.painting_blocked());
    assert_eq!(editor.outbox.pending().len(), MAX_STROKES);
    let layer = editor.target().unwrap();
    let action = |points| {
        Action::Stroke(Stroke {
            layer,
            points,
            point_scale: 1,
            color: Rgba([0, 0, 0, 64]),
            size: 1,
            round: false,
            mode: paintcraft_tools::BlendMode::SourceOver,
        })
    };
    assert!(!editor.request(999, action(vec![(1, 1)])));
    assert_eq!(editor.outbox.pending().len(), MAX_STROKES);
    editor.cancel_pending_edits();
    let mut oversized = Vec::with_capacity(MAX_POINT_CAPACITY + 1);
    oversized.push((1, 1));
    assert!(!editor.request(1000, action(oversized)));
    assert!(!editor.has_pending_edits());
    assert!(editor.request(1001, action(vec![(1, 1); MAX_POINTS])));
    let status = editor.status().to_owned();
    assert!(
        !editor.request(1001, action(vec![(1, 1)])),
        "duplicate activation is not a new admission"
    );
    assert_eq!(editor.status(), status);
    assert!(!editor.request(1002, action(vec![(2, 2)])));
    assert_eq!(editor.outbox.pending().len(), 1);
    editor.cancel_pending_edits();
    release.send(()).unwrap();
    until(|| service.available());
    assert_eq!(editor.publications(), 0);
}
#[test]
fn live_point_limit_keeps_the_captured_prefix_and_accepts_its_release() {
    let (mut editor, service) = editor();
    let release = service.pause();
    editor.pointer_down((0, 0));
    for n in 0..MAX_POINTS + 2 {
        editor.pointer_move(((n % 2) as i64, 3));
    }
    assert_eq!(editor.gesture_points().len(), MAX_POINTS);
    assert!(editor.status().contains("allowance reached"));
    assert!(
        !editor.painting_blocked(),
        "release must still be processed"
    );
    editor.pointer_up();
    assert_eq!(editor.outbox.pending().len(), 1);
    editor.cancel_pending_edits();
    release.send(()).unwrap();
    until(|| service.available());
}

#[test]
fn live_capacity_growth_is_admitted_before_replacing_the_captured_prefix() {
    let (mut editor, service) = editor();
    let release = service.pause();
    let mut points = Vec::with_capacity(MAX_POINT_CAPACITY - 3);
    points.push((7, 7));
    let layer = editor.target().unwrap();
    assert!(editor.request(
        999,
        Action::Stroke(Stroke {
            layer,
            points,
            point_scale: 1,
            color: Rgba([0, 0, 0, 255]),
            size: 1,
            round: false,
            mode: paintcraft_tools::BlendMode::SourceOver
        })
    ));
    editor.pointer_down((0, 0));
    let mut rejected = false;
    for x in 1..10 {
        let prefix = editor.gesture_points().to_vec();
        let capacity = editor.tools.gesture_point_capacity();
        editor.pointer_move((x, 0));
        assert!(editor.paint_load().capacity <= MAX_POINT_CAPACITY);
        if editor.status().contains("allowance reached") {
            assert_eq!(editor.gesture_points(), prefix);
            assert_eq!(editor.tools.gesture_point_capacity(), capacity);
            rejected = true;
            break;
        }
    }
    assert!(rejected);
    assert!(
        !editor.painting_blocked(),
        "release still commits the retained prefix"
    );
    editor.pointer_up();
    assert_eq!(editor.outbox.pending().len(), 2);
    editor.cancel_pending_edits();
    assert!(editor.preview().is_none());
    release.send(()).unwrap();
    until(|| service.available());
}

#[test]
fn invalid_fixed_point_conversion_keeps_the_live_brush_and_allows_valid_movement_afterward() {
    let (mut editor, service) = editor();
    let release = service.pause();
    editor.tool = Tool::Brush;
    editor.options.round = true;
    editor.pointer_down_subpixel((256, 256), paintcraft_input::PointerButton::Primary);
    let prefix = editor.gesture_points().to_vec();
    for point in [(i64::MAX, 0), (0, i64::MIN)] {
        editor.pointer_move(point);
        assert_eq!(editor.gesture_points(), prefix);
        assert!(editor.status().contains("fixed-point representation"));
    }
    editor.pointer_move((2, 2));
    assert_eq!(editor.gesture_points(), &[(256, 256), (640, 640)]);
    editor.pointer_up();
    assert_eq!(editor.outbox.pending().len(), 1);
    editor.cancel_pending_edits();
    release.send(()).unwrap();
    until(|| service.available());
}

#[test]
fn a_file_boundary_hides_an_unreleased_live_gesture_until_the_capture_finishes() {
    let (mut editor, service) = editor();
    let release = service.pause();
    stroke(&mut editor, Rgba([255, 0, 0, 128]), (3, 3));
    editor.color = Rgba([0, 0, 255, 128]);
    editor.pointer_down((3, 3));
    assert_eq!(preview_pixel(&editor, (3, 3)), Rgba([127, 63, 191, 255]));
    let boundary = editor.queue_file_barrier();
    assert_eq!(preview_pixel(&editor, (3, 3)), Rgba([255, 127, 127, 255]));
    release.send(()).unwrap();
    until(|| {
        editor.run_pending();
        editor.file_barrier_ready(boundary)
    });
    assert!(editor.preview().is_none());
    assert!(editor.preview_batch().is_none());
    assert_eq!(pixel(&editor, (3, 3)), Rgba([255, 127, 127, 255]));
    editor.cancel_gesture();
    editor.release_file_barrier(boundary);
    assert_eq!(editor.publications(), 1);
}
