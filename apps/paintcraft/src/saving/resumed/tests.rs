use super::*;
use crate::saving::tests::{drain, recovery_app, scratch};
use paintcraft_formats::{
    pcnd,
    save_recovery::{SavedAttempt, Stamp},
};

fn orphan(root: &Path) -> (Found, PathBuf) {
    let editor = Editor::new_document(13, 7).unwrap();
    let destination = root.join("saved.pcnd");
    let retained = SavedAttempt::capture(
        root,
        editor.document(),
        &destination,
        Stamp {
            session: Uuid::new_v4(),
            sequence: 29,
            generation: 4,
            operation: "save-as",
        },
    )
    .unwrap();
    let mut outcome = save::save_with_preparation(
        &destination,
        true,
        1 << 30,
        |sink| pcnd::write_to(editor.document(), sink),
        |outcome| retained.prepare(outcome),
    );
    assert!(outcome.is_success());
    outcome.durable = false;
    retained.finish(&outcome).unwrap();
    drop(retained);
    (Recovery::scan(root).pop().unwrap(), destination)
}
fn select(app: &mut App, found: &Found, selected: Option<PathBuf>) {
    app.saving.as_mut().unwrap().inspection.dialog = Some(Reacquire {
        scope: app.recovery_scope,
        found: found.clone(),
    });
    app.recovered_destination_selected(selected);
}

#[test]
fn inspection_and_confirmation_never_clean_or_retarget_the_current_document() {
    let root = scratch();
    let (found, destination) = orphan(&root);
    let before = save_recovery::read_record(&found)
        .unwrap()
        .get("receipt")
        .unwrap()
        .clone();
    let mut app = recovery_app(root.clone());
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let key = app.editor.render_key();
    app.path_text = "keep-this-field.pcnd".into();
    select(&mut app, &found, Some(destination.clone()));
    drain(&mut app);
    assert_eq!(app.editor.render_key(), key);
    assert!(app.editor.is_dirty());
    assert!(app.bound.is_none());
    assert_eq!(app.path_text, "keep-this-field.pcnd");
    app.saving
        .as_mut()
        .unwrap()
        .inspect_recovered(
            app.recovery_scope,
            found.clone(),
            destination,
            save::ReconcileMode::ConfirmDurability,
        )
        .unwrap();
    drain(&mut app);
    assert!(app.editor.is_dirty());
    assert!(app.bound.is_none());
    assert_eq!(app.editor.render_key(), key);
    let saving = app.saving.as_ref().unwrap();
    assert!(
        saving.last_receipt.is_none(),
        "a prior-process receipt is not the live session's receipt"
    );
    let report = &saving.inspection.report.as_ref().unwrap().result;
    assert_eq!(
        report.receipt.get("durability"),
        Some(&Value::str("confirmed"))
    );
    for key in [
        "operation_id",
        "source_state_id",
        "save_sequence",
        "binding_generation",
    ] {
        assert_eq!(before.get(key), report.receipt.get(key));
    }
    assert!(found.checkpoint.exists());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn picker_cancellation_and_stale_configuration_do_not_submit_io_or_change_the_path_field() {
    let root = scratch();
    let (found, destination) = orphan(&root);
    let mut app = recovery_app(root.clone());
    let text = app.path_text.clone();
    select(&mut app, &found, None);
    assert!(!app.saving.as_ref().unwrap().pending());
    assert_eq!(app.path_text, text);
    app.saving.as_mut().unwrap().inspection.dialog = Some(Reacquire {
        scope: app.recovery_scope,
        found,
    });
    app.recovery_scope = Uuid::new_v4();
    app.recovered_destination_selected(Some(destination));
    assert!(!app.saving.as_ref().unwrap().pending());
    assert_eq!(app.path_text, text);
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn late_results_are_not_presented_after_the_offer_is_hidden() {
    let root = scratch();
    let (found, destination) = orphan(&root);
    let mut app = recovery_app(root.clone());
    select(&mut app, &found, Some(destination));
    app.saving.as_mut().unwrap().cancel_inspection();
    app.recoverable.clear();
    drain(&mut app);
    assert!(app.saving.as_ref().unwrap().inspection.report.is_none());
    assert!(found.checkpoint.exists());
    assert!(app.bound.is_none());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn durability_confirmation_runs_through_real_recovery_widgets() {
    use egui_kittest::{Harness, kittest::Queryable};
    let root = scratch();
    let (found, destination) = orphan(&root);
    let mut app = recovery_app(root.clone());
    select(&mut app, &found, Some(destination));
    drain(&mut app);
    let key = app.editor.render_key();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1100.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    harness.run();
    harness
        .get_by_label("Confirm destination durability")
        .click();
    harness.step();
    drain(harness.state_mut());
    assert_eq!(harness.state().editor.render_key(), key);
    assert!(harness.state().bound.is_none());
    assert_eq!(
        harness
            .state()
            .saving
            .as_ref()
            .unwrap()
            .inspection
            .report
            .as_ref()
            .unwrap()
            .result
            .receipt
            .get("durability"),
        Some(&Value::str("confirmed"))
    );
    assert!(found.checkpoint.exists());
    drop(harness);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_new_editor_session_suppresses_an_old_inspection_completion() {
    let root = scratch();
    let (found, destination) = orphan(&root);
    let mut app = recovery_app(root.clone());
    select(&mut app, &found, Some(destination));
    // Consume the exclusive read reservation but defer presentation, matching
    // the worker-completion/publication boundary under a new editor session.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let (request, result) = loop {
        if let Some((_, PendingKind::Resumed(request), result)) =
            app.saving.as_mut().unwrap().poll_completion()
        {
            break (request, result);
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    };
    app.install(Editor::new_document(7, 9).unwrap()).unwrap();
    app.complete_resumed_inspection(*request, result);
    drain(&mut app);
    assert_eq!(app.editor.document().canvas, (7, 9));
    assert!(app.saving.as_ref().unwrap().inspection.report.is_none());
    assert!(app.bound.is_none());
    assert!(found.checkpoint.exists());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn synchronous_recovery_discard_uses_the_saved_attempts_source_namespace() {
    use egui_kittest::{Harness, kittest::Queryable};
    let root = scratch();
    let (found, _) = orphan(&root);
    let app = App::new(None).with_recovery(root.clone());
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1100.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    harness.run();
    harness.get_by_label("Discard").click();
    harness.run();
    assert!(!found.checkpoint.exists());
    assert!(harness.state().recoverable.is_empty());
    drop(harness);
    std::fs::remove_dir_all(root).unwrap();
}
