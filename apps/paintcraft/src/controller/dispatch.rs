//! Retain a captured initial task across temporary worker-admission failures.
use super::*;
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_editor::session::HistoryTicket;
use paintcraft_runtime::Cancellation;

pub(super) type Task = Box<dyn FnOnce(Cancellation) -> Result<background::Completion> + Send>;
pub(super) struct Rejected {
    pub(super) error: Error,
    pub(super) task: Task,
}
pub(super) struct Waiting {
    task: Task,
    base: HistoryTicket,
    done: String,
    empty: &'static str,
    target_alias: Option<&'static str>,
}
impl std::fmt::Debug for Waiting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaitingDispatch")
            .field("base", &self.base)
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}
impl Editor {
    pub(super) fn dispatch_initial(
        &mut self,
        task: Result<Task>,
        done: String,
        empty: &'static str,
        target_alias: Option<&'static str>,
    ) {
        match task {
            Ok(task) => self.dispatch_captured(Waiting {
                task,
                base: self.session.history_ticket(),
                done,
                empty,
                target_alias,
            }),
            Err(error) => self.dispatch_failed(error),
        }
    }
    fn dispatch_failed(&mut self, error: Error) {
        self.preview_lineage = Uuid::new_v4();
        self.status = format!("Edit not prepared: {error}");
    }
    fn dispatch_captured(&mut self, waiting: Waiting) {
        let service = self.editing.as_ref().expect("background editing enabled");
        let Waiting {
            task,
            base,
            done,
            empty,
            target_alias,
        } = waiting;
        match service.submit_retained(task, done.clone(), empty, target_alias) {
            Ok(pending) => {
                self.status = format!("{done}…");
                self.pending_edit = Some(pending);
            }
            Err(Rejected { error, task }) if error.code == ErrorCode::ResourceLimit => {
                self.waiting_dispatch = Some(Waiting {
                    task,
                    base,
                    done,
                    empty,
                    target_alias,
                });
                self.status = "Waiting for editing worker capacity; Escape cancels the edit".into();
            }
            Err(rejected) => self.dispatch_failed(rejected.error),
        }
    }
    pub(super) fn retry_waiting_dispatch(&mut self) {
        let Some(waiting) = self.waiting_dispatch.as_ref() else {
            return;
        };
        if waiting.base != self.session.history_ticket() {
            self.waiting_dispatch = None;
            self.publication_preview = None;
            self.preview_queue_revision.advance();
            self.preview_lineage = Uuid::new_v4();
            self.status = "Waiting edit cancelled: its document base changed".into();
            return;
        }
        if !self
            .editing
            .as_ref()
            .is_some_and(|service| service.available())
        {
            return;
        }
        let waiting = self.waiting_dispatch.take().expect("waiting dispatch");
        self.dispatch_captured(waiting);
    }
}

#[cfg(test)]
#[path = "dispatch/tests.rs"]
mod tests;
