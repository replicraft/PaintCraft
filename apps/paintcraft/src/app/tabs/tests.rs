use super::*;
use std::time::Instant;

fn studio() -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app
}
fn paint(app: &mut App, x: i64) {
    let layer = app.editor.target().unwrap();
    app.editor.request(
        100 + x as u64,
        Action::Stroke {
            layer,
            points: vec![(x, 5)],
            color: Rgba([10, 20, 30, 255]),
            size: 1,
            round: false,
        },
    );
    app.editor.run_pending();
}
fn drain(app: &mut App, ctx: &egui::Context) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while app
        .saving
        .as_ref()
        .is_some_and(saving::NativeSaving::pending)
    {
        assert!(Instant::now() < deadline);
        app.poll_native_saves(ctx);
        std::thread::yield_now();
    }
}

#[test]
fn five_documents_keep_their_editors_history_bindings_and_zoom_across_modes() {
    let mut app = studio();
    let ctx = egui::Context::default();
    let mut documents = Vec::new();
    for i in 0..5 {
        if i > 0 {
            app.new_document();
        }
        paint(&mut app, i + 1);
        app.zoom = 1u32 << i;
        app.bound = Some(PathBuf::from(format!("doc-{i}.pcnd")));
        documents.push((
            app.active_tab_id(),
            app.editor.document_snapshot(),
            app.editor.history_view().ticket,
            app.zoom,
            app.bound.clone(),
        ));
    }
    assert_eq!(app.document_count(), 5);
    assert_eq!(
        documents
            .iter()
            .map(|(_, d, ..)| d.document_id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        5
    );
    app.choose_experience(Experience::Classic);
    app.choose_variant(ClassicVariant::V11);
    for (id, doc, ticket, zoom, bound) in &documents {
        app.activate_tab(*id, &ctx);
        assert!(app.editor.document().same_semantics(doc));
        assert_eq!(app.editor.history_view().ticket, *ticket);
        assert_eq!(app.zoom, *zoom);
        assert_eq!(&app.bound, bound);
    }
    app.choose_experience(Experience::Studio);
    assert_eq!(app.document_count(), 5);
}

#[test]
fn inactive_save_completion_binds_and_acknowledges_only_its_originating_document() {
    let mut app = studio();
    let ctx = egui::Context::default();
    app.saving = Some(saving::NativeSaving::new(ctx.clone()).unwrap());
    paint(&mut app, 1);
    let a = app.active_tab_id();
    let snapshot = app.editor.document_snapshot();
    let dir = std::env::temp_dir().join(format!("paintcraft-tabs-save-{}", Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("a.pcnd");
    app.do_save(&ctx, path.clone());
    app.new_document();
    paint(&mut app, 2);
    let b = app.active_tab_id();
    let b_key = app.editor.history_view().ticket;
    drain(&mut app, &ctx);
    assert_eq!(app.active_tab_id(), b);
    assert_eq!(app.editor.history_view().ticket, b_key);
    assert!(app.editor.is_dirty());
    assert!(app.bound.is_none());
    app.activate_tab(a, &ctx);
    assert!(!app.editor.is_dirty());
    assert_eq!(
        app.bound.as_ref(),
        Some(&std::fs::canonicalize(&path).unwrap())
    );
    assert!(app.editor.document().same_semantics(&snapshot));
    assert!(
        paintcraft_formats::pcnd::read(&std::fs::read(&path).unwrap())
            .unwrap()
            .same_semantics(&snapshot)
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn dirty_inactive_documents_prevent_an_unguarded_window_close() {
    let mut app = studio();
    let ctx = egui::Context::default();
    paint(&mut app, 1);
    app.new_document();
    assert!(!app.editor.is_dirty());
    assert!(app.any_dirty_documents());
    app.close_after_requested_saves(&ctx);
    assert_eq!(app.confirm, Some(Confirm::QuitUnsaved));
    assert_eq!(app.document_count(), 2);
}

#[test]
fn closing_the_last_clean_tab_leaves_an_empty_workspace_and_new_works_again() {
    let mut app = studio();
    let ctx = egui::Context::default();
    app.request_close_tab(app.active_tab_id(), &ctx);
    assert_eq!(app.document_count(), 0);
    assert!(!app.any_dirty_documents());
    app.new_document();
    assert_eq!(app.document_count(), 1);
    assert!(!app.editor.is_dirty());
}

#[test]
fn background_documents_keep_independent_recovery_checkpoints() {
    let mut app = studio();
    let ctx = egui::Context::default();
    app.saving = Some(saving::NativeSaving::new(ctx.clone()).unwrap());
    let dir = std::env::temp_dir().join(format!("paintcraft-tabs-recovery-{}", Uuid::new_v4()));
    app = app.with_recovery(dir.clone());
    paint(&mut app, 1);
    let a = app.active_tab_id();
    let snapshot_a = app.editor.document_snapshot();
    app.last_autosave = -100.0;
    app.new_document();
    paint(&mut app, 2);
    let b = app.active_tab_id();
    let snapshot_b = app.editor.document_snapshot();
    app.last_autosave = -100.0;
    drain(&mut app, &ctx);
    app.tick_documents(&ctx);
    drain(&mut app, &ctx);
    assert_eq!(app.active_tab_id(), b);
    assert!(app.editor.checkpoint_info().is_some());
    app.activate_tab(a, &ctx);
    assert!(app.editor.checkpoint_info().is_some());
    assert!(app.editor.document().same_semantics(&snapshot_a));
    assert!(
        Recovery::scan(&dir).is_empty(),
        "both live owners remain locked"
    );
    drop(app);
    let recovered = Recovery::scan(&dir)
        .into_iter()
        .map(|f| Recovery::read_checkpoint(&f.checkpoint).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(recovered.len(), 2);
    assert!(recovered.iter().any(|d| d.same_semantics(&snapshot_a)));
    assert!(recovered.iter().any(|d| d.same_semantics(&snapshot_b)));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_and_close_can_finish_in_the_background_without_closing_the_foreground_tab() {
    let mut app = studio();
    let ctx = egui::Context::default();
    app.saving = Some(saving::NativeSaving::new(ctx.clone()).unwrap());
    paint(&mut app, 1);
    let a = app.active_tab_id();
    let source = app.editor.document().state_id;
    let dir = std::env::temp_dir().join(format!("paintcraft-tab-close-save-{}", Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    app.tab_save_close = Some((a, source));
    app.do_save(&ctx, dir.join("a.pcnd"));
    app.new_document();
    paint(&mut app, 2);
    let b = app.active_tab_id();
    let snapshot = app.editor.document_snapshot();
    drain(&mut app, &ctx);
    app.pump_tab_close(&ctx);
    assert_eq!(app.document_count(), 1);
    assert_eq!(app.active_tab_id(), b);
    assert!(app.editor.document().same_semantics(&snapshot));
    assert!(app.editor.is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn new_drafts_after_save_and_close_keep_the_document_open() {
    let mut app = studio();
    let ctx = egui::Context::default();
    app.saving = Some(saving::NativeSaving::new(ctx.clone()).unwrap());
    paint(&mut app, 1);
    let id = app.active_tab_id();
    let source = app.editor.document().state_id;
    let dir = std::env::temp_dir().join(format!("paintcraft-tab-new-drafts-{}", Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    app.tab_save_close = Some((id, source));
    app.do_save(&ctx, dir.join("a.pcnd"));
    let layer = app.editor.target().unwrap();
    app.name_edit = Some((layer, "Uncommitted name".into()));
    drain(&mut app, &ctx);
    app.pump_tab_close(&ctx);
    assert_eq!(app.document_count(), 1);
    assert_eq!(app.name_edit, Some((layer, "Uncommitted name".into())));
    assert!(app.tab_save_close.is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cross_tab_saves_share_fifo_order_for_the_same_destination() {
    let mut app = studio();
    let ctx = egui::Context::default();
    app.saving = Some(saving::NativeSaving::new(ctx.clone()).unwrap());
    let dir = std::env::temp_dir().join(format!("paintcraft-tabs-fifo-{}", Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("ordered.pcnd");
    paint(&mut app, 1);
    let a = app.active_tab_id();
    let original = app.editor.document_snapshot();
    app.do_save(&ctx, path.clone());
    app.new_document();
    paint(&mut app, 2);
    let expected = app.editor.document_snapshot();
    app.save_selected(&ctx, path.clone());
    assert_eq!(
        app.confirm,
        Some(Confirm::ReplaceSharedFile(path.clone())),
        "a second tab also confirms a destination still pending in the queue"
    );
    app.confirm = None;
    app.do_save(&ctx, path.clone());
    drain(&mut app, &ctx);
    let written = paintcraft_formats::pcnd::read(&std::fs::read(path).unwrap()).unwrap();
    assert!(written.same_semantics(&expected));
    app.activate_tab(a, &ctx);
    assert!(
        app.editor.is_dirty(),
        "the first document no longer has matching content in the shared file"
    );
    assert!(app.editor.document().same_semantics(&original));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn recovered_document_opens_a_new_tab_without_replacing_unsaved_work() {
    let dir = std::env::temp_dir().join(format!("paintcraft-tabs-restore-{}", Uuid::new_v4()));
    let orphan = Recovery::start(&dir).unwrap();
    let doc = paintcraft_document::new_document(
        12,
        12,
        Some(Rgba([90, 80, 70, 255])),
        47,
        paintcraft_document::POLICY,
    )
    .unwrap();
    orphan.checkpoint_document(&doc, None).unwrap();
    drop(orphan);
    let offer = Recovery::scan(&dir).remove(0);
    let mut app = studio();
    let ctx = egui::Context::default();
    app.saving = Some(saving::NativeSaving::new(ctx.clone()).unwrap());
    app = app.with_recovery(dir.clone());
    drain(&mut app, &ctx);
    paint(&mut app, 1);
    let a = app.active_tab_id();
    let snapshot = app.editor.document_snapshot();
    app.do_recover(offer);
    drain(&mut app, &ctx);
    assert_eq!(app.document_count(), 2);
    assert_ne!(
        app.active_tab_id(),
        a,
        "a foreground restore selects its new document tab"
    );
    assert!(app.editor.document().same_semantics(&doc));
    assert!(app.editor.is_dirty());
    assert!(app.bound.is_none());
    app.activate_tab(a, &ctx);
    assert!(app.editor.document().same_semantics(&snapshot));
    assert!(app.editor.is_dirty());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn checkpoint_backpressure_retries_later_tabs_instead_of_starving_them() {
    let mut app = studio();
    let ctx = egui::Context::default();
    let dir = std::env::temp_dir().join(format!("paintcraft-tabs-fairness-{}", Uuid::new_v4()));
    app = app.with_recovery(dir.clone());
    for i in 0..9 {
        if i > 0 {
            app.new_document();
        }
        paint(&mut app, i + 1);
        app.last_autosave = -100.0;
    }
    // Retrofit the worker after creating live stores so this isolates checkpoint
    // queue admission from the separately tested initialization backpressure.
    app.saving = Some(saving::NativeSaving::new(ctx.clone()).unwrap());
    let original = app.active_tab_id();
    for _ in 0..4 {
        app.tick_documents(&ctx);
        drain(&mut app, &ctx);
    }
    assert_eq!(app.active_tab_id(), original);
    let ids = app.tabs.iter().map(|t| t.id).collect::<Vec<_>>();
    for id in ids {
        app.swap_tab_state(id);
        assert!(
            app.editor.checkpoint_info().is_some(),
            "every document gets a turn without advancing the clock"
        );
    }
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn opening_an_already_bound_file_selects_its_existing_tab() {
    let mut app = studio();
    let ctx = egui::Context::default();
    paint(&mut app, 1);
    let a = app.active_tab_id();
    let snapshot = app.editor.document_snapshot();
    let dir = std::env::temp_dir().join(format!("paintcraft-tabs-open-{}", Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("opened.pcnd");
    let doc =
        paintcraft_document::new_document(8, 8, None, 47, paintcraft_document::POLICY).unwrap();
    std::fs::write(&path, paintcraft_formats::pcnd::write(&doc).unwrap()).unwrap();
    app.open_path(&path).unwrap();
    let b = app.active_tab_id();
    let ticket = app.editor.history_view().ticket;
    assert_eq!(app.document_count(), 2);
    app.activate_tab(a, &ctx);
    assert!(app.editor.document().same_semantics(&snapshot));
    app.open_path(&dir.join(".").join("opened.pcnd")).unwrap();
    assert_eq!(app.document_count(), 2);
    assert_eq!(app.active_tab_id(), b);
    assert_eq!(app.editor.history_view().ticket, ticket);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn delayed_file_dialog_result_is_applied_to_its_originating_document() {
    let mut app = studio();
    let ctx = egui::Context::default();
    app.saving = Some(saving::NativeSaving::new(ctx.clone()).unwrap());
    paint(&mut app, 1);
    let a = app.active_tab_id();
    let snapshot = app.editor.document_snapshot();
    app.dialog_owner = Some(a);
    app.new_document();
    paint(&mut app, 2);
    let b = app.active_tab_id();
    let b_snapshot = app.editor.document_snapshot();
    let dir = std::env::temp_dir().join(format!("paintcraft-tabs-dialog-{}", Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("a.pcnd");
    app.complete_dialog(
        &ctx,
        paintcraft_platform::dialogs::Completion {
            kind: Kind::SaveAs,
            path: Some(path.clone()),
        },
    );
    drain(&mut app, &ctx);
    assert_eq!(app.active_tab_id(), a);
    assert!(app.editor.document().same_semantics(&snapshot));
    assert!(!app.editor.is_dirty());
    assert!(
        paintcraft_formats::pcnd::read(&std::fs::read(path).unwrap())
            .unwrap()
            .same_semantics(&snapshot)
    );
    app.activate_tab(b, &ctx);
    assert!(app.bound.is_none());
    assert!(app.editor.document().same_semantics(&b_snapshot));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn startup_recovery_offers_remain_workspace_wide_after_switching_or_closing_tabs() {
    let dir = std::env::temp_dir().join(format!("paintcraft-tabs-offers-{}", Uuid::new_v4()));
    let orphan = Recovery::start(&dir).unwrap();
    let doc =
        paintcraft_document::new_document(8, 8, None, 81, paintcraft_document::POLICY).unwrap();
    orphan.checkpoint_document(&doc, None).unwrap();
    drop(orphan);
    let mut app = studio();
    let ctx = egui::Context::default();
    app.saving = Some(saving::NativeSaving::new(ctx.clone()).unwrap());
    app = app.with_recovery(dir.clone());
    let a = app.active_tab_id();
    app.new_document();
    let b = app.active_tab_id();
    drain(&mut app, &ctx);
    assert_eq!(app.active_tab_id(), b);
    assert_eq!(app.recoverable.len(), 1);
    app.request_close_tab(a, &ctx);
    drain(&mut app, &ctx);
    app.pump_tab_close(&ctx);
    assert_eq!(app.document_count(), 1);
    assert_eq!(
        app.recoverable.len(),
        1,
        "closing the discovering document does not hide recovery offers"
    );
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
