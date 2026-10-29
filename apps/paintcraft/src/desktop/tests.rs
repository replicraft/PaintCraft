//! Actual App recovery from a native capture refusal.
use super::*;
use paintcraft_tools::Action;

#[test]
fn capture_fallback_keeps_document_and_ordinary_pending_edits() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.initialize_editing(ctx.clone());
    let budget = app.worker_budget.clone();
    let mut host = Host::new(app, 8, &budget).unwrap();
    let before = host.app_mut().editor.document_snapshot();
    let depth = host.app_mut().editor.undo_depth();
    host.app_mut().editor.request(9137, Action::AddLayer);
    assert!(host.app_mut().tablet.is_some());
    restore_pointer_input(&mut host, &ctx);
    let app = host.app_mut();
    assert!(app.tablet.is_none());
    assert!(app.editor.document().same_semantics(&before));
    assert_eq!(app.editor.undo_depth(), depth);
    // Native refusal must not discard a genuine queued GUI edit.
    let deadline = Instant::now() + std::time::Duration::from_secs(10);
    let mut published = 0;
    while app.editor.has_pending_edits() {
        assert!(
            Instant::now() < deadline,
            "ordinary edit did not finish after fallback"
        );
        published += app.editor.run_pending();
        std::thread::yield_now();
    }
    assert_eq!(published, 1);
    assert_eq!(
        app.editor.document().layers_in_order().count(),
        before.layers_in_order().count() + 1
    );
    assert_eq!(app.editor.undo_depth(), depth + 1);
}
