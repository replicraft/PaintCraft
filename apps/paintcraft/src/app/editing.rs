//! Shared native editing and document-owned deferred file requests.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum FileRequest {
    Save(PathBuf),
    Export(PathBuf),
}
#[derive(Debug)]
pub(super) struct DeferredFile {
    activation: u64,
    barrier: Uuid,
    request: FileRequest,
    started: bool,
}

impl App {
    pub(super) fn initialize_editing(&mut self, context: egui::Context) {
        match controller::background::Service::new(context) {
            Ok(service) => {
                self.editor.connect_editing(service.clone());
                self.editing = Some(service);
            }
            Err(error) => self
                .editor
                .set_status(format!("Background editing unavailable: {error}")),
        }
    }
    pub(super) fn any_pending_edits(&self) -> bool {
        self.editor.has_pending_edits()
            || self.tabs.iter().any(|tab| {
                tab.state
                    .as_ref()
                    .is_some_and(|state| state.editor.has_pending_edits())
            })
    }
    pub(super) fn any_deferred_files(&self) -> bool {
        !self.deferred_files.is_empty()
            || self.tabs.iter().any(|tab| {
                tab.state
                    .as_ref()
                    .is_some_and(|state| !state.deferred_files.is_empty())
            })
    }
    pub(super) fn tab_edit_pending(&self, id: Uuid) -> bool {
        if self.active_tab_id() == id {
            self.editor.has_pending_edits() || !self.deferred_files.is_empty()
        } else {
            self.tabs
                .iter()
                .find(|tab| tab.id == id)
                .and_then(|tab| tab.state.as_ref())
                .is_some_and(|state| {
                    state.editor.has_pending_edits() || !state.deferred_files.is_empty()
                })
        }
    }
    pub(super) fn defer_file(&mut self, request: FileRequest, activation: u64) {
        if self
            .deferred_files
            .iter()
            .any(|pending| pending.activation == activation && pending.request == request)
        {
            return;
        }
        if self.deferred_files.len() >= 4 {
            self.editor
                .set_status("File request not queued: four requests already wait for editing.");
            return;
        }
        let barrier = self.editor.queue_file_barrier();
        self.deferred_files.push_back(DeferredFile {
            activation,
            barrier,
            request,
            started: false,
        });
        self.editor
            .set_status("Waiting for earlier edits before capturing the file request…");
        self.context.request_repaint();
    }
    /// Poll every document, including inactive tabs. Targeted runtime polling
    /// keeps results and admission charged to their originating controller.
    pub(super) fn poll_edits(&mut self, ctx: &egui::Context) {
        let Some(service) = &self.editing else {
            return;
        };
        // There may be no open tabs left to poll a discarded controller.
        service.reap_abandoned();
        let mut original = self.active_tab_id();
        let ids = self.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>();
        for id in ids {
            self.swap_tab_state(id);
            if let Some(service) = &self.editing {
                self.editor.connect_editing(service.clone());
            }
            if self.editor.run_pending() > 0 {
                ctx.request_repaint();
            }
            let can_run_file = self.confirm.is_none()
                && !self.dialogs.as_ref().is_some_and(NativeDialogs::pending);
            if can_run_file && let Some(request) = self.deferred_files.front() {
                if request.started {
                    // The export modal finished while this boundary held the
                    // captured revision. Later commands may now proceed.
                    let request = self.deferred_files.pop_front().expect("deferred file");
                    self.editor.release_file_barrier(request.barrier);
                } else if self.editor.file_barrier_ready(request.barrier) {
                    let request = request.request.clone();
                    match request {
                        FileRequest::Save(path) => {
                            let request = self.deferred_files.pop_front().expect("deferred save");
                            self.save_activation = None;
                            self.capture_save(ctx, path);
                            self.editor.release_file_barrier(request.barrier);
                        }
                        FileRequest::Export(path) => {
                            self.deferred_files
                                .front_mut()
                                .expect("deferred export")
                                .started = true;
                            self.plan_export(path);
                            if self.confirm.is_some() {
                                original = id;
                                self.reset_tab_presentation();
                            }
                        }
                    }
                }
            }
            self.finish_save();
        }
        self.swap_tab_state(original);
        if self.any_pending_edits() || self.any_deferred_files() {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else if self.close_after_edits {
            self.close_after_edits = false;
            self.close_after_requested_saves(ctx);
        }
    }
}

#[cfg(test)]
#[path = "editing/tests.rs"]
mod tests;
