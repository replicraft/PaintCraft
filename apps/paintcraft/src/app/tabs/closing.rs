//! Closing owns a stable tab identity and waits for captured file operations.
use super::*;

impl App {
    fn tab_io_pending(&self, id: Uuid) -> bool {
        self.saving
            .as_ref()
            .is_some_and(|s| s.pending_for_tab(self, id))
            || self.tab_edit_pending(id)
    }

    pub(crate) fn request_close_tab(&mut self, id: Uuid, ctx: &egui::Context) {
        if self.confirm.is_some()
            || self.tab_closing.is_some()
            || self.tab_close_wait.is_some()
            || self.tab_save_close.is_some()
            || self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
        {
            return;
        }
        self.activate_tab(id, ctx);
        if self.active_tab_id() != id {
            return;
        }
        self.resolve_tab_input(ctx);
        if let Some(saving) = &mut self.saving {
            saving.cancel_restore();
        }
        if self.tab_io_pending(id) {
            self.tab_close_wait = Some(id);
            self.editor
                .set_status("Waiting for this document's file operations before closing…");
        } else if self.editor.is_dirty() || self.has_document_drafts() {
            self.tab_closing = Some(id);
        } else {
            self.try_close_tab(id, ctx);
        }
    }

    pub(crate) fn try_close_tab(&mut self, id: Uuid, ctx: &egui::Context) {
        if self.active_tab_id() != id {
            return;
        }
        if let Err(error) = self.prepare_editor_replacement() {
            if error.code == paintcraft_core::types::ErrorCode::ResourceLimit {
                self.tab_close_wait = Some(id);
                ctx.request_repaint_after(Duration::from_millis(50));
            }
            self.editor
                .set_status(format!("Document remains open: {error}"));
            return;
        }
        if let Some(saving) = &mut self.saving {
            saving.cancel_restore();
            saving.cancel_inspection();
        }
        self.tabs.remove(self.active_tab);
        if self.tabs.is_empty() {
            self.editor =
                Editor::new_document(DEFAULT_SIZE.0, DEFAULT_SIZE.1).expect("default canvas");
            if let Some(saving) = &mut self.saving {
                saving.new_session();
            }
            self.active_tab = 0;
            self.bound = None;
            self.imported_source = None;
            self.auxiliary_sources.clear();
            self.path_text = "untitled.pcnd".into();
            self.pending_save = None;
            self.deferred_files.clear();
            self.name_edit = None;
            self.opacity_edit = None;
            self.recovery_needs_init = false;
        } else {
            self.active_tab = self.active_tab.min(self.tabs.len() - 1);
            let mut next = self.tabs[self.active_tab]
                .state
                .take()
                .expect("parked document");
            next.swap(self);
        }
        self.tab_closing = None;
        self.tab_close_wait = None;
        self.tab_save_close = None;
        self.reset_tab_presentation();
        ctx.request_repaint();
    }

    pub(crate) fn tab_close_dialog(&mut self, ctx: &egui::Context) {
        let Some(id) = self.tab_closing else {
            return;
        };
        if self.active_tab_id() != id {
            self.tab_closing = None;
            return;
        }
        use paintcraft_ui::confirmation::{Choice, Prompt};
        let choice = paintcraft_ui::confirmation::show(
            ctx,
            egui::Id::unique("close-document-confirmation"),
            Prompt {
                title: "Save changes before closing this document?",
                message: "This document has unsaved changes.",
                accept: "Save and close",
                discard: Some("Don't save"),
            },
        );
        if ctx.current_pass_index() != 0 {
            return;
        }
        match choice {
            Some(Choice::Accept) => {
                self.tab_closing = None;
                self.resolve_tab_input(ctx);
                self.tab_save_close = Some((id, self.editor.document().state_id));
                self.save_requested(ctx);
            }
            Some(Choice::Discard) => {
                self.tab_closing = None;
                self.try_close_tab(id, ctx);
            }
            Some(Choice::Cancel) => self.tab_closing = None,
            None => {}
        }
    }

    pub(crate) fn request_workspace_close(&mut self, ctx: &egui::Context) {
        if self.any_pending_edits() || self.any_deferred_files() {
            self.close_after_edits = true;
            ctx.request_repaint();
            return;
        }
        self.closing_workspace = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    pub(crate) fn pump_tab_close(&mut self, ctx: &egui::Context) {
        if self.closing_workspace {
            return;
        }
        if self.confirm.is_some() || self.dialogs.as_ref().is_some_and(NativeDialogs::pending) {
            return;
        }
        if let Some(id) = self.tab_close_wait
            && !self.tab_io_pending(id)
        {
            self.tab_close_wait = None;
            self.request_close_tab(id, ctx);
        }
        let Some((id, source)) = self.tab_save_close else {
            return;
        };
        if self.tab_io_pending(id) {
            return;
        }
        let original = self.active_tab_id();
        if !self.swap_tab_state(id) {
            self.tab_save_close = None;
            return;
        }
        self.tab_save_close = None;
        if !self.editor.is_dirty()
            && self.editor.document().state_id == source
            && !self.has_document_drafts()
        {
            self.try_close_tab(id, ctx);
        } else {
            self.editor.set_status("Document remains open: saving was cancelled, failed, or newer edits are still unsaved.");
        }
        if self.swap_tab_state(original) {
            self.reset_tab_presentation();
        }
    }

    pub(crate) fn clean_all_recovery_before_close(
        &mut self,
    ) -> paintcraft_core::types::Result<bool> {
        let original = self.active_tab_id();
        let ids = self.tabs.iter().map(|t| t.id).collect::<Vec<_>>();
        let mut queued = false;
        let mut result = Ok(());
        for id in ids {
            self.swap_tab_state(id);
            match self.clean_recovery_before_close() {
                Ok(pending) => queued |= pending,
                Err(error)
                    if error.code == paintcraft_core::types::ErrorCode::ResourceLimit
                        && self
                            .saving
                            .as_ref()
                            .is_some_and(saving::NativeSaving::pending) =>
                {
                    queued = true;
                    break;
                }
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        self.swap_tab_state(original);
        result.map(|()| queued)
    }
}
