use super::*;
use std::time::Instant;
fn source(inactive: bool, asynchronous: bool) -> (App, egui::Context, Uuid, Option<Uuid>) {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    let other = inactive.then(|| {
        let id = app.active_tab_id();
        app.new_document();
        id
    });
    app.editor = Editor::with_document(crate::editing::text_submission_tests::boundary(), None);
    app.editor.test_persistent_validator(Some(
        paintcraft_formats::pcnd::check_compact_metadata_admission,
    ));
    let ctx = egui::Context::default();
    if asynchronous {
        app.initialize_editing(ctx.clone());
    }
    let owner = app.active_tab_id();
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((2, 2));
    app.editor.pointer_up();
    app.editor.type_text("Ink");
    (app, ctx, owner, other)
}
#[test]
fn save_and_switch_refuses_failed_retained_text_on_active_and_inactive_tabs() {
    for asynchronous in [false, true] {
        for inactive in [false, true] {
            let (mut app, ctx, owner, other) = source(inactive, asynchronous);
            let before = app.editor.document_snapshot();
            let history = app.editor.history_view();
            app.editor.place_text();
            app.editor.run_pending();
            let until = Instant::now() + Duration::from_secs(10);
            while !app.editor.failed_text_retained() || !app.editor.can_capture_export() {
                app.poll_edits(&ctx);
                assert!(Instant::now() < until, "{}", app.editor.status());
                std::thread::sleep(Duration::from_millis(1));
            }
            if let Some(other) = other {
                app.activate_tab(other, &ctx);
            }
            let origin = app.active_tab_id();
            let count = app.document_count();
            app.begin_interface_switch(Experience::Classic, &ctx);
            assert!(
                app.switching.is_none(),
                "failed boundary must cancel Save-mode transition"
            );
            assert_eq!(app.experience(), Experience::Studio);
            assert_eq!(app.active_tab_id(), origin);
            assert_eq!(app.document_count(), count);
            assert!(app.editor.status().contains("Interface unchanged"));
            app.swap_tab_state(owner);
            assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
            assert_eq!(app.editor.history_view(), history);
            assert_eq!(app.editor.canvas_text_content(), Some("Ink"));
            assert!(app.editor.failed_text_retained());
        }
    }
}
#[test]
fn explicit_discard_and_switch_waits_for_actual_text_worker_retirement() {
    for inactive in [false, true] {
        let (mut app, ctx, owner, other) = source(inactive, true);
        let service = app.editing.clone().unwrap();
        let (prepared, release) = service.pause_next_completion();
        let before = app.editor.document_snapshot();
        app.editor.place_text();
        app.editor.run_pending();
        prepared.recv_timeout(Duration::from_secs(10)).unwrap();
        app.editor.type_text("!"); // Retire an already prepared older intention.
        assert!(app.editor.failed_text_retained());
        if let Some(other) = other {
            app.activate_tab(other, &ctx);
        }
        app.start_interface_switch(Experience::Classic, true, &ctx);
        assert_eq!(
            app.experience(),
            Experience::Studio,
            "cannot switch before old worker retires"
        );
        assert!(app.switching.is_some());
        app.swap_tab_state(owner);
        assert!(
            app.editor.canvas_text_content().is_none(),
            "only explicit Discard removes the draft"
        );
        assert!(app.editor.has_processing_edits());
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert!(
            app.worker_budget.usage().reserved > 0,
            "older worker still owns its reservation"
        );
        release.send(()).unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        while app.experience() != Experience::Classic {
            app.poll_edits(&ctx);
            app.pump_interface_switch(&ctx);
            assert!(Instant::now() < until, "{}", app.editor.status());
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(app.switching.is_none());
        assert!(!app.any_pending_edits());
        assert_eq!(
            app.worker_budget.usage().reserved,
            0,
            "retired worker releases its reservation before switching"
        );
    }
}
