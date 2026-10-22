//! One shared native editing worker. Only each originating controller publishes.
use super::*;
use eframe::egui;
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_editor::session::{HistoryTicket, PreparedEdit, PreparedNavigation};
use paintcraft_runtime::{Cancellation, Executor, JobHandle, Limits};
use std::sync::Mutex;

// Provisional preparation admission, not allocator/RSS accounting. The service
// admits one transaction across all documents, including unconsumed results.
const PREPARATION_CHARGE: u64 = 2 << 30;
#[derive(Debug)]
pub(super) enum Completion {
    Edit(PreparedEdit),
    Deferred(Box<retry::Deferred>),
    Navigation(PreparedNavigation),
    Maintenance(paintcraft_editor::session::PreparedHistoryMaintenance),
}
#[derive(Clone, Debug)]
pub(crate) struct Service {
    executor: Arc<Mutex<Executor<Completion>>>,
    context: egui::Context,
    pub(super) residency: paintcraft_runtime::SharedBudget,
    pub(super) metadata: paintcraft_runtime::SharedBudget,
}
#[derive(Debug)]
pub(super) struct Pending {
    service: Service,
    handle: JobHandle,
    done: String,
    empty: &'static str,
    target_alias: Option<&'static str>,
}
impl Pending {
    pub(super) fn poll(&self) -> Option<Result<Completion>> {
        match self.service.lock() {
            Ok(mut executor) => executor.poll_for(self.handle),
            Err(error) => Some(Err(error)),
        }
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        if let Ok(mut executor) = self.service.executor.lock() {
            executor.abandon(self.handle);
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) enum NavigationRequest {
    Undo,
    Redo,
    Seek {
        ticket: HistoryTicket,
        history_id: Option<Uuid>,
    },
}
impl Service {
    #[cfg(test)]
    pub(crate) fn new(context: egui::Context) -> Result<Self> {
        Self::with_budget(
            context,
            paintcraft_runtime::SharedBudget::new(PREPARATION_CHARGE),
        )
    }
    #[cfg(test)]
    pub(crate) fn with_residency(
        context: egui::Context,
        residency: paintcraft_runtime::SharedBudget,
    ) -> Result<Self> {
        let mut service = Self::new(context)?;
        service.residency = residency;
        Ok(service)
    }
    pub(crate) fn with_budget(
        context: egui::Context,
        budget: paintcraft_runtime::SharedBudget,
    ) -> Result<Self> {
        if budget.usage().limit < PREPARATION_CHARGE {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "worker budget cannot fit edit preparation",
            ));
        }
        Ok(Self {
            executor: Arc::new(Mutex::new(Executor::with_budget(
                Limits {
                    workers: 1,
                    pending_jobs: 1,
                    transient_bytes: PREPARATION_CHARGE,
                },
                budget,
            )?)),
            context,
            residency: paintcraft_raster::Admission::new(0).residency_budget(),
            metadata: paintcraft_raster::Admission::new(0).paint_metadata_budget(),
        })
    }
    pub(crate) fn metadata_budget(&self) -> paintcraft_runtime::SharedBudget {
        self.metadata.clone()
    }
    #[cfg(test)]
    pub(crate) fn with_metadata(mut self, budget: paintcraft_runtime::SharedBudget) -> Self {
        self.metadata = budget;
        self
    }
    pub(super) fn admission(&self, limit: u64) -> paintcraft_raster::Admission {
        paintcraft_raster::Admission::with_residency(limit, self.residency.clone())
            .with_paint_metadata_budget(self.metadata.clone())
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Executor<Completion>>> {
        self.executor.lock().map_err(|_| {
            Error::new(
                ErrorCode::InvariantViolation,
                "editing worker coordinator unavailable",
            )
        })
    }
    pub(crate) fn reap_abandoned(&self) {
        if let Ok(mut executor) = self.lock() {
            executor.reap_abandoned();
        }
    }
    pub(super) fn available(&self) -> bool {
        self.lock()
            .map(|mut executor| {
                executor.reap_abandoned();
                executor.can_submit(PREPARATION_CHARGE)
            })
            .unwrap_or(true) // Admission will report the error, rather than stall forever.
    }
    pub(super) fn submit(
        &self,
        task: impl FnOnce(Cancellation) -> Result<Completion> + Send + 'static,
        done: String,
        empty: &'static str,
        target_alias: Option<&'static str>,
    ) -> Result<Pending> {
        self.submit_retained(Box::new(task), done, empty, target_alias)
            .map_err(|rejected| rejected.error)
    }
    pub(super) fn submit_retained(
        &self,
        task: dispatch::Task,
        done: String,
        empty: &'static str,
        target_alias: Option<&'static str>,
    ) -> std::result::Result<Pending, dispatch::Rejected> {
        let context = self.context.clone();
        let mut executor = match self.lock() {
            Ok(executor) => executor,
            Err(error) => return Err(dispatch::Rejected { error, task }),
        };
        let handle = match executor
            .submit_retained(PREPARATION_CHARGE, task, move || context.request_repaint())
        {
            Ok(handle) => handle,
            Err(rejected) => {
                return Err(dispatch::Rejected {
                    error: rejected.error,
                    task: rejected.task,
                });
            }
        };
        Ok(Pending {
            service: self.clone(),
            handle,
            done,
            empty,
            target_alias,
        })
    }
}
#[derive(Debug)]
pub(super) enum Queued {
    Edit(Action),
    FileBarrier(Uuid),
}

impl Editor {
    pub(crate) fn connect_editing(&mut self, service: Service) {
        self.editing = Some(service);
    }
    /// Includes commands waiting behind an earlier transaction or another tab.
    pub fn has_pending_edits(&self) -> bool {
        self.has_inflight_edit()
            || self
                .actions
                .iter()
                .any(|action| matches!(action, Queued::Edit(_)))
            || !self.outbox.is_empty()
    }
    /// Place a snapshot boundary after all earlier accepted activations.
    /// Later activations wait until the file coordinator releases this boundary.
    pub(crate) fn queue_file_barrier(&mut self) -> Uuid {
        self.actions
            .extend(self.outbox.drain().into_iter().map(Queued::Edit));
        let id = Uuid::new_v4();
        self.actions.push_back(Queued::FileBarrier(id));
        self.preview_queue_revision.advance();
        id
    }
    pub(crate) fn file_barrier_ready(&self, id: Uuid) -> bool {
        self.file_barrier == Some(id)
    }
    pub(crate) fn release_file_barrier(&mut self, id: Uuid) {
        if self.file_barrier == Some(id) {
            self.file_barrier = None;
            self.preview_queue_revision.advance();
        }
    }
    pub(crate) fn at_file_barrier(&self) -> bool {
        self.file_barrier.is_some()
    }
    pub(crate) fn can_capture_file(&self) -> bool {
        self.file_barrier.is_some() || !self.has_pending_edits()
    }
    pub(crate) fn preview_owner(&self) -> (Uuid, Option<u64>) {
        (self.preview_lineage, self.preview_token())
    }
    pub fn preview_token(&self) -> Option<u64> {
        self.tools.preview_token().or_else(|| {
            self.publication_preview
                .as_ref()
                .map(|_| self.publications())
        })
    }
    /// Escape cancels private work; the worker's reservation remains charged
    /// until completion. Already published edits are never undone implicitly.
    pub fn cancel_pending_edits(&mut self) {
        self.preview_lineage = Uuid::new_v4();
        self.actions
            .retain(|action| matches!(action, Queued::FileBarrier(_)));
        self.outbox.drain();
        if let Some(pending) = &self.pending_edit {
            if let Ok(executor) = pending.service.lock() {
                executor.cancel(pending.handle);
            }
            self.status = "Cancelling edit…".into();
        }
        let waiting = self.waiting_edit.take().is_some();
        let dispatch = self.waiting_dispatch.take().is_some();
        if (waiting || dispatch) && self.pending_edit.is_none() {
            self.status = "Edit cancelled".into();
        }
        self.publication_preview = None;
    }
    pub(super) fn start_edit(
        &mut self,
        batch: Batch,
        done: &str,
        target_alias: Option<&'static str>,
    ) {
        let admission = self
            .editing
            .as_ref()
            .expect("editing service")
            .admission(u64::MAX);
        let task = self.session.preparation().map(|preparation| {
            Box::new(move |cancellation: Cancellation| {
                retry::prepare_batch(preparation, Arc::new(batch), admission, || {
                    cancellation.check()
                })
            }) as dispatch::Task
        });
        self.dispatch_initial(task, done.to_owned(), "", target_alias);
    }
    /// Build tool/font commands on the admitted worker, retaining the captured plan
    /// unchanged if shared worker capacity disappears before submission.
    pub(super) fn start_tool_plan(&mut self, preview: Preview, done: &str) {
        let plan = self.prepare_preview(Some(preview));
        let admission = self
            .editing
            .as_ref()
            .expect("editing service")
            .admission(u64::MAX);
        let task = self.session.preparation().map(|preparation| {
            Box::new(move |cancellation: Cancellation| {
                let check = || cancellation.check();
                match plan.build(check)? {
                    Some(batch) => {
                        retry::prepare_batch(preparation, Arc::new(batch), admission, check)
                    }
                    None => preparation.no_change(check).map(Completion::Edit),
                }
            }) as dispatch::Task
        });
        self.dispatch_initial(task, done.into(), "", None);
    }
    /// Returns false only for controllers intentionally using synchronous mode.
    pub(super) fn start_navigation(&mut self, request: NavigationRequest) -> bool {
        if self.editing.is_none() {
            return false;
        }
        self.cancel_gesture();
        let (done, empty) = match request {
            NavigationRequest::Undo => ("Undo", "Nothing to undo"),
            NavigationRequest::Redo => ("Redo", "Nothing to redo"),
            NavigationRequest::Seek { .. } => ("History restored", "History unchanged"),
        };
        let admission = self
            .editing
            .as_ref()
            .expect("editing service")
            .admission(u64::MAX);
        let task = self.session.preparation().map(|preparation| {
            Box::new(move |cancellation: Cancellation| {
                retry::prepare_navigation(preparation, request, admission, || cancellation.check())
            }) as dispatch::Task
        });
        self.dispatch_initial(task, done.into(), empty, None);
        true
    }
    #[cfg(test)]
    fn install_pending(&mut self, result: Result<Pending>) {
        match result {
            Ok(pending) => {
                self.status = format!("{}…", pending.done);
                self.pending_edit = Some(pending);
            }
            Err(error) => {
                self.preview_lineage = Uuid::new_v4();
                self.status = format!("Edit not prepared: {error}");
            }
        }
    }
    pub(super) fn poll_edit(&mut self) {
        let Some(pending) = &self.pending_edit else {
            return;
        };
        let result = pending.poll();
        let Some(result) = result else {
            return;
        };
        let pending = self.pending_edit.take().expect("pending edit");
        let result = match result {
            Ok(Completion::Deferred(deferred)) => {
                self.status = "Waiting for memory capacity; Escape cancels the edit".into();
                self.waiting_edit = Some(retry::Waiting {
                    deferred: *deferred,
                    done: pending.done.clone(),
                    empty: pending.empty,
                    last_probe: std::time::Instant::now(),
                    target_alias: pending.target_alias,
                });
                return;
            }
            result => result,
        };
        self.preview_queue_revision.advance();
        self.publication_preview = None;
        let result = match result {
            Ok(Completion::Edit(prepared)) => self
                .session
                .publish_prepared(prepared, || Ok(()))
                .map(|receipt| {
                    self.accept_edit(&receipt, &pending.done, pending.target_alias);
                }),
            Ok(Completion::Navigation(prepared)) => self
                .session
                .publish_navigation(prepared, || Ok(()))
                .map(|navigation| {
                    self.status = if navigation.is_some() {
                        pending.done.clone()
                    } else {
                        pending.empty.into()
                    };
                    self.retarget();
                }),
            Ok(Completion::Deferred(_)) => unreachable!("deferred edits were retained above"),
            Ok(Completion::Maintenance(_)) => Err(Error::new(
                ErrorCode::InvariantViolation,
                "maintenance result routed to edit consumer",
            )),
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            self.preview_lineage = Uuid::new_v4();
            self.status = if error.code == ErrorCode::OperationCancelled {
                "Edit cancelled".into()
            } else {
                format!("Edit not published: {error}")
            };
        }
    }
}

pub(super) fn preview(action: &Action) -> Option<Preview> {
    match action {
        Action::Stroke(stroke) => Some(Preview::Path(stroke.clone())),
        Action::Shape {
            tool,
            from,
            to,
            stroke,
            fill,
            ..
        } => Some(Preview::Shape {
            tool: *tool,
            from: *from,
            to: *to,
            stroke: *stroke,
            fill: *fill,
        }),
        Action::Text {
            layer,
            style,
            background,
            at,
            text,
            scale,
            color,
            ..
        } => Some(Preview::Text {
            layer: *layer,
            style: style.clone(),
            background: *background,
            at: *at,
            text: text.clone(),
            scale: *scale,
            color: *color,
        }),
        _ => None,
    }
}

#[cfg(test)]
#[path = "background/tests.rs"]
mod tests;
