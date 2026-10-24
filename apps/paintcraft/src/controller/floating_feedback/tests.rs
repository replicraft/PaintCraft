use super::*;
use eframe::egui;
use paintcraft_core::types::resources::SharedBudget;
use std::time::{Duration, Instant};
fn until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready() {
        assert!(
            Instant::now() < deadline,
            "floating feedback worker stalled"
        );
        std::thread::yield_now();
    }
}
fn fixture() -> Editor {
    let mut editor = Editor::new_document(12, 8).unwrap();
    editor.color = Rgba([23, 45, 67, 255]);
    editor.pointer_down((2, 2));
    editor.pointer_up();
    editor.run_pending();
    editor.request(
        101,
        Action::Select {
            corners: Some(((2, 2), (4, 3))),
        },
    );
    editor.run_pending();
    editor.select_tool(Tool::Select);
    editor
}
fn begin(editor: &mut Editor) -> (Arc<Document>, Selection, Selection, Document, usize) {
    let source = editor.document_snapshot();
    let selection = editor.selection();
    let depth = editor.undo_depth();
    editor.pointer_down((2, 2));
    editor.pointer_move((6, 4));
    let expected = paintcraft_editor::execute::execute(
        editor.document(),
        editor.selection(),
        &editor.preview_batch().unwrap().unwrap(),
    )
    .unwrap()
    .doc;
    let displayed = editor.display_selection();
    (source, selection, displayed, expected, depth)
}
#[test]
fn queued_and_worker_capacity_wait_keep_moved_overlay_private_then_commit_once_and_undo() {
    let service = background::Service::new(egui::Context::default()).unwrap();
    let release = service.pause();
    let mut editor = fixture();
    editor.connect_editing(service);
    let count = editor.publications();
    let (source, selection, displayed, expected, depth) = begin(&mut editor);
    editor.pointer_up();
    assert!(!editor.has_floating_selection());
    assert_eq!(
        editor.display_selection(),
        displayed,
        "release must not flash the original field"
    );
    assert!(
        matches!(editor.preview(), Some(Preview::MoveSelection { .. })),
        "queued pixels and overlay share the same prefix"
    );
    for _ in 0..4 {
        editor.run_pending();
        assert_eq!(editor.display_selection(), displayed);
        assert_eq!(editor.selection(), selection);
        assert!(Arc::ptr_eq(&source, &editor.document_snapshot()));
        assert_eq!(editor.undo_depth(), depth);
        assert_eq!(editor.publications(), count);
    }
    release.send(()).unwrap();
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert_eq!(editor.publications(), count + 1);
    assert!(editor.document().same_semantics(&expected));
    assert_eq!(
        editor.display_selection(),
        displayed,
        "published field is not offset twice"
    );
    assert!(editor.preview().is_none());
    editor.request(102, Action::Undo);
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert!(editor.document().same_semantics(&source));
    assert_eq!(editor.selection(), selection);
}
#[test]
fn rejected_initial_plan_retains_exact_feedback_until_capacity_returns() {
    let budget = SharedBudget::new(3 << 30);
    let service =
        background::Service::with_budget(egui::Context::default(), budget.clone()).unwrap();
    let mut editor = fixture();
    editor.connect_editing(service);
    let (source, selection, displayed, expected, depth) = begin(&mut editor);
    let preview = editor.preview().unwrap();
    editor.cancel_gesture();
    let blocker = budget.reserve(2 << 30).unwrap();
    editor.publication_preview = Some(preview.clone());
    let Preview::MoveSelection {
        layer,
        offset,
        clear,
        transparent_key,
    } = preview
    else {
        panic!("move")
    };
    editor.move_selection(layer, offset, clear, transparent_key);
    assert!(editor.waiting_dispatch.is_some());
    for _ in 0..4 {
        editor.run_pending();
        assert_eq!(editor.display_selection(), displayed);
        assert!(Arc::ptr_eq(&source, &editor.document_snapshot()));
        assert_eq!(editor.undo_depth(), depth);
    }
    drop(blocker);
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert!(editor.document().same_semantics(&expected));
    assert_eq!(editor.display_selection(), displayed);
    editor.request(102, Action::Undo);
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
    assert!(editor.document().same_semantics(&source));
    assert_eq!(editor.selection(), selection);
}
#[test]
fn cached_preview_cannot_survive_a_wrong_target_or_identical_ticket_session_replacement() {
    let mut editor = fixture();
    let (_, selection, _, _, _) = begin(&mut editor);
    let retained = editor.preview_snapshot().unwrap();
    let target = editor.target;
    editor.target = None;
    assert!(editor.preview_snapshot().is_none());
    assert_eq!(editor.display_selection(), selection);
    editor.target = target;
    assert!(editor.preview_snapshot().is_some());
    let source = editor.document_snapshot();
    let ticket = editor.session.ticket();
    let mut replacement = Editor::with_document((*source).clone(), None);
    replacement.request(
        301,
        Action::Select {
            corners: Some(((0, 0), (0, 0))),
        },
    );
    replacement.run_pending();
    replacement.request(
        302,
        Action::Select {
            corners: Some(((2, 2), (4, 3))),
        },
    );
    replacement.run_pending();
    assert_eq!(replacement.session.ticket(), ticket);
    editor.session = replacement.session;
    assert!(!Arc::ptr_eq(&source, &editor.document_snapshot()));
    assert!(matches!(&*retained, Preview::MoveSelection { .. }));
    assert!(
        editor.preview_snapshot().is_none(),
        "a weak cache hit still has to validate its frozen source"
    );
    assert_eq!(editor.display_selection(), selection);
}
