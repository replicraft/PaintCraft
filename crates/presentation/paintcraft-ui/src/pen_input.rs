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

mod continuation;
pub use continuation::Continuation;
mod contact_tokens;
mod native;
mod native_contacts;
pub use native_contacts::{NativePenCancellation, NativePenTarget};
mod native_touch;
pub use native_touch::{
    NativeFingerAdmission, NativeFingerMetadata, NativeTouchBatch, NativeTouchCancellation,
    NativeTouchOutcome, NativeTouchTarget,
};
mod publication;
pub use publication::PublicationGuard;

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
    pen_epoch: u64,
    native: native::Metadata,
    native_touch: native_touch::Resident,
    native_contacts: native_contacts::Resident,
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
    pen_epoch: Option<u64>,
    native_touch: Option<native_touch::Lease>,
    native_contacts: Option<contact_tokens::Leases>,
    acknowledged: Option<u64>,
    canvas_samples: bool,
    last_down: Option<u64>,
}
/// The trusted host fills this only after an earlier acquisition-tagged contact completion
/// actually completed exactly one typed editor publication or history-navigation
/// result. The host must bind that actual result to the exact initiating edge and
/// clean service barrier; changed tickets alone are insufficient. Current context
/// is checked again, never authorizing unrelated GUI publications.
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
        Self::construct(source_capacity, ui_capacity, budget, false, false)
    }
    /// Explicit native-touch candidate configuration. Default/browser sources
    /// retain the original quote and allocate no native bank or native frames.
    pub fn new_native_touch(
        source_capacity: usize,
        ui_capacity: usize,
        budget: &SharedBudget,
    ) -> Result<Self> {
        Self::construct(source_capacity, ui_capacity, budget, true, false)
    }
    /// Candidate native source with exact per-contact pen and touch authority.
    /// Default/browser sources retain their original bank allocation semantics.
    /// Each handoff binds every included contact. If an aggregated interval
    /// contains a retired contact, the whole handoff refuses publication;
    /// separately drained completed work retains its independent authority.
    pub fn new_native_contacts(
        source_capacity: usize,
        ui_capacity: usize,
        budget: &SharedBudget,
    ) -> Result<Self> {
        Self::construct(source_capacity, ui_capacity, budget, true, true)
    }
    fn construct(
        source_capacity: usize,
        ui_capacity: usize,
        budget: &SharedBudget,
        native_touch: bool,
        native_contacts: bool,
    ) -> Result<Self> {
        if !(2..=65_537).contains(&source_capacity) || !(1..=196_608).contains(&ui_capacity) {
            return Err(invalid(Invalid::Capacity));
        }
        let bytes = source_capacity * std::mem::size_of::<Sample>()
            + (ui_capacity + 3) * std::mem::size_of::<OrderedUiEvent>();
        let mut storage = budget.account(AllocationClass::PaintMetadata);
        storage.resize(
            (bytes + std::mem::size_of::<native::Metadata>() + std::mem::size_of::<u64>()) as u64,
        )?;
        // Separate reservation for at most one full source/UI handoff. Acquiring
        // it now avoids callbacks into GUI/resource watchers under a native gate.
        let frames = budget
            .reserve_capacity(
                (bytes
                    + if native_contacts {
                        std::mem::size_of::<contact_tokens::Leases>()
                    } else {
                        0
                    }) as u64,
            )?
            .scope(AllocationClass::PaintMetadata);
        let native_touch = if native_touch {
            native_touch::Resident::new(budget, source_capacity)?
        } else {
            native_touch::Resident::default()
        };
        let native_contacts = if native_contacts {
            native_contacts::Resident::new(budget)?
        } else {
            native_contacts::Resident::default()
        };
        let receiver = Receiver::new(source_capacity, ui_capacity).map_err(invalid)?;
        storage.resize(
            (receiver.storage_bytes()
                + std::mem::size_of::<native::Metadata>()
                + std::mem::size_of::<u64>()) as u64,
        )?;
        Ok(Self(Arc::new(Mutex::new(State {
            receiver,
            view: None,
            _storage: storage,
            frames,
            window_epoch: 0,
            canvas_epoch: 0,
            pen_epoch: 0,
            native: Default::default(),
            native_touch,
            native_contacts,
        }))))
    }
    /// Publish only a complete final layout. Changed owners/transforms retire
    /// queued source before any host can replay it against a different document.
    pub fn publish(&self, view: View) -> std::result::Result<(), Invalid> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if !view.valid() {
            state.native_touch.fence_all();
            if state.receiver.has_capture_or_samples() {
                state.native.fence();
                state.receiver.cancel();
            }
            state.window_epoch = state.window_epoch.saturating_add(1);
            state.native.fence();
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
            state.native_touch.fence_all();
            state.native.fence();
            state.window_epoch = state.window_epoch.saturating_add(1);
        }
        if retire_canvas {
            state.native_touch.fence_canvas();
            state.canvas_epoch = state.canvas_epoch.saturating_add(1);
        }
        if (retire_all && state.receiver.has_capture_or_samples())
            || (retire_canvas && state.receiver.has_canvas_capture_or_samples())
        {
            state.native.fence();
            state.receiver.cancel();
        }
        state.view = Some(view);
        Ok(())
    }
    /// Compatibility conversion for an already completed view. Acquisition
    /// callers must use submit_native so metadata is observed while no view is
    /// available. Missing view refuses explicitly instead of dropping a tail.
    pub fn submit_with(
        &self,
        delivery: Delivery,
        convert: impl FnOnce(f64) -> std::result::Result<RawSample, Invalid>,
    ) -> std::result::Result<Route, Invalid> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let Some(view) = state.view else {
            return Err(Invalid::Coordinates);
        };
        let mut raw = match convert(view.pixels_per_point) {
            Ok(raw) => raw,
            Err(error) => {
                state.native.fence();
                state.receiver.cancel();
                return Err(error);
            }
        };
        let position = raw.logical_position.take();
        state.submit_native(delivery, raw, None, |_| Ok(position))
    }
    /// DOM/native intake can fence physical tails after host-side retirement.
    /// Reading this token neither drains input nor invokes GUI callbacks.
    pub fn cancellation_epoch(&self) -> u64 {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state
            .receiver
            .cancellation_epoch()
            .saturating_add(state.pen_epoch)
    }
    /// Retire classified pen authority, including already-drained batches,
    /// without changing genuine mouse GUI ownership or global publication guards.
    /// Returns the GUI-reset decision for immediate host-side private-batch
    /// retirement after releasing this lock. Hosts must discard their drained
    /// pending pen batch without calling global Source.cancel a second time.
    pub fn retire_pen(&self) -> bool {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.native.fence();
        state.pen_epoch = state.pen_epoch.saturating_add(1);
        if state.receiver.has_capture_or_samples() {
            state.receiver.cancel_pen_only();
        }
        state.receiver.reset_gui_pointer_pending()
    }
    pub fn cancel(&self) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.native_touch.fence_all();
        state.native.fence();
        state.receiver.cancel();
    }
    /// Display/window layout changed before a completed GUI pass. New pen
    /// contacts wait for a fresh view instead of using the old scale/origin.
    pub fn invalidate_view(&self) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.native_touch.fence_all();
        if state.receiver.has_capture_or_samples() {
            state.native.fence();
            state.receiver.cancel();
        }
        state.native.fence();
        state.window_epoch = state.window_epoch.saturating_add(1);
        state.view = None;
    }
    pub fn fault(&self) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.native_touch.fence_all();
        state.native.fence();
        state.receiver.fault();
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
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if held && state.native_touch.active() {
            return false;
        }
        let accepted = state.receiver.mouse_capture(held);
        if held && accepted {
            state.native.forget_button_edges();
        }
        accepted
    }
    /// Native hosts invoke this inside Installation::with_delivery, before editor
    /// work. A failed delivery discards even a fully queued but unpublished Up.
    pub fn take(&self, frame: u64, owner: Owner, delivery_failed: bool) -> Result<Batch> {
        let frame_id = frame;
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if delivery_failed {
            state.native_touch.fence_all();
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
                state.native_touch.fence_all();
                state.window_epoch = state.window_epoch.saturating_add(1);
            } else {
                state.native_touch.fence_canvas();
            }
            let canvas = state.receiver.has_canvas_capture_or_samples();
            if (different_session && state.receiver.has_capture_or_samples()) || canvas {
                state.native.fence();
                state.receiver.cancel();
            }
            if different_session || canvas {
                state.view = None;
            }
        }
        let mut storage = state.frames.account();
        let binding_bytes = if state.native_contacts.configured() {
            std::mem::size_of::<contact_tokens::Leases>()
        } else {
            0
        };
        let frame = state
            .receiver
            .take(frame, |bytes| {
                storage
                    .resize((bytes + binding_bytes) as u64)
                    .map_err(|_| Invalid::Capacity)
            })
            .map_err(invalid)?;
        let canvas_samples = !frame.samples.is_empty();
        let last_down = frame
            .samples
            .iter()
            .rfind(|s| s.phase == Phase::Down)
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
                pen_epoch: Some(state.pen_epoch),
                native_touch: None,
                native_contacts: state.native_contacts.take(frame_id),
                acknowledged: None,
                canvas_samples,
                last_down,
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
        self.current_binding(state, &batch.binding, live_owner)
    }
    fn current_binding(&self, state: &State, binding: &Binding, live_owner: Owner) -> bool {
        if binding.source.as_ptr() != Arc::as_ptr(&self.0)
            || binding.cancel_epoch == u64::MAX
            || binding.cancel_epoch != state.receiver.cancellation_epoch()
            || binding
                .pen_epoch
                .is_some_and(|epoch| epoch == u64::MAX || epoch != state.pen_epoch)
            || binding
                .native_touch
                .as_ref()
                .is_some_and(|lease| !lease.current())
            || binding.native_contacts.as_ref().is_some_and(|leases| {
                !leases.current() || !state.native_contacts.context_matches(leases, binding.owner)
            })
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
    reset_gui_pointer: bool,
}
impl Pass {
    pub fn begin(&mut self, frame: u64, input: &Frame) -> bool {
        if self.frame == Some(frame) {
            return false;
        }
        self.frame = Some(frame);
        self.suppress = input.suppress_legacy_pointer;
        self.cancelled = input.cancelled;
        self.reset_gui_pointer = input.reset_gui_pointer;
        true
    }
    pub fn suppress_legacy_pointer(&self) -> bool {
        self.suppress
    }
    pub fn cancelled(&self) -> bool {
        self.cancelled
    }
    pub fn reset_gui_pointer(&self) -> bool {
        self.reset_gui_pointer
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod retirement_tests;

#[cfg(test)]
mod button_edge_tests;
