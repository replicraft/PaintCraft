//! Pressure maintenance visits inactive owners before the visible document.
use super::*;
use paintcraft_core::types::resources::BudgetUsage;
impl App {
    pub(super) fn retry_reclaimed_render(&mut self, usage: BudgetUsage) {
        if self
            .render_residency_retry
            .is_some_and(|(key, used)| self.texture_key == Some(key) && usage.reserved < used)
        {
            self.texture_key = None;
            self.render_residency_retry = None;
        }
    }
    pub(super) fn poll_residency_pressure(&mut self, ctx: &egui::Context) {
        self.poll_residency_usage(ctx, paintcraft_raster::Admission::new(0).residency_usage());
    }
    pub(super) fn poll_residency_usage(&mut self, ctx: &egui::Context, usage: BudgetUsage) {
        let Some(service) = self.editing.clone() else {
            return;
        };
        let target = (64 << 20).min(usage.limit / 4);
        let pressure = usage.limit.saturating_sub(usage.reserved) < target;
        let mut pending = false;
        let foreground = self.any_pending_edits();
        let mut completed = false;
        let mut visit = |editor: &mut Editor| {
            editor.connect_editing(service.clone());
            completed |= editor.poll_residency_maintenance();
            if pressure && (!foreground || !completed) {
                editor.start_residency_maintenance();
            }
            pending |= editor.has_pending_maintenance();
        };
        if let Some(workspace) = &mut self.other_workspace {
            for tab in &mut workspace.tabs {
                if let Some(state) = &mut tab.state {
                    visit(&mut state.editor);
                }
            }
            visit(&mut workspace.active.editor);
        }
        for tab in &mut self.tabs {
            if let Some(state) = &mut tab.state {
                visit(&mut state.editor);
            }
        }
        visit(&mut self.editor);
        if pending {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else if pressure {
            ctx.request_repaint_after(Duration::from_secs(5));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    fn paint(editor: &mut Editor, x: i64) {
        editor.pointer_down((x, 1));
        editor.pointer_up();
        editor.run_pending();
    }
    fn version(editor: &Editor) -> paintcraft_raster::TileVersion {
        editor.document().layers[&editor.target().unwrap()]
            .raster
            .tile_leases()
            .next()
            .unwrap()
            .1
            .version()
    }
    fn pressure() -> BudgetUsage {
        BudgetUsage {
            limit: 1 << 20,
            reserved: 1 << 20,
            peak: 1 << 20,
        }
    }
    fn pump(app: &mut App, ctx: &egui::Context, mut done: impl FnMut(&App) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            app.poll_edits(ctx);
            if done(app) {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    #[test]
    fn inactive_history_is_reclaimed_without_switching_tabs_or_losing_queued_foreground_paint() {
        let mut app = App::new(None);
        app.settings.experience = Experience::Studio;
        app.editor = Editor::new_document(8, 8).unwrap();
        let ctx = egui::Context::default();
        paint(&mut app.editor, 0);
        let old = version(&app.editor);
        paint(&mut app.editor, 1);
        paint(&mut app.editor, 2);
        let inactive = app.active_tab_id();
        let history = app.editor.history_view();
        let ticket = app.editor.render_key();
        app.editor
            .acknowledge_saved_state(app.editor.document().state_id);
        app.new_document();
        let active = app.active_tab_id();
        app.initialize_editing(ctx.clone());
        app.poll_residency_usage(&ctx, pressure());
        assert!(
            app.tabs
                .iter()
                .find(|tab| tab.id == inactive)
                .unwrap()
                .state
                .as_ref()
                .unwrap()
                .editor
                .has_pending_maintenance()
        );
        assert!(!app.editor.has_pending_maintenance());
        app.editor.pointer_down((3, 3));
        app.editor.pointer_up();
        // Keep pressure asserted while foreground input is queued. Completion
        // must yield the shared worker before starting another maintenance step.
        let deadline = Instant::now() + Duration::from_secs(10);
        while old.resident_bytes() != 0 {
            assert!(Instant::now() < deadline);
            app.poll_residency_usage(&ctx, pressure());
            std::thread::yield_now();
        }
        assert!(
            !app.tabs
                .iter()
                .find(|tab| tab.id == inactive)
                .unwrap()
                .state
                .as_ref()
                .unwrap()
                .editor
                .has_pending_maintenance()
        );
        pump(&mut app, &ctx, |app| !app.editor.has_pending_edits());
        assert_eq!(app.active_tab_id(), active);
        assert_eq!(app.editor.publications(), 1);
        assert_eq!(
            app.editor.document().layers[&app.editor.target().unwrap()]
                .raster
                .get(3, 3),
            Rgba([0, 0, 0, 255])
        );
        let owner = &app
            .tabs
            .iter()
            .find(|tab| tab.id == inactive)
            .unwrap()
            .state
            .as_ref()
            .unwrap()
            .editor;
        assert_eq!(owner.history_view(), history);
        assert_eq!(owner.render_key(), ticket);
        assert!(!owner.is_dirty());
    }
    #[test]
    fn parked_frontend_workspace_history_is_polled_without_activating_it() {
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        let mut workspace = tabs::WorkspaceDocuments::new((8, 8), false);
        paint(&mut workspace.active.editor, 0);
        let old = version(&workspace.active.editor);
        paint(&mut workspace.active.editor, 1);
        paint(&mut workspace.active.editor, 2);
        let ticket = workspace.active.editor.render_key();
        let active = app.active_tab_id();
        app.other_workspace = Some(Box::new(workspace));
        app.initialize_editing(ctx.clone());
        app.poll_residency_usage(&ctx, pressure());
        pump(&mut app, &ctx, |_| old.resident_bytes() == 0);
        assert_eq!(app.active_tab_id(), active);
        assert_eq!(
            app.other_workspace
                .as_ref()
                .unwrap()
                .active
                .editor
                .render_key(),
            ticket
        );
    }
    #[test]
    fn resource_failed_render_retries_after_reclamation_without_changing_document_key() {
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        let key = app.presentation_key();
        app.install_render(
            &ctx,
            key,
            Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "injected memory pressure",
            )),
        );
        assert_eq!(app.texture_key, Some(key));
        // Inject two counter observations; no allocation/RSS result is inferred.
        app.render_residency_retry = Some((key, 512));
        app.retry_reclaimed_render(BudgetUsage {
            limit: 1024,
            reserved: 512,
            peak: 512,
        });
        assert_eq!(app.texture_key, Some(key));
        app.retry_reclaimed_render(BudgetUsage {
            limit: 1024,
            reserved: 256,
            peak: 512,
        });
        assert!(app.texture_key.is_none());
        app.sync_texture(&ctx);
        assert_eq!(app.texture_key, Some(key));
        assert_eq!(app.rendered, render_flat(app.editor.document()).unwrap());
        assert!(app.render_residency_retry.is_none());
    }
}
