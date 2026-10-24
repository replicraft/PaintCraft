use super::*;
use std::time::Instant;

fn frame(ctx: &egui::Context) {
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
}
fn setup(native: bool) -> (App, egui::Context, PathBuf, Vec<Uuid>) {
    let ctx = egui::Context::default();
    let dir = std::env::temp_dir().join(format!("paintcraft-save-all-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    if native {
        app.saving = Some(saving::NativeSaving::new(ctx.clone()).unwrap());
    }
    let mut ids = Vec::new();
    for number in 0..3 {
        if number != 0 {
            app.new_document();
        }
        app.editor.request(number + 1, Action::AddLayer);
        app.editor.run_pending();
        app.path_text = dir
            .join(format!("doc-{number}.pcnd"))
            .to_string_lossy()
            .into_owned();
        ids.push(app.active_tab_id());
    }
    (app, ctx, dir, ids)
}
fn pump(app: &mut App, ctx: &egui::Context) {
    let start = Instant::now();
    while app.save_all.is_some() {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "{}",
            app.editor.status()
        );
        app.editor.run_pending();
        app.finish_save();
        app.poll_native_saves(ctx);
        app.poll_edits(ctx);
        app.pump_save_all(ctx);
        frame(ctx);
        std::thread::yield_now();
    }
}

#[test]
fn saves_each_dirty_tab_with_exact_snapshots_and_restores_the_origin() {
    for native in [false, true] {
        let (mut app, ctx, dir, ids) = setup(native);
        let origin = app.active_tab_id();
        let mut before = Vec::new();
        for id in &ids {
            app.swap_tab_state(*id);
            before.push((
                app.editor.document_snapshot(),
                app.editor.history_view().ticket,
            ));
        }
        app.zoom = 0.375;
        app.begin_save_all(&ctx);
        assert!(app.action_context().blocked);
        pump(&mut app, &ctx);
        assert_eq!(app.active_tab_id(), origin);
        assert_eq!(app.zoom, 0.375);
        assert!(!app.any_dirty_documents());
        assert!(!app.action_context().can_save_all);
        assert_eq!(app.editor.status(), "All modified documents saved.");
        for (number, id) in ids.iter().enumerate() {
            app.swap_tab_state(*id);
            assert_eq!(app.editor.history_view().ticket, before[number].1);
            assert!(
                Editor::open(&dir.join(format!("doc-{number}.pcnd")))
                    .unwrap()
                    .document()
                    .same_semantics(&before[number].0)
            );
            assert!(!app.editor.is_dirty());
        }
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn failure_stops_before_later_documents_and_does_not_close_the_window() {
    for native in [false, true] {
        let (mut app, ctx, dir, ids) = setup(native);
        app.swap_tab_state(ids[1]);
        app.path_text = dir.join("missing/work.pcnd").to_string_lossy().into_owned();
        app.swap_tab_state(ids[2]);
        app.begin_save_all(&ctx);
        app.save_all.as_mut().unwrap().close_after = true;
        pump(&mut app, &ctx);
        assert_eq!(app.active_tab_id(), ids[2]);
        assert!(dir.join("doc-0.pcnd").exists());
        assert!(!dir.join("doc-2.pcnd").exists());
        assert!(app.editor.status().starts_with("Save All stopped"));
        assert!(!app.closing_workspace);
        app.swap_tab_state(ids[0]);
        assert!(!app.editor.is_dirty());
        app.swap_tab_state(ids[1]);
        assert!(app.editor.is_dirty());
        app.swap_tab_state(ids[2]);
        assert!(app.editor.is_dirty());
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn cancelled_overwrite_keeps_the_existing_file_and_later_tabs_untouched() {
    let (mut app, ctx, dir, ids) = setup(false);
    std::fs::write(dir.join("doc-1.pcnd"), b"unrelated existing bytes").unwrap();
    app.begin_save_all(&ctx);
    app.editor.run_pending();
    app.finish_save();
    app.pump_save_all(&ctx);
    assert!(
        matches!(app.confirm, Some(Confirm::ReplaceFile(_))),
        "confirm {:?}, status {}, state {:?}",
        app.confirm,
        app.editor.status(),
        app.save_all
    );
    app.confirm = None; // Cancel the existing shared overwrite confirmation.
    pump(&mut app, &ctx);
    assert_eq!(app.active_tab_id(), ids[2]);
    assert_eq!(
        std::fs::read(dir.join("doc-1.pcnd")).unwrap(),
        b"unrelated existing bytes"
    );
    assert!(!dir.join("doc-2.pcnd").exists());
    assert!(app.any_dirty_documents());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_cancelled_save_as_completion_stops_without_touching_later_tabs() {
    let (mut app, ctx, dir, ids) = setup(false);
    app.swap_tab_state(ids[0]);
    app.path_text.clear();
    app.swap_tab_state(ids[2]);
    app.begin_save_all(&ctx);
    app.dialog_owner = Some(ids[0]);
    app.complete_dialog(
        &ctx,
        paintcraft_platform::dialogs::Completion {
            kind: Kind::SaveAs,
            path: None,
        },
    );
    pump(&mut app, &ctx);
    assert_eq!(app.active_tab_id(), ids[2]);
    assert!(!dir.join("doc-1.pcnd").exists());
    assert!(!dir.join("doc-2.pcnd").exists());
    assert!(app.any_dirty_documents());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_newer_edit_after_save_capture_remains_dirty_and_stops_the_batch() {
    for native in [false, true] {
        let (mut app, ctx, dir, ids) = setup(native);
        app.begin_save_all(&ctx);
        assert_eq!(app.active_tab_id(), ids[0]);
        let captured = app.editor.document_snapshot();
        app.editor.request(999, Action::AddLayer);
        app.editor.run_pending();
        pump(&mut app, &ctx);
        assert_eq!(app.active_tab_id(), ids[2]);
        app.swap_tab_state(ids[0]);
        assert!(app.editor.is_dirty());
        assert!(
            Editor::open(&dir.join("doc-0.pcnd"))
                .unwrap()
                .document()
                .same_semantics(&captured)
        );
        assert!(!dir.join("doc-1.pcnd").exists());
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn clean_documents_are_skipped_even_if_their_bound_file_changed_externally() {
    let (mut app, ctx, dir, ids) = setup(false);
    app.swap_tab_state(ids[0]);
    app.save_requested(&ctx);
    app.editor.run_pending();
    app.finish_save();
    assert!(
        !app.editor.is_dirty(),
        "status {}, pending {:?}",
        app.editor.status(),
        app.pending_save
    );
    let path = dir.join("doc-0.pcnd");
    std::fs::write(&path, b"external replacement").unwrap();
    frame(&ctx);
    app.swap_tab_state(ids[2]);
    app.begin_save_all(&ctx);
    pump(&mut app, &ctx);
    assert_eq!(std::fs::read(path).unwrap(), b"external replacement");
    assert!(!app.any_dirty_documents());
    assert!(dir.join("doc-1.pcnd").exists());
    assert!(dir.join("doc-2.pcnd").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_as_completion_returns_to_its_owner_before_the_batch_advances() {
    let (mut app, ctx, dir, ids) = setup(false);
    app.swap_tab_state(ids[0]);
    app.path_text.clear();
    let source = app.editor.document_snapshot();
    app.swap_tab_state(ids[2]);
    app.begin_save_all(&ctx);
    app.dialog_owner = Some(ids[0]);
    app.swap_tab_state(ids[2]);
    app.complete_dialog(
        &ctx,
        paintcraft_platform::dialogs::Completion {
            kind: Kind::SaveAs,
            path: Some(dir.join("chosen.pcnd")),
        },
    );
    pump(&mut app, &ctx);
    assert_eq!(app.active_tab_id(), ids[2]);
    assert!(
        Editor::open(&dir.join("chosen.pcnd"))
            .unwrap()
            .document()
            .same_semantics(&source)
    );
    assert!(dir.join("doc-1.pcnd").exists());
    assert!(dir.join("doc-2.pcnd").exists());
    assert!(!app.any_dirty_documents());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn close_during_save_all_waits_for_every_document_before_closing() {
    for native in [false, true] {
        let (mut app, ctx, dir, _) = setup(native);
        app.begin_save_all(&ctx);
        app.save_all.as_mut().unwrap().close_after = true;
        pump(&mut app, &ctx);
        let deadline = Instant::now() + Duration::from_secs(15);
        while !app.closing_workspace {
            assert!(Instant::now() < deadline, "{}", app.editor.status());
            app.poll_native_saves(&ctx);
            frame(&ctx);
            std::thread::yield_now();
        }
        assert!(!app.any_dirty_documents());
        for number in 0..3 {
            assert!(dir.join(format!("doc-{number}.pcnd")).exists());
        }
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
