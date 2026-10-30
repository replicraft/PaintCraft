//! One shared native editing worker. Only each originating controller publishes.
use super::*;
use eframe::egui;
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_editor::session::{HistoryTicket, PreparedEdit, PreparedNavigation};
use paintcraft_runtime::{Cancellation, Executor, JobHandle, Limits};
use std::sync::Mutex;
#[path = "background/import.rs"]
mod import;
pub(crate) use import::ImportPending;
#[path = "background/jxr_export.rs"]
mod jxr_export;
pub(crate) use jxr_export::{JxrExportPending, PreparedJxrExport};

#[path = "background/msp_export.rs"]
mod msp_export;
pub(crate) use msp_export::MspExportPending;
#[path = "background/dds_export.rs"]
mod dds_export;
pub(crate) use dds_export::DdsExportPending;

#[path = "background/tiff_import.rs"]
mod tiff_import;
pub(crate) use tiff_import::{TiffCompletion, TiffPending};

// Provisional preparation admission, not allocator/RSS accounting. The service
// admits one transaction across all documents, including unconsumed results.
const PREPARATION_CHARGE: u64 = 2 << 30;
#[derive(Debug)]
pub(super) enum Completion {
    Edit(PreparedEdit),
    PenEdit(super::pen::Prepared),
    TiffImport(TiffCompletion),
    CodecImport(Box<paintcraft_formats::codec_worker::native::PreparedImport>),
    JxrExport(Box<PreparedJxrExport>),
    DdsExport(Box<crate::dds_export::worker::Prepared>),
    MspExport(Box<crate::msp_export::worker::Prepared>),
    Pick(Box<picker::Picked>),
    Deferred(Box<retry::Deferred>),
    Navigation(PreparedNavigation),
    Maintenance(paintcraft_editor::session::PreparedHistoryMaintenance),
}
#[derive(Clone, Debug)]
pub(crate) struct Service {
    executor: Arc<Mutex<Executor<Completion>>>,
    #[cfg(test)]
    completion_gate: Arc<Mutex<Option<tests::CompletionGate>>>,
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
    pub(super) native_activation: Option<native_publication::Scope>,
    pub(super) text: Option<text_submission::Attempt>,
    pub(super) filter: Option<Uuid>,
}
impl Pending {
    pub(super) fn cancel(&self) {
        if let Ok(executor) = self.service.lock() {
            executor.cancel(self.handle);
        }
    }
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
            #[cfg(test)]
            completion_gate: Arc::new(Mutex::new(None)),
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
        #[cfg(test)]
        let task = self.gate_completion(task);
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
            native_activation: None,
            text: None,
            filter: None,
        })
    }
}
// Keep the admitted scope inline through the existing FIFO. Boxing would add
// an unpriced infallible allocation for every accepted activation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(super) enum Queued {
    Edit(native_publication::QueuedEdit),
    FileBarrier(Uuid),
    TextCaptureFailure(paintcraft_tools::TextStamp),
}

impl Editor {
    pub(crate) fn connect_editing(&mut self, service: Service) {
        self.editing = Some(service);
    }
    /// Includes commands waiting behind an earlier transaction or another tab.
    pub fn has_pending_edits(&self) -> bool {
        self.has_inflight_edit()
            || self.retained_text_blocks_actions()
            || self
                .actions
                .iter()
                .any(|action| matches!(action, Queued::Edit(_)))
            || !self.outbox.is_empty()
    }
    /// Place a snapshot boundary after all earlier accepted activations.
    /// Later activations wait until the file coordinator releases this boundary.
    pub(crate) fn queue_file_barrier(&mut self) -> Uuid {
        self.drain_activations();
        let id = Uuid::new_v4();
        self.actions.push_back(Queued::FileBarrier(id));
        self.preview_queue_revision.advance();
        id
    }
    pub(crate) fn file_barrier_live(&self, id: Uuid) -> bool {
        self.file_barrier == Some(id)
            || self
                .actions
                .iter()
                .any(|queued| matches!(queued, Queued::FileBarrier(current) if *current == id))
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
        !self.filter_edit_open
            && !self.has_image_paste()
            && !self.text_placement_failed()
            && (self.file_barrier.is_some() || !self.has_pending_edits())
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
        if self.retained_text_blocks_actions() {
            self.cancel_text();
            return;
        }
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
        if let Err(error) = self
            .check_text_attempt(pending.text)
            .and_then(|_| self.check_filter_attempt(pending.filter))
        {
            self.finish_filter_attempt(pending.filter, false);
            self.finish_text_attempt(pending.text, false);
            self.publication_preview = None;
            self.preview_queue_revision.advance();
            self.status = format!("Edit not published: {error}");
            return;
        }
        let result = match result {
            Ok(Completion::Deferred(deferred)) => {
                self.status = "Waiting for memory capacity; Escape cancels the edit".into();
                self.waiting_edit = Some(retry::Waiting {
                    deferred: *deferred,
                    done: pending.done.clone(),
                    empty: pending.empty,
                    last_probe: std::time::Instant::now(),
                    target_alias: pending.target_alias,
                    native_activation: pending.native_activation.clone(),
                    text: pending.text,
                    filter: pending.filter,
                });
                return;
            }
            result => result,
        };
        self.preview_queue_revision.advance();
        self.publication_preview = None;
        let before_canvas = self.document().canvas;
        let before = self.session.ticket();
        let result = match result {
            Ok(Completion::Pick(picked)) => self.accept_pick(*picked),
            Ok(Completion::Edit(prepared)) => self
                .session
                .publish_prepared(prepared, || {
                    native_publication::check(pending.native_activation.as_ref())
                })
                .map(|receipt| {
                    self.record_native_publication(before, &receipt);
                    self.record_native_job_activation(
                        pending
                            .native_activation
                            .as_ref()
                            .map(|scope| scope.activation),
                    );
                    self.accept_edit(&receipt, &pending.done, pending.target_alias, before_canvas);
                }),
            Ok(Completion::PenEdit(prepared)) => {
                let completion = prepared.source_completion();
                prepared
                    .publish_with(&mut self.session, || {
                        native_publication::check(pending.native_activation.as_ref())
                    })
                    .map(|receipt| {
                        self.record_native_pen_publication(before, &receipt, completion);
                        self.record_native_job_activation(
                            pending
                                .native_activation
                                .as_ref()
                                .map(|scope| scope.activation),
                        );
                        self.accept_edit(
                            &receipt,
                            &pending.done,
                            pending.target_alias,
                            before_canvas,
                        );
                    })
            }
            Ok(Completion::Navigation(prepared)) => self
                .session
                .publish_navigation(prepared, || {
                    native_publication::check(pending.native_activation.as_ref())
                })
                .map(|navigation| {
                    self.record_native_navigation(before, navigation.as_ref());
                    if navigation.is_some() {
                        self.record_native_job_activation(
                            pending
                                .native_activation
                                .as_ref()
                                .map(|scope| scope.activation),
                        );
                    }
                    self.status = if navigation.is_some() {
                        pending.done.clone()
                    } else {
                        pending.empty.into()
                    };
                    self.retarget();
                }),
            Ok(Completion::Deferred(_)) => unreachable!("deferred edits were retained above"),
            Ok(
                Completion::TiffImport(_)
                | Completion::CodecImport(_)
                | Completion::JxrExport(_)
                | Completion::DdsExport(_)
                | Completion::MspExport(_),
            ) => Err(Error::new(
                ErrorCode::InvariantViolation,
                "Codec import completion belongs to the application",
            )),
            Ok(Completion::Maintenance(_)) => Err(Error::new(
                ErrorCode::InvariantViolation,
                "maintenance result routed to edit consumer",
            )),
            Err(error) => Err(error),
        };
        self.finish_text_attempt(pending.text, result.is_ok());
        self.finish_filter_attempt(pending.filter, result.is_ok());
        if let Err(error) = result {
            self.preview_lineage = Uuid::new_v4();
            self.status = if pending.done == picker::STATUS {
                if error.code == ErrorCode::OperationCancelled {
                    "Color sample cancelled".into()
                } else {
                    format!("Color sample failed: {error}")
                }
            } else if error.code == ErrorCode::OperationCancelled {
                "Edit cancelled".into()
            } else {
                format!("Edit not published: {error}")
            };
        }
    }
}

pub(super) fn preview(action: &Action) -> Option<Preview> {
    match action {
        Action::Filter(edit) => Some(Preview::Filter(edit.clone())),
        Action::MoveSelection {
            layer,
            offset,
            clear,
            transparent_key,
        } => Some(Preview::MoveSelection {
            layer: *layer,
            offset: *offset,
            clear: *clear,
            transparent_key: *transparent_key,
        }),
        Action::Contour(contour) => Some(Preview::Contour(contour.clone())),
        Action::Stroke(stroke) => Some(Preview::Path(stroke.clone())),
        Action::SampledStroke(stroke) => Some(Preview::SampledStroke(stroke.clone())),
        Action::PenStroke(stroke) => Some(Preview::PenStroke(stroke.clone())),
        Action::ResourceShape(shape) => Some(Preview::ResourceShape(*shape)),
        Action::Shape {
            layer,
            tool,
            from,
            to,
            stroke,
            fill,
            ..
        } => Some(Preview::Shape {
            layer: *layer,
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
