//! Shared pen ingress. Native delivery gates may call only the context-free
//! `Source` methods; GUI/editor work happens after the gate and ingress lock end.
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{AllocationClass, BudgetAccount, BudgetScope, SharedBudget},
};
use paintcraft_input::tablet::{
    Invalid, Permissions, Phase, RawSample, Sample, Transform,
    receiver::{Delivery, Frame, OrderedUiEvent, Receiver, Route},
};
use std::sync::{Arc, Mutex};

/// Exact host ownership, including state publication and interface changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owner {
    pub session: u128,
    pub state: u128,
    pub publication: u64,
    pub interface: u8,
}
/// Last completed layout. The GUI copies hit geometry outside the ingress lock.
/// Missing/overflowed hit information must set `blocked`, never guess canvas hit.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub owner: Owner,
    pub transform: Transform,
    pub pixels_per_point: f64,
    pub canvas: egui::Rect,
    pub exclusions: [Option<egui::Rect>; 64],
    pub focused: bool,
    pub blocked: bool,
    pub ime: bool,
}
impl View {
    fn permissions(self, at: Option<(f64, f64)>) -> Permissions {
        let hit = at.is_some_and(|(x, y)| {
            let at = egui::pos2(x as f32, y as f32);
            self.canvas.contains(at)
                && !self
                    .exclusions
                    .iter()
                    .flatten()
                    .any(|rect| rect.contains(at))
        });
        Permissions {
            owner: self.transform.owner,
            focused: self.focused,
            canvas_hit: hit,
            modal: self.blocked,
            ime: self.ime,
        }
    }
    fn valid(self) -> bool {
        self.pixels_per_point.is_finite()
            && self.pixels_per_point > 0.0
            && self.transform.document((0.0, 0.0)).is_ok()
            && self.canvas.is_finite()
            && self
                .exclusions
                .iter()
                .flatten()
                .all(|rect| rect.is_finite())
    }
}
#[derive(Debug)]
struct State {
    receiver: Receiver,
    view: Option<View>,
    // Storage drops before its charge. This accounts retained queue capacity,
    // not allocator metadata, GUI framework storage, or process RSS.
    _storage: BudgetAccount,
    frames: BudgetScope,
    window_epoch: u64,
    canvas_epoch: u64,
}
/// Cloneable endpoint for native sinks/DOM listeners; it never touches egui Context.
#[derive(Clone, Debug)]
pub struct Source(Arc<Mutex<State>>);
/// A drained payload owns its resource declaration until source/UI Vecs drop.
#[derive(Debug)]
pub struct Batch {
    pub frame: Frame,
    _storage: BudgetAccount,
    binding: Binding,
}
#[derive(Debug)]
struct Binding {
    source: std::sync::Weak<Mutex<State>>,
    owner: Owner,
    view: Option<View>,
    cancel_epoch: u64,
    window_epoch: u64,
    canvas_epoch: u64,
    acknowledged: Option<u64>,
    canvas_samples: bool,
    first_down: Option<u64>,
}
/// The trusted host fills this only after an earlier acquisition-tagged MouseUp
/// actually committed exactly one editor receipt. Current context is checked
/// again; this is never an authorization to rebase unrelated GUI publications.
#[derive(Clone, Copy, Debug)]
pub struct Predecessor {
    pub before: Owner,
    pub after: Owner,
    pub sequence: u64,
    pub transform: Transform,
    pub canvas: egui::Rect,
    pub pixels_per_point: f64,
    pub focused: bool,
    pub modal: bool,
    pub ime: bool,
}
impl Source {
    pub fn new(source_capacity: usize, ui_capacity: usize, budget: &SharedBudget) -> Result<Self> {
        if !(2..=65_537).contains(&source_capacity) || !(1..=196_608).contains(&ui_capacity) {
            return Err(invalid(Invalid::Capacity));
        }
        let bytes = source_capacity * std::mem::size_of::<Sample>()
            + (ui_capacity + 3) * std::mem::size_of::<OrderedUiEvent>();
        let mut storage = budget.account(AllocationClass::PaintMetadata);
        storage.resize(bytes as u64)?;
        // Separate reservation for at most one full source/UI handoff. Acquiring
        // it now avoids callbacks into GUI/resource watchers under a native gate.
        let frames = budget
            .reserve_capacity(bytes as u64)?
            .scope(AllocationClass::PaintMetadata);
        let receiver = Receiver::new(source_capacity, ui_capacity).map_err(invalid)?;
        storage.resize(receiver.storage_bytes() as u64)?;
        Ok(Self(Arc::new(Mutex::new(State {
            receiver,
            view: None,
            _storage: storage,
            frames,
            window_epoch: 0,
            canvas_epoch: 0,
        }))))
    }
    /// Publish only a complete final layout. Changed owners/transforms retire
    /// queued source before any host can replay it against a different document.
    pub fn publish(&self, view: View) -> std::result::Result<(), Invalid> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if !view.valid() {
            if state.receiver.has_capture_or_samples() {
                state.receiver.cancel();
            }
            state.window_epoch = state.window_epoch.saturating_add(1);
            state.view = None;
            return Err(Invalid::Coordinates);
        }
        let retire_all = state.view.is_some_and(|old| {
            old.owner.session != view.owner.session
                || old.owner.interface != view.owner.interface
                || old.pixels_per_point != view.pixels_per_point
                || (!old.blocked && view.blocked)
                || (!old.ime && view.ime)
        }) || !view.focused;
        let retire_canvas = state.view.is_some_and(|old| {
            old.owner != view.owner || old.transform != view.transform || old.canvas != view.canvas
        });
        if retire_all {
            state.window_epoch = state.window_epoch.saturating_add(1);
        }
        if retire_canvas {
            state.canvas_epoch = state.canvas_epoch.saturating_add(1);
        }
        if (retire_all && state.receiver.has_capture_or_samples())
            || (retire_canvas && state.receiver.has_canvas_capture_or_samples())
        {
            state.receiver.cancel();
        }
        state.view = Some(view);
        Ok(())
    }
    /// Convert OS physical/surface coordinates using this exact view's display
    /// scale, then capture its transform under the same lock. No callback escapes.
    pub fn submit_with(
        &self,
        delivery: Delivery,
        convert: impl FnOnce(f64) -> std::result::Result<RawSample, Invalid>,
    ) -> std::result::Result<Route, Invalid> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let Some(view) = state.view else {
            return Ok(Route::Suppressed);
        };
        let raw = match convert(view.pixels_per_point) {
            Ok(raw) => raw,
            Err(error) => {
                state.receiver.cancel();
                return Err(error);
            }
        };
        // Receiver checks stream identity before accepting positionless lifecycle
        // events. Only the explicit Source.cancel() operation is window-wide.
        state.receiver.submit(
            raw,
            view.transform,
            view.permissions(raw.logical_position),
            delivery,
        )
    }
    pub fn cancel(&self) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .receiver
            .cancel();
    }
    /// Display/window layout changed before a completed GUI pass. New pen
    /// contacts wait for a fresh view instead of using the old scale/origin.
    pub fn invalidate_view(&self) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.receiver.has_capture_or_samples() {
            state.receiver.cancel();
        }
        state.window_epoch = state.window_epoch.saturating_add(1);
        state.view = None;
    }
    pub fn fault(&self) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .receiver
            .fault();
    }
    /// True means the contact/tail must be suppressed as a palm.
    pub fn finger(&self, id: u64, phase: Phase) -> std::result::Result<bool, Invalid> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .receiver
            .finger(id, phase)
    }
    pub fn mouse_capture(&self, held: bool) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .receiver
            .mouse_capture(held)
    }
    /// Native hosts invoke this inside Installation::with_delivery, before editor
    /// work. A failed delivery discards even a fully queued but unpublished Up.
    pub fn take(&self, frame: u64, owner: Owner, delivery_failed: bool) -> Result<Batch> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if delivery_failed {
            state.receiver.fault();
        }
        if state.view.is_some_and(|view| view.owner != owner) {
            let different_session = state.view.is_some_and(|view| {
                view.owner.session != owner.session || view.owner.interface != owner.interface
            });
            // A previous handoff may already own all records. Invalidation is
            // independent of what remains in the ingress queue.
            state.canvas_epoch = state.canvas_epoch.saturating_add(1);
            if different_session {
                state.window_epoch = state.window_epoch.saturating_add(1);
            }
            let canvas = state.receiver.has_canvas_capture_or_samples();
            if (different_session && state.receiver.has_capture_or_samples()) || canvas {
                state.receiver.cancel();
            }
            if different_session || canvas {
                state.view = None;
            }
        }
        let mut storage = state.frames.account();
        let frame = state
            .receiver
            .take(frame, |bytes| {
                storage.resize(bytes as u64).map_err(|_| Invalid::Capacity)
            })
            .map_err(invalid)?;
        let canvas_samples = !frame.samples.is_empty();
        let first_down = frame
            .samples
            .iter()
            .find(|s| s.phase == Phase::Down)
            .map(|s| s.sequence);
        Ok(Batch {
            frame,
            _storage: storage,
            binding: Binding {
                source: Arc::downgrade(&self.0),
                owner,
                view: state.view,
                cancel_epoch: state.receiver.cancellation_epoch(),
                window_epoch: state.window_epoch,
                canvas_epoch: state.canvas_epoch,
                acknowledged: None,
                canvas_samples,
                first_down,
            },
        })
    }
    /// A drained handoff stays scoped even though its records left the queue.
    /// UI records permit same-workspace document/view changes; canvas data does not.
    pub fn batch_is_current(&self, batch: &Batch, live_owner: Owner) -> bool {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        self.current_in(&state, batch, live_owner)
    }
    fn current_in(&self, state: &State, batch: &Batch, live_owner: Owner) -> bool {
        let binding = &batch.binding;
        if binding.source.as_ptr() != Arc::as_ptr(&self.0)
            || binding.cancel_epoch == u64::MAX
            || binding.cancel_epoch != state.receiver.cancellation_epoch()
            || binding.window_epoch == u64::MAX
            || binding.window_epoch != state.window_epoch
        {
            return false;
        }
        let (Some(view), Some(captured)) = (state.view, binding.view) else {
            return false;
        };
        if binding.owner.session != live_owner.session
            || binding.owner.interface != live_owner.interface
            || view.owner.session != live_owner.session
            || view.owner.interface != live_owner.interface
            || !view.focused
            || captured.pixels_per_point != view.pixels_per_point
        {
            return false;
        }
        if !binding.canvas_samples {
            return true;
        }
        binding.canvas_epoch != u64::MAX
            && binding.canvas_epoch == state.canvas_epoch
            && binding.owner == live_owner
            && view.owner == live_owner
            && !view.blocked
            && !view.ime
            && captured.transform == view.transform
            && captured.canvas == view.canvas
    }
    /// Accept one proven earlier mouse completion before a later admitted pen
    /// Down. The opaque batch, exact view and one receipt step are all required.
    pub fn acknowledge_predecessor(
        &self,
        batch: &mut Batch,
        proof: Predecessor,
    ) -> std::result::Result<(), Invalid> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if !self.current_in(&state, batch, proof.before) {
            return Err(Invalid::Owner);
        }
        let Some(mut view) = state.view else {
            return Err(Invalid::Owner);
        };
        if proof.before.session != proof.after.session
            || proof.before.interface != proof.after.interface
            || proof.before.publication.checked_add(1) != Some(proof.after.publication)
            || !proof.focused
            || proof.modal
            || proof.ime
            || proof.transform != view.transform
            || proof.canvas != view.canvas
            || proof.pixels_per_point != view.pixels_per_point
        {
            return Err(Invalid::Owner);
        }
        if batch
            .binding
            .acknowledged
            .is_some_and(|old| proof.sequence <= old)
            || !batch
                .binding
                .first_down
                .is_some_and(|sequence| sequence > proof.sequence)
        {
            return Err(Invalid::Order);
        }
        view.owner = proof.after;
        state.view = Some(view);
        batch.binding.owner = proof.after;
        if let Some(captured) = &mut batch.binding.view {
            captured.owner = proof.after;
        }
        batch.binding.acknowledged = Some(proof.sequence);
        Ok(())
    }
}
fn invalid(reason: Invalid) -> Error {
    Error::new(
        match reason {
            Invalid::Capacity => ErrorCode::ResourceLimit,
            Invalid::Owner => ErrorCode::ConflictStaleBase,
            _ => ErrorCode::InputSchema,
        },
        format!("Pen input retired: {reason:?}"),
    )
}
/// Once-per-input-frame latch. Repeated GUI passes may observe flags but never
/// replay source or clear suppression merely because Receiver.take is empty.
#[derive(Default, Debug)]
pub struct Pass {
    frame: Option<u64>,
    suppress: bool,
    cancelled: bool,
}
impl Pass {
    pub fn begin(&mut self, frame: u64, input: &Frame) -> bool {
        if self.frame == Some(frame) {
            return false;
        }
        self.frame = Some(frame);
        self.suppress = input.suppress_legacy_pointer;
        self.cancelled = input.cancelled;
        true
    }
    pub fn suppress_legacy_pointer(&self) -> bool {
        self.suppress
    }
    pub fn cancelled(&self) -> bool {
        self.cancelled
    }
}

#[cfg(test)]
mod tests;
