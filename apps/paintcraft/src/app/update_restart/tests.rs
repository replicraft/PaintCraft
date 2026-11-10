use super::*;

#[test]
fn prompt_cancel_restores_exact_unapplied_settings_and_dirty_document() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let document = app.editor.document_snapshot();
    let settings = app.settings.clone();
    app.open_settings(&ctx);
    let Some(Confirm::Settings(mut draft)) = app.confirm.take() else {
        panic!("settings draft");
    };
    draft.values.experience = Experience::Studio;
    let expected = draft.clone();
    app.update_restart = Some(State::Prompt {
        settings: Some(draft),
    });
    app.cancel_update_restart(&ctx);
    assert!(app.update_restart.is_none());
    assert_eq!(app.confirm, Some(Confirm::Settings(expected)));
    assert_eq!(app.settings, settings);
    assert!(app.editor.document().same_semantics(&document));
    assert!(app.editor.is_dirty());
    assert!(!app.allow_close);
}

#[test]
fn classic_and_studio_save_gate_use_actual_files_without_quit_anyway() {
    for experience in [Experience::Classic, Experience::Studio] {
        let ctx = egui::Context::default();
        let directory =
            std::env::temp_dir().join(format!("paintcraft-update-save-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut app = App::new(None);
        app.settings.experience = experience;
        app.path_text = directory.join("work.pcnd").to_string_lossy().into_owned();
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        let before = app.editor.document_snapshot();
        app.update_restart = Some(State::Prompt { settings: None });
        app.begin_update_saves(&ctx);
        for _ in 0..12 {
            if !matches!(app.update_restart, Some(State::Saving { .. })) {
                break;
            }
            let mut output = ctx.run_ui(Default::default(), |ui| app.pump_update_saves(ui));
            output.textures_delta.clear();
        }
        assert!(matches!(app.update_restart, Some(State::Draining(None))));
        assert!(!app.any_dirty_documents());
        assert!(!app.allow_close);
        assert!(
            Editor::open(&directory.join("work.pcnd"))
                .unwrap()
                .document()
                .same_semantics(&before)
        );
        assert!(app.installation_lock.is_none());
        assert!(!app.updates.verified_ready());
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn failed_save_never_reaches_drain_or_discards_modified_document() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.path_text = format!("/missing-paintcraft-{}/work.pcnd", Uuid::new_v4());
    app.editor.request(1, Action::AddLayer);
    app.editor.run_pending();
    let before = app.editor.document_snapshot();
    app.update_restart = Some(State::Prompt { settings: None });
    app.begin_update_saves(&ctx);
    for _ in 0..8 {
        if app.update_restart.is_none() {
            break;
        }
        let mut output = ctx.run_ui(Default::default(), |ui| app.pump_update_saves(ui));
        output.textures_delta.clear();
    }
    assert!(app.update_restart.is_none());
    assert!(app.editor.is_dirty());
    assert!(app.editor.document().same_semantics(&before));
    assert!(!app.allow_close);
}

#[test]
fn parked_modified_workspace_is_not_covered_by_active_only_dirty_check() {
    let mut app = App::new(None);
    let mut parked = tabs::WorkspaceDocuments::new((2, 2), false);
    parked.active.editor.request(1, Action::AddLayer);
    parked.active.editor.run_pending();
    app.other_workspace = Some(Box::new(parked));
    assert!(!app.any_dirty_documents());
    assert!(!app.update_parked_idle());
}

#[test]
fn native_close_during_restart_cannot_enter_quit_anyway() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.update_restart = Some(State::Prompt { settings: None });
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let mut output = ctx.run_ui(input, |ui| app.update_restart_ui(ui));
    output.textures_delta.clear();
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose)
    );
    assert!(!matches!(app.confirm, Some(Confirm::QuitUnsaved)));
    assert!(!app.allow_close);
}

mod native_saving {
    use super::*;
    use std::time::{Duration, Instant};
    const DEADLINE: Duration = Duration::from_secs(5);

    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    // App (and its actual FIFO worker) retires before the final directory owner.
    struct Fixture {
        app: App,
        ctx: egui::Context,
        ids: Vec<Uuid>,
        sources: Vec<Arc<paintcraft_document::Document>>,
        directory: Directory,
    }
    impl Fixture {
        fn new(experience: Experience) -> Self {
            let ctx = egui::Context::default();
            let directory = Directory(
                std::env::temp_dir().join(format!("paintcraft-update-native-{}", Uuid::new_v4())),
            );
            std::fs::create_dir_all(&directory.0).unwrap();
            let mut app = App::new(None);
            app.settings.experience = experience;
            app.saving = Some(saving::NativeSaving::new(ctx.clone()).unwrap());
            let mut ids = Vec::new();
            let mut sources = Vec::new();
            for index in 0..2 {
                if index != 0 {
                    app.add_document_tab(Editor::new_document(3, 2).unwrap(), &ctx)
                        .unwrap();
                }
                app.editor.request(index + 1, Action::AddLayer);
                app.editor.run_pending();
                app.path_text = directory
                    .0
                    .join(format!("document-{index}.pcnd"))
                    .to_string_lossy()
                    .into_owned();
                ids.push(app.active_tab_id());
                sources.push(app.editor.document_snapshot());
            }
            Self {
                app,
                ctx,
                ids,
                sources,
                directory,
            }
        }
        fn path(&self, index: usize) -> PathBuf {
            self.directory.0.join(format!("document-{index}.pcnd"))
        }
        fn begin(&mut self) {
            self.app.update_restart = Some(State::Prompt { settings: None });
            self.app.begin_update_saves(&self.ctx);
            assert!(matches!(
                self.app.update_restart,
                Some(State::Saving { .. })
            ));
            assert!(!self.app.allow_close);
            assert!(!self.app.updates.verified_ready());
        }
        fn pump(&mut self) {
            let start = Instant::now();
            while matches!(self.app.update_restart, Some(State::Saving { .. })) {
                assert!(
                    start.elapsed() < DEADLINE,
                    "native restart save gate timed out: {}",
                    self.app.editor.status()
                );
                let mut output = self
                    .ctx
                    .run_ui(Default::default(), |ui| self.app.pump_update_saves(ui));
                output.textures_delta.clear();
                std::thread::yield_now();
            }
            assert!(!self.app.saving.as_ref().unwrap().pending());
        }
    }

    #[test]
    fn real_fifo_saves_both_owner_bound_tabs_before_drain_in_classic_and_studio() {
        for experience in [Experience::Classic, Experience::Studio] {
            let mut f = Fixture::new(experience);
            assert_eq!(f.app.document_count(), 2);
            let origin = f.app.active_tab_id();
            let mut tickets = Vec::new();
            for id in &f.ids {
                assert!(f.app.swap_tab_state(*id));
                tickets.push(f.app.editor.history_view().ticket);
            }
            assert!(f.app.swap_tab_state(origin));
            f.begin();
            // Completion remains owned by the real I/O queue even if the OS
            // write has already finished. No UI adoption or synchronous fallback.
            assert!(f.app.saving.as_ref().unwrap().pending());
            assert!(f.app.any_dirty_documents());
            f.pump();
            assert!(matches!(f.app.update_restart, Some(State::Draining(None))));
            assert_eq!(f.app.active_tab_id(), origin);
            assert!(!f.app.any_dirty_documents());
            assert!(!f.app.saving.as_ref().unwrap().failed());
            for (index, ticket) in tickets.iter().enumerate() {
                assert!(f.app.swap_tab_state(f.ids[index]));
                assert_eq!(f.app.editor.history_view().ticket, *ticket);
                assert!(!f.app.editor.is_dirty());
                let saved = Editor::open(&f.path(index)).unwrap();
                assert!(saved.document().same_semantics(&f.sources[index]));
                assert_eq!(saved.document().document_id, f.sources[index].document_id);
                assert_eq!(saved.document().state_id, f.sources[index].state_id);
            }
            assert!(!f.app.allow_close);
            assert!(!f.app.closing_workspace);
            assert!(f.app.installation_lock.is_none());
        }
    }

    #[test]
    fn real_fifo_write_failure_preserves_dirty_source_and_never_enters_drain() {
        for experience in [Experience::Classic, Experience::Studio] {
            let mut f = Fixture::new(experience);
            let bad = f.directory.0.join("missing/document.pcnd");
            f.app.path_text = bad.to_string_lossy().into_owned();
            let origin = f.app.active_tab_id();
            f.begin();
            assert!(f.app.saving.as_ref().unwrap().pending());
            f.pump();
            assert!(f.app.update_restart.is_none());
            assert_eq!(f.app.active_tab_id(), origin);
            assert!(f.app.saving.as_ref().unwrap().failed());
            assert!(f.path(0).is_file());
            assert!(!bad.exists());
            assert!(!f.path(1).exists());
            assert!(f.app.editor.is_dirty());
            assert!(f.app.editor.document().same_semantics(&f.sources[1]));
            assert!(f.app.any_dirty_documents());
            assert!(!f.app.allow_close);
            assert!(!f.app.closing_workspace);
            assert!(!f.app.updates.verified_ready());
        }
    }

    #[test]
    fn cancelled_overwrite_with_native_coordinator_keeps_bytes_and_later_tab_dirty() {
        for experience in [Experience::Classic, Experience::Studio] {
            let mut f = Fixture::new(experience);
            let existing = b"original unrelated destination bytes";
            std::fs::write(f.path(0), existing).unwrap();
            f.begin();
            assert!(matches!(f.app.confirm, Some(Confirm::ReplaceFile(_))));
            assert!(!f.app.saving.as_ref().unwrap().pending());
            f.app.confirm = None; // The existing overwrite Cancel path.
            f.pump();
            assert!(f.app.update_restart.is_none());
            assert_eq!(std::fs::read(f.path(0)).unwrap(), existing);
            assert!(!f.path(1).exists());
            for index in 0..2 {
                assert!(f.app.swap_tab_state(f.ids[index]));
                assert!(f.app.editor.is_dirty());
                assert!(f.app.editor.document().same_semantics(&f.sources[index]));
            }
            assert!(!f.app.allow_close);
            assert!(!f.app.closing_workspace);
        }
    }
}
