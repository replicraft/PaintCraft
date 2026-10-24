//! Recent-file chooser uses the ordinary owner/dirty guarded Open path.
use super::*;
use paintcraft_ui::t;
#[path = "recent/store.rs"]
mod store;
pub(super) use store::History;

impl App {
    pub(super) fn begin_recent_files(&mut self, ctx: &egui::Context) {
        if !self.editor.shape_file_ready() {
            return;
        }
        self.resolve_tab_input(ctx);
        self.confirm = Some(Confirm::RecentFiles);
    }
    pub(super) fn recent_files_dialog(&mut self, ctx: &egui::Context) {
        let mut selected = None;
        let mut clear = false;
        let mut close = false;
        let ready = !self.editor.has_pending_edits()
            && self.saving.as_ref().is_none_or(|saving| !saving.pending());
        let width = (ctx.content_rect().width() - 64.0).clamp(240.0, 560.0);
        let height = (ctx.content_rect().height() - 128.0).clamp(100.0, 480.0);
        let response = egui::Modal::new(egui::Id::unique("recent-files")).show(ctx, |ui| {
            ui.set_width(width);
            ui.heading(t!(ctx, "Recent files"));
            if let Some(detail) = &self.recent.warning {
                ui.label(t!(
                    ctx,
                    "Recent file history is unavailable: { $detail }",
                    detail = detail
                ));
            }
            if self.recent.entries.is_empty() {
                ui.label(t!(ctx, "No recent files yet."));
            }
            if !ready {
                ui.label(t!(ctx, "Waiting for pending work…"));
            }
            egui::ScrollArea::vertical()
                .max_height(height)
                .show(ui, |ui| {
                    for path in &self.recent.entries {
                        ui.push_id(path, |ui| {
                            let name = path
                                .file_name()
                                .unwrap_or(path.as_os_str())
                                .to_string_lossy();
                            let label = format!(
                                "{}\n{}",
                                name,
                                path.parent().unwrap_or(Path::new("")).display()
                            );
                            if ui
                                .add_enabled(
                                    ready,
                                    egui::Button::new(label)
                                        .min_size(Vec2::new(width, 40.0))
                                        .wrap(),
                                )
                                .on_hover_text(path.display().to_string())
                                .clicked()
                            {
                                selected = Some(path.clone());
                            }
                        });
                    }
                });
            ui.separator();
            ui.horizontal(|ui| {
                clear = ui
                    .add_enabled(
                        !self.recent.entries.is_empty() || self.recent.warning.is_some(),
                        egui::Button::new(t!(ctx, "Clear recent history")),
                    )
                    .clicked();
                close = ui.button(t!(ctx, "Cancel")).clicked();
            });
        });
        if ctx.current_pass_index() != 0 {
            return;
        }
        if clear {
            self.recent.clear();
        }
        if let Some(path) = selected {
            self.confirm = None;
            self.open_selected(path);
        } else if close || response.should_close() {
            self.confirm = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (App, PathBuf) {
        let dir = std::env::temp_dir().join(format!("paintcraft-recent-app-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = std::fs::canonicalize(dir).unwrap();
        let settings = Settings {
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        };
        settings.save(&dir).unwrap();
        (App::new(None).with_settings(dir.clone()), dir)
    }
    fn project(path: &Path) {
        let doc = paintcraft_document::new_document(
            7,
            5,
            Some(Rgba([20, 40, 80, 255])),
            31,
            paintcraft_document::POLICY,
        )
        .unwrap();
        std::fs::write(path, paintcraft_formats::pcnd::write(&doc).unwrap()).unwrap();
    }
    #[test]
    fn startup_open_persists_and_failed_open_keeps_document_and_history() {
        let (_, dir) = setup();
        let a = dir.join("first.pcnd");
        project(&a);
        let mut app = App::new(Some(a.clone())).with_settings(dir.clone());
        assert_eq!(app.recent.entries, vec![a.clone()]);
        assert_eq!(History::load(dir.clone()).entries, app.recent.entries);
        let snapshot = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        assert!(app.open_path(&dir.join("missing.pcnd")).is_err());
        assert!(Arc::ptr_eq(&snapshot, &app.editor.document_snapshot()));
        assert_eq!(ticket, app.editor.history_view().ticket);
        assert_eq!(app.recent.entries, vec![a]);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn imported_pictures_record_the_source_without_binding_or_rewriting_it() {
        let (mut app, dir) = setup();
        let path = dir.join("picture.png");
        let bytes = image::encode(
            ImageFormat::Png,
            2,
            1,
            &[12, 34, 56, 255, 80, 90, 100, 127],
            90,
        )
        .unwrap();
        std::fs::write(&path, &bytes).unwrap();
        app.open_path(&path).unwrap();
        assert!(app.bound.is_none());
        assert_eq!(app.recent.entries, vec![path.clone()]);
        assert_eq!(app.editor.document().canvas, (2, 1));
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn saves_enter_history_only_after_success_and_metadata_failures_do_not_fail_a_save() {
        let (mut app, dir) = setup();
        let path = dir.join("saved.pcnd");
        let ctx = egui::Context::default();
        app.save_selected(&ctx, path.clone());
        assert!(app.recent.entries.is_empty());
        app.editor.run_pending();
        app.finish_save();
        assert_eq!(app.recent.entries, vec![path.clone()]);
        let count = app.recent.entries.len();
        let mut frame = ctx.run_ui(Default::default(), |_| {});
        frame.textures_delta.clear();
        app.save_selected(&ctx, dir.join("missing/no.pcnd"));
        app.editor.run_pending();
        app.finish_save();
        assert_eq!(app.recent.entries.len(), count);
        // A metadata destination failure must not invalidate a successful document save.
        std::fs::remove_file(dir.join("recent-files.json")).unwrap();
        std::fs::create_dir(dir.join("recent-files.json")).unwrap();
        let second = dir.join("second.pcnd");
        let mut frame = ctx.run_ui(Default::default(), |_| {});
        frame.textures_delta.clear();
        app.save_selected(&ctx, second.clone());
        app.editor.run_pending();
        app.finish_save();
        assert_eq!(app.bound, Some(second.clone()));
        assert!(second.exists());
        assert!(!app.editor.is_dirty());
        assert!(app.recent.warning.is_some());
        assert_eq!(app.recent.entries[0], second);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn classic_recent_open_uses_dirty_guard_and_clearing_never_changes_work() {
        let (mut app, dir) = setup();
        let path = dir.join("picture.pcnd");
        project(&path);
        app.open_path(&path).unwrap();
        app.editor.request(501, Action::AddLayer);
        app.editor.run_pending();
        let snapshot = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        app.open_selected(path.clone());
        assert_eq!(app.confirm, Some(Confirm::DiscardForOpen(path)));
        assert!(Arc::ptr_eq(&snapshot, &app.editor.document_snapshot()));
        app.confirm = None;
        app.recent.clear();
        assert_eq!(ticket, app.editor.history_view().ticket);
        assert!(app.editor.is_dirty());
        assert!(Arc::ptr_eq(&snapshot, &app.editor.document_snapshot()));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn studio_reuses_an_open_binding_and_native_save_completions_record_success() {
        let (mut app, dir) = setup();
        app.settings.experience = Experience::Studio;
        let path = dir.join("studio.pcnd");
        project(&path);
        app.open_path(&path).unwrap();
        let owner = app.active_tab_id();
        let count = app.document_count();
        app.new_document();
        app.open_path(&path).unwrap();
        assert_eq!(app.active_tab_id(), owner);
        assert_eq!(app.document_count(), count + 1);
        assert_eq!(app.recent.entries, vec![path]);
        app.saving = Some(saving::NativeSaving::new(egui::Context::default()).unwrap());
        let saved = dir.join("native.pcnd");
        app.save_selected(&egui::Context::default(), saved.clone());
        assert!(!app.recent.entries.contains(&saved));
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while app.saving.as_ref().unwrap().pending() {
            app.poll_native_saves(&egui::Context::default());
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(app.recent.entries[0], saved);
        assert_eq!(History::load(dir.clone()).entries, app.recent.entries);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
