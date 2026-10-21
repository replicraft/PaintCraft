use super::*;
use std::time::{Duration, Instant};

fn checked(editor: &Editor) -> Option<Arc<Preview>> {
    let snapshot = editor.preview_snapshot();
    assert_eq!(snapshot.as_deref(), editor.collect_preview().as_ref());
    snapshot
}
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}
#[test]
fn idle_and_repeated_points_reuse_snapshot_but_growth_keeps_old_snapshot_immutable() {
    let mut editor = Editor::new_document(8, 8).unwrap();
    editor.pointer_down((1, 1));
    let first = checked(&editor).unwrap();
    editor.pointer_move((1, 1));
    editor.color = Rgba([99; 4]); // Future settings cannot change a captured gesture.
    editor.options.pencil_size = 7;
    assert!(Arc::ptr_eq(&first, &checked(&editor).unwrap()));
    editor.pointer_move((2, 1));
    let grown = checked(&editor).unwrap();
    assert!(!Arc::ptr_eq(&first, &grown));
    assert!(matches!(first.as_ref(), Preview::Path(path) if path.points == [(1,1)]));
    assert!(matches!(grown.as_ref(), Preview::Path(path) if path.points == [(1,1),(2,1)]));
    let weak = Arc::downgrade(&grown);
    drop(grown);
    assert!(weak.upgrade().is_none());
    checked(&editor).unwrap(); // Expired entries rebuild correctly.
    editor.cancel_gesture();
    assert!(checked(&editor).is_none());
}
#[test]
fn shape_constraint_and_text_edits_refresh_the_snapshot() {
    let mut editor = Editor::new_document(8, 8).unwrap();
    editor.tool = Tool::Rect;
    editor.pointer_down((0, 0));
    editor.pointer_move((5, 2));
    let rectangle = checked(&editor).unwrap();
    editor.constrain_shape(true);
    let square = checked(&editor).unwrap();
    assert_ne!(rectangle, square);
    editor.constrain_shape(false);
    assert_eq!(checked(&editor).unwrap(), rectangle);
    editor.select_tool(Tool::Text);
    editor.pointer_down((1, 1));
    checked(&editor).unwrap();
    editor.type_text("ab");
    let text = checked(&editor).unwrap();
    editor.type_text("");
    assert!(Arc::ptr_eq(&text, &checked(&editor).unwrap()));
    editor.backspace();
    assert_ne!(text, checked(&editor).unwrap());
    editor.set_text_style(paintcraft_core::types::text::TextOptions::default());
    checked(&editor).unwrap();
    editor.commit_text();
    assert!(checked(&editor).is_none());
    editor.run_pending();
    checked(&editor);
}
#[test]
fn queued_paint_file_boundaries_and_completion_never_reuse_stale_snapshots() {
    let service = background::Service::new(eframe::egui::Context::default()).unwrap();
    let release = service.pause();
    let mut editor = Editor::new_document(8, 8).unwrap();
    editor.connect_editing(service.clone());
    for x in 0..3 {
        editor.pointer_down((x, 1));
        checked(&editor).unwrap();
        editor.pointer_up();
        checked(&editor).unwrap();
    }
    editor.run_pending();
    let queued = checked(&editor).unwrap();
    editor.run_pending();
    assert!(Arc::ptr_eq(&queued, &checked(&editor).unwrap()));
    let barrier = editor.queue_file_barrier();
    checked(&editor).unwrap();
    release.send(()).unwrap();
    until(|| {
        editor.run_pending();
        checked(&editor);
        editor.file_barrier_ready(barrier)
    });
    assert!(checked(&editor).is_none());
    editor.release_file_barrier(barrier);
    editor.pointer_down((4, 1));
    checked(&editor).unwrap();
    editor.pointer_up();
    checked(&editor).unwrap();
    editor.cancel_pending_edits();
    assert!(checked(&editor).is_none());
    editor.request(12345, Action::Undo);
    checked(&editor);
    until(|| {
        editor.run_pending();
        checked(&editor);
        !editor.has_pending_edits()
    });
    assert_eq!(editor.publications(), 4);
}
