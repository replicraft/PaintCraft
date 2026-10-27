//! Shared pen ingress. Native delivery gates may call only the context-free
//! `Source` methods; GUI/editor work happens after the gate and ingress lock end.
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{AllocationClass, BudgetAccount, BudgetScope, SharedBudget},
};
use paintcraft_input::tablet::{
    Invalid, Permissions, Phase, RawSample, Sample, Transform,
    receiver::{Delivery, Frame, Receiver, Route, UiEvent},
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
}
/// Cloneable endpoint for native sinks/DOM listeners; it never touches egui Context.
#[derive(Clone, Debug)]
pub struct Source(Arc<Mutex<State>>);
/// A drained payload owns its resource declaration until source/UI Vecs drop.
#[derive(Debug)]
pub struct Batch {
    pub frame: Frame,
    _storage: BudgetAccount,
}
impl Source {
    pub fn new(source_capacity: usize, ui_capacity: usize, budget: &SharedBudget) -> Result<Self> {
        if !(2..=65_537).contains(&source_capacity) || !(1..=196_608).contains(&ui_capacity) {
            return Err(invalid(Invalid::Capacity));
        }
        let bytes = source_capacity * std::mem::size_of::<Sample>()
            + (ui_capacity + 3) * std::mem::size_of::<UiEvent>();
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
        let retire_canvas = state
            .view
            .is_some_and(|old| old.owner != view.owner || old.transform != view.transform);
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
        Ok(Batch {
            frame,
            _storage: storage,
        })
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
