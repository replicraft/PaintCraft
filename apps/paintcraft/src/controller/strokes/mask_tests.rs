//! Host boundaries must retain exact finite mask coverage, including complements.
use super::*;
use std::time::{Duration, Instant};

const BASE: Rgba = Rgba([21, 45, 69, 255]);
const PAINT: Rgba = Rgba([211, 37, 83, 255]);
const CLEAR: Rgba = Rgba([255; 4]);

fn polygon() -> Value {
    Value::object([
        ("op", Value::str("selection.set_polygon")),
        (
            "points",
            Value::Array(
                [(1, 1), (7, 1), (7, 3), (3, 3), (3, 7), (1, 7)]
                    .into_iter()
                    .map(|(x, y)| Value::object([("x", Value::Int(x)), ("y", Value::Int(y))]))
                    .collect(),
            ),
        ),
    ])
}
fn selected(x: i64, y: i64, inverted: bool) -> bool {
    let inside =
        (1..7).contains(&x) && (1..3).contains(&y) || (1..3).contains(&x) && (3..7).contains(&y);
    inside != inverted
}
fn check_pixels(document: &Document, inverted: bool, selected_color: Rgba) {
    let layer = &document.layers[&document.roots[0]];
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(
                layer.raster.get(x, y),
                if selected(x, y, inverted) {
                    selected_color
                } else {
                    BASE
                },
                "pixel {x},{y}, inverted={inverted}"
            );
        }
    }
}
fn drain(editor: &mut Editor) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while editor.has_pending_edits() {
        assert!(Instant::now() < deadline, "mask editing worker stalled");
        editor.run_pending();
        std::thread::yield_now();
    }
}
fn seeded(inverted: bool) -> Editor {
    let mut editor = Editor::new_document(8, 8).unwrap();
    editor.clear_canvas(BASE);
    let mut operations = vec![polygon()];
    if inverted {
        operations.push(Value::object([("op", Value::str("selection.invert"))]));
    }
    assert!(
        editor
            .commit(operations, "Selection changed")
            .unwrap()
            .committed
    );
    assert!(matches!(editor.selection(), Selection::Mask(_)));
    editor
}
fn brush(editor: &mut Editor) {
    editor.select_tool(Tool::Brush);
    editor.options.round = false;
    editor.options.size = 20;
    editor.color = PAINT;
    editor.pointer_down((4, 4));
}

#[test]
fn mask_worker_brush_preview_publication_delete_and_history_match_explicit_pixels() {
    for inverted in [false, true] {
        let mut editor = seeded(inverted);
        let selection = editor.selection();
        let before = editor.document_snapshot();
        let history = editor.history_view();
        let depth = editor.undo_depth();
        let service = background::Service::new(eframe::egui::Context::default()).unwrap();
        let release = service.pause();
        editor.connect_editing(service);
        brush(&mut editor);
        let request = editor.prepare_preview(editor.preview());
        let source = before.clone();
        let captured_selection = selection.clone();
        let preview = std::thread::spawn(move || {
            let batch = request.build(|| Ok(())).unwrap().unwrap();
            paintcraft_editor::execute::execute(&source, captured_selection, &batch)
                .unwrap()
                .doc
        })
        .join()
        .unwrap();
        check_pixels(&preview, inverted, PAINT);
        assert!(Arc::ptr_eq(&before, &editor.document_snapshot()));
        assert_eq!(editor.history_view(), history);
        editor.pointer_up();
        editor.run_pending();
        assert!(editor.has_pending_edits());
        assert_eq!(editor.history_view(), history);
        release.send(()).unwrap();
        drain(&mut editor);
        check_pixels(editor.document(), inverted, PAINT);
        assert!(editor.document().same_semantics(&preview));
        assert_eq!(editor.selection(), selection);
        assert_eq!(editor.undo_depth(), depth + 1);
        editor.request(101, Action::Undo);
        drain(&mut editor);
        assert!(editor.document().same_semantics(&before));
        assert_eq!(editor.selection(), selection);
        editor.request(102, Action::Redo);
        drain(&mut editor);
        check_pixels(editor.document(), inverted, PAINT);
        editor.request(103, Action::EraseSelection { color: CLEAR });
        drain(&mut editor);
        check_pixels(editor.document(), inverted, CLEAR);
        assert_eq!(editor.selection(), selection);
        editor.request(104, Action::Undo);
        drain(&mut editor);
        check_pixels(editor.document(), inverted, PAINT);
        assert_eq!(editor.selection(), selection);
    }
}

#[test]
fn a_selection_only_publication_rejects_an_older_masked_worker_result() {
    let mut editor = seeded(false);
    let before = editor.document_snapshot();
    let service = background::Service::new(eframe::egui::Context::default()).unwrap();
    editor.connect_editing(service.clone());
    brush(&mut editor);
    let batch = editor.preview_batch().unwrap().unwrap();
    editor.cancel_gesture();
    let preparation = editor.session.preparation().unwrap();
    let (started, wait_started) = std::sync::mpsc::sync_channel(1);
    let (release, wait_release) = std::sync::mpsc::channel();
    let pending = service
        .submit(
            move |cancellation| {
                started.send(()).unwrap();
                wait_release.recv_timeout(Duration::from_secs(10)).unwrap();
                preparation
                    .apply(&batch, || cancellation.check())
                    .map(background::Completion::Edit)
            },
            "Stroke".into(),
            "",
            None,
        )
        .unwrap();
    editor.pending_edit = Some(pending);
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(editor.has_pending_edits());
    // Simulate a competing selection publisher while the worker owns the old source.
    editor.editing = None;
    assert!(
        editor
            .commit(
                vec![Value::object([("op", Value::str("selection.invert"))])],
                "Selection changed"
            )
            .unwrap()
            .committed
    );
    editor.editing = Some(service);
    let newer = editor.document_snapshot();
    let selection = editor.selection();
    let history = editor.history_view();
    assert!(newer.same_semantics(&before));
    release.send(()).unwrap();
    drain(&mut editor);
    assert!(Arc::ptr_eq(&newer, &editor.document_snapshot()));
    assert_eq!(editor.selection(), selection);
    assert_eq!(editor.history_view(), history);
    assert!(
        editor.status().contains("no longer matches"),
        "{}",
        editor.status()
    );
    let Selection::Mask(mask) = selection else {
        unreachable!()
    };
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(mask.coverage(x, y) != 0, selected(x, y, true));
        }
    }
}
