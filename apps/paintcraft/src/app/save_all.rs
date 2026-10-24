//! Save All serializes normal owner-bound saves without changing document contents.
use super::*;

#[derive(Debug)]
pub(super) struct Batch {
    remaining: Vec<Uuid>,
    awaiting: Option<Uuid>,
    origin: Uuid,
    pub close_after: bool,
}
impl App {
    pub(super) fn begin_save_all(&mut self, ctx: &egui::Context) {
        if self.experience() != Experience::Studio
            || self.action_context().blocked
            || self.tab_close_wait.is_some()
            || self.tab_save_close.is_some()
            || self.document_count() == 0
            || !self.editor.shape_file_ready()
        {
            return;
        }
        self.resolve_tab_input(ctx);
        self.save_all = Some(Batch {
            remaining: self.tabs.iter().map(|tab| tab.id).rev().collect(),
            awaiting: None,
            origin: self.active_tab_id(),
            close_after: false,
        });
        self.editor.set_status("Saving all modified documents…");
        self.pump_save_all(ctx);
    }

    fn stop_save_all(&mut self, ctx: &egui::Context, origin: Uuid, detail: Option<String>) {
        self.swap_tab_state(origin);
        self.reset_tab_presentation();
        self.editor.set_status(detail.map_or_else(
            || "Save All stopped; some documents remain unsaved.".into(),
            |detail| format!("Save All stopped: {detail}"),
        ));
        ctx.request_repaint();
    }

    pub(super) fn pump_save_all(&mut self, ctx: &egui::Context) {
        if self.save_all.is_none()
            || self.any_pending_edits()
            || self.any_deferred_files()
            || self.confirm.is_some()
            || self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
            || self
                .saving
                .as_ref()
                .is_some_and(saving::NativeSaving::pending)
        {
            return;
        }
        let mut batch = self.save_all.take().expect("active Save All");
        if let Some(id) = batch.awaiting.take() {
            if !self.swap_tab_state(id) {
                self.stop_save_all(ctx, batch.origin, None);
                return;
            }
            if self
                .saving
                .as_ref()
                .is_some_and(saving::NativeSaving::failed)
            {
                let detail = self.editor.status().to_owned();
                self.stop_save_all(ctx, batch.origin, Some(detail));
                return;
            }
            if self.editor.is_dirty() || self.has_document_drafts() {
                self.stop_save_all(ctx, batch.origin, None);
                return;
            }
        }
        while let Some(id) = batch.remaining.pop() {
            if !self.swap_tab_state(id) {
                self.stop_save_all(ctx, batch.origin, None);
                return;
            }
            if !self.editor.shape_file_ready() {
                let detail = self.editor.status().to_owned();
                self.stop_save_all(ctx, batch.origin, Some(detail));
                return;
            }
            if self.editor.is_dirty() || self.has_document_drafts() {
                self.resolve_tab_input(ctx);
                self.reset_tab_presentation();
                batch.awaiting = Some(id);
                self.save_all = Some(batch);
                self.save_requested(ctx);
                ctx.request_repaint();
                return;
            }
        }
        self.swap_tab_state(batch.origin);
        self.reset_tab_presentation();
        if self.any_dirty_documents()
            || self
                .saving
                .as_ref()
                .is_some_and(saving::NativeSaving::failed)
        {
            self.stop_save_all(ctx, batch.origin, None);
        } else {
            self.editor.set_status("All modified documents saved.");
            if batch.close_after {
                self.close_after_requested_saves(ctx);
            }
        }
        ctx.request_repaint();
    }
}

#[cfg(test)]
#[path = "save_all/tests.rs"]
mod tests;
