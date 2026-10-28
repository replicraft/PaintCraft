//! Bounded semantic-edge host for the optional owned native event loop.
//! No window/monitor/executor is created here, and startup does not enable it.
use crate::controller::native_publication::{Activation, Scope};
use crate::*;
use paintcraft_core::types::{Error, ErrorCode, Result, resources::SharedBudget};
use paintcraft_input::tablet::Phase;
use paintcraft_platform::tablet::{CaptureRecord, DeliveryHealth, RecordFault};
use paintcraft_ui::pen_input::{Owner, Predecessor};
use std::sync::{Arc, Mutex};
#[path = "owned_tablet/projection.rs"]
pub mod projection;
use projection::{Frame, Pointer, Projection};
#[derive(Clone, Copy, Debug)]
enum Origin {
    Pen {
        identity: paintcraft_input::tablet::Identity,
        sequence: u64,
    },
    Gui(u64),
}
#[derive(Clone, Copy, Debug)]
struct Barrier {
    before: Owner,
    origin: Origin,
    activation: Activation,
    cancellation_epoch: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    Idle,
    Advanced,
    WaitingForOwnEdit,
    WaitingForPriorEdit,
    Retired,
}
pub struct Step {
    pub output: egui::FullOutput,
    pub progress: Progress,
    pub error: Option<Error>,
}
impl std::fmt::Debug for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedTabletStep")
            .field("progress", &self.progress)
            .finish_non_exhaustive()
    }
}
/// A native owner retains this host and its Installation/window together. Sink
/// ingestion and snapshot happen under Installation's delivery gate; `step`
/// runs GUI/editor work only after that gate and the ingress mutex are released.
pub struct Host {
    app: App,
    ingress: Arc<Mutex<Projection>>,
    pending: Option<Frame>,
    cursor: usize,
    waiting: Option<Barrier>,
    limit: usize,
    _references: paintcraft_core::types::resources::BudgetAccount,
    own_work: Option<Activation>,
    fresh_health: Option<DeliveryHealth>,
    delivery_failed: bool,
    delivery: Option<paintcraft_platform::tablet::DeliveryCheck>,
}
impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedTabletHost")
            .field("cursor", &self.cursor)
            .field("waiting", &self.waiting)
            .finish_non_exhaustive()
    }
}
impl Host {
    pub fn new(mut app: App, limit: usize, budget: &SharedBudget) -> Result<Self> {
        if app.editing.is_none() {
            return Err(Error::new(
                ErrorCode::TargetInvalid,
                "Initialize the existing native editing service before ordered tablet capture",
            ));
        }
        let projection = Projection::new(limit, budget)?;
        app.connect_owned_tablet(projection.source(), budget)?;
        let mut references =
            budget.account(paintcraft_core::types::resources::AllocationClass::PaintMetadata);
        references.resize(
            (2 * limit * std::mem::size_of::<usize>() + 4 * std::mem::size_of::<Scope>()) as u64,
        )?;
        Ok(Self {
            app,
            ingress: Arc::new(Mutex::new(projection)),
            pending: None,
            cursor: 0,
            waiting: None,
            limit,
            _references: references,
            own_work: None,
            fresh_health: None,
            delivery_failed: false,
            delivery: None,
        })
    }
    /// Bind the actual installation before runtime input. A cloned authority is
    /// invalidated when that installation retires; it cannot keep capture alive.
    pub fn bind_delivery(&mut self, check: paintcraft_platform::tablet::DeliveryCheck) {
        self.delivery = Some(check);
    }
    pub fn app(&self) -> &App {
        &self.app
    }
    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }
    pub fn capture_endpoint(
        &self,
    ) -> impl Fn(CaptureRecord) -> std::result::Result<(), RecordFault> + Send + Sync + 'static
    {
        let ingress = self.ingress.clone();
        move |record| {
            ingress
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .capture(record)
        }
    }
    /// Only snapshot/discard here: callers may invoke this inside with_delivery.
    /// Never take another snapshot while a prior one owns a service barrier.
    pub fn snapshot(&mut self, health: DeliveryHealth) -> Result<bool> {
        // Health observation is mandatory even when a prior snapshot/job is
        // retained. This closure does safe metadata retirement only, no editor.
        self.fresh_health = Some(health);
        self.delivery_failed |= health == DeliveryHealth::Failed;
        if self.delivery_failed {
            self.ingress
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .fault();
            return Ok(false);
        }
        if self.pending.is_some() {
            return Ok(false);
        }
        let frame = self
            .ingress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take(health)?;
        if frame.entries.is_empty() {
            return Ok(false);
        }
        self.pending = Some(frame);
        self.cursor = 0;
        Ok(true)
    }
    fn cancel_own_work(&mut self) {
        if let Some(activation) = self.own_work.take() {
            self.app.editor.abandon_native_activation(activation);
        }
    }
    fn retire(&mut self) {
        self.cancel_own_work();
        self.pending = None;
        self.waiting = None;
        self.cursor = 0;
        self.app.retire_owned_tablet();
        self.ingress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .fault();
    }
    fn render(&mut self, ctx: &egui::Context, input: egui::RawInput) -> egui::FullOutput {
        let output = ctx.run_ui(input, |ui| self.app.ui(ui));
        self.app.editor.end_native_activation();
        output
    }
    fn publish_view(&mut self) -> Result<()> {
        if let Some(view) = self.app.finish_owned_view() {
            self.ingress
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .publish(view)?;
        }
        Ok(())
    }
    fn future_canvas(&self) -> bool {
        self.pending.as_ref().is_some_and(|f| {
            f.entries[self.cursor..].iter().any(
                |e| matches!(&e.pointer,Pointer::Pen(batch) if !batch.frame.samples.is_empty()),
            )
        })
    }
    fn finish_barrier(&mut self, barrier: Barrier) -> Result<()> {
        let after = self.app.owned_owner();
        let witness = self
            .app
            .editor
            .take_native_publication()
            .filter(|w| w.activation() == Some(barrier.activation));
        let navigation = self.app.editor.take_native_navigation().filter(|w| {
            w.activation() == Some(barrier.activation)
                && w.owner().as_u128() == barrier.before.session
                && w.before().state_id.as_u128() == barrier.before.state
                && w.before().publication_epoch == barrier.before.publication
                && w.state().as_u128() == after.state
                && w.publication() == after.publication
        });
        if matches!(barrier.origin, Origin::Pen { .. }) && navigation.is_some() {
            return Err(stale(
                "Navigation did not complete this pen service barrier",
            ));
        }
        let unchanged = barrier.before == after;
        let valid = witness.filter(|w| {
            w.owner().as_u128() == barrier.before.session
                && w.before().state_id.as_u128() == barrier.before.state
                && w.before().publication_epoch == barrier.before.publication
                && w.state().as_u128() == after.state
                && w.publication() == after.publication
        });
        let committed = valid.filter(|w| w.committed() && w.history().is_some());
        if let Origin::Pen { identity, sequence } = barrier.origin
            && let Some(w) = valid
            && w.contact_completion().is_none_or(|c| {
                c.identity() != identity
                    || c.sequence() != sequence
                    || c.capture_owner() != barrier.before.session
            })
        {
            return Err(stale(
                "Receipt did not publish this captured pen completion",
            ));
        }
        if !unchanged && committed.is_none() && navigation.is_none() && self.future_canvas() {
            // Navigation remains a distinct result. Do not forge an authored
            // receipt or rebase later canvas work from a changed ticket alone.
            return Err(stale(
                "No matching authored receipt for queued canvas continuation",
            ));
        }
        if committed.is_some() || navigation.is_some() {
            let ordinal = match barrier.origin {
                Origin::Pen { sequence, .. } => sequence,
                Origin::Gui(sequence) => sequence,
            };
            let mut ingress = self.ingress.lock().unwrap_or_else(|e| e.into_inner());
            let Some(old) = ingress.view() else {
                return Err(stale("Missing captured view"));
            };
            let Some(current) = self.app.owned_deferred_view() else {
                return Err(stale("Missing current view"));
            };
            if old.transform != current.transform
                || old.canvas != current.canvas
                || old.pixels_per_point != current.pixels_per_point
                || !current.focused
                || current.blocked
                || current.ime
            {
                return Err(stale(
                    "Known view or permission change retired continuation",
                ));
            }
            ingress.flush_deferred_pen()?;
            let source = ingress.source();
            let queued = ingress.queued_mut();
            let pending = self
                .pending
                .as_mut()
                .map_or(&mut [][..], |frame| &mut frame.entries[self.cursor..]);
            let mut batches = Vec::new();
            batches
                .try_reserve_exact(pending.len() + queued.len())
                .map_err(|_| resource("Continuation references"))?;
            if batches.capacity() > 2 * self.limit {
                return Err(resource("Continuation reference capacity"));
            }
            for entry in pending.iter_mut().chain(queued) {
                if let Pointer::Pen(batch) = &mut entry.pointer
                    && !batch.frame.samples.is_empty()
                {
                    if batch
                        .frame
                        .samples
                        .first()
                        .is_none_or(|s| s.phase != Phase::Down || s.sequence <= ordinal)
                    {
                        return Err(stale("Continuation is not a later captured Down"));
                    }
                    batches.push(batch);
                }
            }
            if !batches.is_empty() {
                source
                    .acknowledge_predecessors(
                        &mut batches,
                        Predecessor {
                            before: barrier.before,
                            after,
                            sequence: ordinal,
                            transform: current.transform,
                            canvas: current.canvas,
                            pixels_per_point: current.pixels_per_point,
                            focused: current.focused,
                            modal: current.blocked,
                            ime: current.ime,
                        },
                    )
                    .map_err(|_| stale("Opaque native continuation was retired"))?;
            }
        }
        self.own_work = None;
        self.publish_view()
    }
    fn completed(
        &mut self,
        ctx: &egui::Context,
        output: egui::FullOutput,
        progress: Progress,
        result: Result<()>,
    ) -> Step {
        match result {
            Ok(()) => Step {
                output,
                progress,
                error: None,
            },
            Err(error) => {
                self.retire();
                ctx.input_mut(|i| i.pointer = Default::default());
                self.app.editor.set_status(error.to_string());
                ctx.request_repaint();
                // Keep the actual frame/texture delta available to the renderer
                // even when input retirement occurs after GUI work.
                Step {
                    output,
                    progress: Progress::Retired,
                    error: Some(error),
                }
            }
        }
    }
    /// At most one semantic edge or one service poll per call. The caller waits
    /// for the existing edit service wake when Waiting; there is no new executor
    /// and no loop that retries permanent admission failure.
    pub fn step(&mut self, ctx: &egui::Context, mut input: egui::RawInput) -> Result<Step> {
        let Some(health) = self.fresh_health.take() else {
            self.retire();
            return Err(stale(
                "Observe native delivery health before every semantic step",
            ));
        };
        #[cfg(not(test))]
        if self.delivery.is_none() {
            self.retire();
            return Err(stale(
                "Bind the actual native installation before ordered intake",
            ));
        }
        if self.delivery_failed || health == DeliveryHealth::Failed {
            // Abandon matching queued/ready result before App::ui can poll it.
            // Unrelated pre-existing jobs have no matching source activation.
            self.retire();
            ctx.input_mut(|i| i.pointer = Default::default());
            self.app.begin_owned_edge(None, false)?;
            let output = self.render(ctx, input);
            let error = Error::new(
                ErrorCode::OperationCancelled,
                "Native delivery failed before publication",
            );
            return Ok(Step {
                output,
                progress: Progress::Retired,
                error: Some(error),
            });
        }
        if input.events.iter().any(pointer_event) {
            self.retire();
            return Err(stale(
                "Owned input must not duplicate framework pointer translation",
            ));
        }
        if (self.waiting.is_some() || self.pending.is_some())
            && (!input.focused || !input.events.is_empty())
        {
            // Framework key/text/IME events have no native pointer ordinal.
            // Preserve their ordinary App delivery, but never guess a semantic
            // join or silently discard them during an asynchronous own job.
            let kind = if !input.focused {
                paintcraft_platform::tablet::Boundary::FocusLost
            } else if input
                .events
                .iter()
                .any(|e| matches!(e, egui::Event::Ime(_)))
            {
                paintcraft_platform::tablet::Boundary::ImeReceipt
            } else {
                paintcraft_platform::tablet::Boundary::Keyboard
            };
            self.cancel_own_work();
            self.pending = None;
            self.cursor = 0;
            self.waiting = None;
            self.app.retire_owned_tablet();
            self.ingress
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .interrupt_pending(kind)?;
            ctx.input_mut(|i| i.pointer = Default::default());
            self.app.defer_owned_wait(false);
            let output = self.render(ctx, input);
            let result = self.publish_view();
            return Ok(self.completed(ctx, output, Progress::Retired, result));
        }
        if let Some(barrier) = self.waiting {
            let source_current = self
                .ingress
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .source()
                .cancellation_epoch()
                == barrier.cancellation_epoch;
            if self.app.action_context().blocked
                || self.app.ime_composing
                || self.app.owned_owner() != barrier.before
                || !source_current
            {
                self.retire();
                ctx.input_mut(|i| i.pointer = Default::default());
                self.app.begin_owned_edge(None, false)?;
                let output = self.render(ctx, input);
                return Ok(Step {
                    output,
                    progress: Progress::Retired,
                    error: Some(stale("Native context retired before own publication")),
                });
            }
            self.app.defer_owned_wait(true);
            let output = self.render(ctx, input);
            if self.app.editor.has_native_work(barrier.activation) {
                return Ok(Step {
                    output,
                    progress: Progress::WaitingForOwnEdit,
                    error: None,
                });
            }
            let result = self.finish_barrier(barrier);
            self.waiting = None;
            return Ok(self.completed(ctx, output, Progress::Advanced, result));
        }
        if self.app.any_pending_edits() || self.app.any_deferred_files() {
            // Never associate an unrelated earlier completion with this edge.
            self.app.defer_owned_wait(false);
            let output = self.render(ctx, input);
            let result = self.publish_view();
            return Ok(self.completed(ctx, output, Progress::WaitingForPriorEdit, result));
        }
        let before = self.app.owned_owner();
        let Some(frame) = &mut self.pending else {
            self.app.begin_owned_edge(None, false)?;
            let output = self.render(ctx, input);
            let result = self.publish_view();
            return Ok(self.completed(ctx, output, Progress::Idle, result));
        };
        if self.cursor == frame.entries.len() {
            self.pending = None;
            self.cursor = 0;
            self.fresh_health = Some(health);
            return self.step(ctx, input);
        }
        let entry = &mut frame.entries[self.cursor];
        let current = self
            .ingress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_current(entry, before);
        if !current {
            self.retire();
            ctx.input_mut(|i| i.pointer = Default::default());
            return Err(stale("Captured native edge is stale"));
        }
        let ordinal = entry.ordinal;
        let source_epoch = self
            .ingress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .source_epoch()
            .ok_or_else(|| stale("Missing native source installation"))?;
        let cancellation_epoch = self
            .ingress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .source()
            .cancellation_epoch();
        let source = self
            .ingress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .source();
        let publication = match &entry.pointer {
            Pointer::Pen(batch) => source.publication_guard(batch, before),
            Pointer::Gui(_) => source.ui_publication_guard(before),
            Pointer::Cancel { .. } => None,
        };
        let pointer = std::mem::replace(&mut entry.pointer, Pointer::Cancel { reset_gui: false });
        self.cursor += 1;
        self.app.editor.take_native_publication(); // clear before the exact edge
        self.app.editor.take_native_navigation();
        let mut origin = None;
        match pointer {
            Pointer::Gui(event) => {
                if let Some(sequence) = ordinal {
                    origin = Some(Origin::Gui(sequence));
                }
                input
                    .events
                    .try_reserve_exact(1)
                    .map_err(|_| resource("Native GUI edge"))?;
                input.events.push(event);
                self.app.begin_owned_edge(None, true)?;
            }
            Pointer::Pen(mut batch) => {
                if let Some(last) = batch.frame.samples.last().filter(|s| s.phase == Phase::Up) {
                    origin = Some(Origin::Pen {
                        identity: last.identity,
                        sequence: last.sequence,
                    });
                } else if batch.frame.samples.is_empty()
                    && let Some(sequence) = ordinal
                {
                    origin = Some(Origin::Gui(sequence));
                }
                if batch.frame.cancelled {
                    ctx.input_mut(|i| i.pointer = Default::default());
                }
                input
                    .events
                    .try_reserve_exact(batch.frame.ui.len())
                    .map_err(|_| resource("Native widget edges"))?;
                input.events.extend(
                    batch
                        .frame
                        .ui
                        .drain(..)
                        .map(|r| projection::ui_event(r.event)),
                );
                self.app.begin_owned_edge(Some(batch), origin.is_some())?;
            }
            Pointer::Cancel { reset_gui } => {
                self.app.retire_owned_tablet();
                if reset_gui {
                    ctx.input_mut(|i| i.pointer = Default::default());
                }
                self.app.begin_owned_edge(None, false)?;
            }
        }
        let sequence = match origin {
            Some(Origin::Pen { sequence, .. }) | Some(Origin::Gui(sequence)) => Some(sequence),
            None => ordinal,
        };
        let activation =
            sequence.map(|sequence| Activation::new(before.session, source_epoch, sequence));
        if let Some(activation) = activation {
            let Some(publication) = publication else {
                self.retire();
                return Err(stale("Native publication admission expired"));
            };
            self.app.editor.begin_native_activation(Scope::new(
                activation,
                publication,
                self.delivery.clone(),
            ));
        }
        let output = self.render(ctx, input);
        let result = if let Some(origin) = origin {
            let activation = activation.expect("source edge activation");
            let barrier = Barrier {
                before,
                origin,
                activation,
                cancellation_epoch,
            };
            if self.app.editor.has_native_work(activation) {
                self.own_work = Some(activation);
                self.waiting = Some(barrier);
                return Ok(Step {
                    output,
                    progress: Progress::WaitingForOwnEdit,
                    error: None,
                });
            }
            self.finish_barrier(barrier)
        } else {
            self.publish_view()
        };
        Ok(self.completed(ctx, output, Progress::Advanced, result))
    }
}
fn stale(message: &'static str) -> Error {
    Error::new(ErrorCode::ConflictStaleBase, message)
}
fn resource(message: &'static str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn pointer_event(event: &egui::Event) -> bool {
    matches!(
        event,
        egui::Event::PointerMoved(_)
            | egui::Event::PointerButton { .. }
            | egui::Event::PointerGone
            | egui::Event::MouseMoved(_)
            | egui::Event::MouseWheel { .. }
    )
}

#[cfg(test)]
#[path = "owned_tablet/tests.rs"]
mod tests;
