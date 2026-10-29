//! Coordinate-free native contact bookkeeping survives invalid GUI layouts.
use super::*;
use paintcraft_input::tablet::{Buttons, Identity, PenButtonEdge, Provenance, Time};
#[derive(Clone, Copy, Debug)]
struct Contact {
    identity: Identity,
    time: Time,
    sequence: u64,
    held: bool,
    down_seen: bool,
    fenced: bool,
    orphan: bool,
}
/// Fixed metadata only; no event callback allocation or invented coordinates.
#[derive(Debug)]
pub(super) struct Metadata([Option<Contact>; 64], [Option<Idle>; 64], usize);
#[derive(Clone, Copy, Debug)]
struct Idle {
    identity: Identity,
    time: Time,
    sequence: u64,
}
impl Default for Metadata {
    fn default() -> Self {
        Self([None; 64], [None; 64], 0)
    }
}
impl Metadata {
    fn idle(&self, identity: Identity) -> bool {
        !self
            .0
            .iter()
            .flatten()
            .any(|c| c.identity == identity && (c.held || c.fenced))
    }
    pub(super) fn forget_button_edges(&mut self) {
        self.1 = [None; 64];
    }
    fn forget_idle(&mut self, identity: Identity) {
        for slot in &mut self.1 {
            if slot.is_some_and(|idle| idle.identity == identity) {
                *slot = None;
            }
        }
    }
    fn remember_idle(&mut self, raw: RawSample) {
        let slot = self
            .1
            .iter()
            .position(|entry| entry.is_some_and(|idle| idle.identity == raw.identity))
            .or_else(|| self.1.iter().position(Option::is_none))
            .unwrap_or_else(|| {
                // Evicting an idle observation only removes edge authority. It
                // never reclaims native Collector clocks or a held/fenced tail.
                let slot = self.2;
                self.2 = (self.2 + 1) % self.1.len();
                slot
            });
        self.1[slot] = Some(Idle {
            identity: raw.identity,
            time: raw.time,
            sequence: raw.sequence,
        });
    }
    fn observed_idle(&self, raw: RawSample) -> bool {
        raw.provenance == Provenance::Actual
            && self.1.iter().flatten().any(|idle| {
                idle.identity == raw.identity
                    && idle.time.clock == raw.time.clock
                    && idle.time.epoch == raw.time.epoch
                    && idle.time.micros <= raw.time.micros
                    && idle.sequence < raw.sequence
            })
    }
    pub(super) fn fence(&mut self) {
        self.forget_button_edges();
        for contact in self.0.iter_mut().flatten() {
            contact.fenced |= contact.held;
            contact.orphan = false;
        }
    }
    pub(super) fn deny(&mut self, identity: Identity) {
        self.forget_idle(identity);
        if let Some(contact) = self.0.iter_mut().flatten().find(|c| c.identity == identity) {
            contact.fenced |= contact.held;
            contact.orphan = false;
        }
    }
    fn observe(
        &mut self,
        raw: RawSample,
        usable_view: bool,
        edge: Option<PenButtonEdge>,
    ) -> std::result::Result<bool, Invalid> {
        if raw.provenance == Provenance::Predicted
            || (raw.provenance == Provenance::Coalesced
                && !matches!(raw.phase, Phase::Move | Phase::Hover))
        {
            return Ok(false);
        }
        if let Some(edge) = edge {
            if edge.identity != raw.identity {
                return Err(Invalid::Owner);
            }
            if raw.provenance != Provenance::Actual
                || (raw.buttons.0 & edge.button.mask() != 0) != edge.pressed
            {
                return Err(Invalid::Order);
            }
        }
        if !raw.identity.kind.is_pen() || raw.logical_position.is_some() {
            return Err(Invalid::Owner);
        }
        if raw.sequence == u64::MAX
            || (raw.provenance == Provenance::Boundary
                && !matches!(raw.phase, Phase::Cancel | Phase::Leave))
        {
            return Err(Invalid::Order);
        }
        let index = self
            .0
            .iter()
            .position(|entry| entry.is_some_and(|c| c.identity == raw.identity));
        let old = index.and_then(|index| self.0[index]);
        if let Some(old) = old {
            if old.time.clock != raw.time.clock
                || old.time.epoch != raw.time.epoch
                || old.time.micros > raw.time.micros
            {
                return Err(Invalid::Clock);
            }
            if old.sequence > raw.sequence {
                return Err(Invalid::Order);
            }
        }
        let buttons = raw.buttons.0 & (Buttons::TIP | Buttons::BARREL | Buttons::BARREL_UPPER);
        let terminal = raw.provenance == Provenance::Actual
            && buttons == 0
            && matches!(raw.phase, Phase::Up | Phase::Hover | Phase::Leave);
        let held = !terminal
            && (matches!(raw.phase, Phase::Down | Phase::Move)
                || buttons != 0
                || old.is_some_and(|c| c.held));
        let down_seen = (raw.provenance == Provenance::Actual && raw.phase == Phase::Down)
            || (raw.phase != Phase::Up && !terminal && old.is_some_and(|c| c.down_seen));
        let explicit_press = edge.is_some_and(|edge| edge.pressed)
            || (raw.provenance == Provenance::Actual && raw.phase == Phase::Down);
        let prior_idle = self.observed_idle(raw);
        let unproved_hold = (buttons & (Buttons::BARREL | Buttons::BARREL_UPPER) != 0
            || raw.phase == Phase::Move)
            && !down_seen
            && !old.is_some_and(|c| c.held && !c.fenced)
            && !explicit_press
            && !prior_idle;
        let strong_fence = (!usable_view && held)
            || (matches!(raw.phase, Phase::Cancel | Phase::Leave) && held)
            || old.is_some_and(|c| c.fenced && !c.orphan);
        let orphan = !strong_fence
            && (unproved_hold || old.is_some_and(|c| c.orphan && !explicit_press && !prior_idle));
        let fenced = strong_fence || orphan;
        if raw.provenance == Provenance::Actual && (held || raw.phase == Phase::Leave) {
            self.forget_idle(raw.identity);
        } else if raw.provenance == Provenance::Actual
            && buttons == 0
            && matches!(raw.phase, Phase::Up | Phase::Hover | Phase::Enter)
        {
            self.remember_idle(raw);
        }
        if held || (fenced && !terminal) {
            let index = index
                .or_else(|| self.0.iter().position(Option::is_none))
                .ok_or(Invalid::Capacity)?;
            self.0[index] = Some(Contact {
                identity: raw.identity,
                time: raw.time,
                sequence: raw.sequence,
                held,
                down_seen,
                fenced,
                orphan,
            });
        } else if let Some(index) = index {
            self.0[index] = None;
        }
        // Proximity or first observed held motion is not a new tip Down. A
        // real subsequent Down may start it. Move denotes contact motion even
        // when inverted contact uses ERASER instead of the ordinary TIP bit.
        let contact_without_down =
            (buttons & Buttons::TIP != 0 || raw.phase == Phase::Move) && !down_seen;
        Ok(usable_view && !fenced && !contact_without_down)
    }
}
impl Source {
    /// Additional idle check for a true lifetime-end record already verified by
    /// the native Collector (installation, identity and ordered source clock).
    /// This never cancels or changes admitted completed batches.
    pub fn retire_native_identity(&self, identity: Identity) -> std::result::Result<(), Invalid> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.native.idle(identity) && !state.receiver.captures_identity(identity) {
            state.native.forget_idle(identity);
            Ok(())
        } else {
            Err(Invalid::Owner)
        }
    }
    /// Observe a real native tail rejected by upstream first-device policy.
    /// This must still see its terminal while coordinate delivery is suppressed.
    pub fn suppress_native(&self, raw: RawSample) -> std::result::Result<Route, Invalid> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(error) = state.native.observe(raw, false, None) {
            if error == Invalid::Capacity {
                state.native.fence();
                state.receiver.fault();
            } else {
                state.native.deny(raw.identity);
                state.cancel_pen_identity(raw.identity);
            }
            return Err(error);
        }
        if raw.provenance != Provenance::Predicted {
            state.cancel_pen_identity(raw.identity);
        }
        Ok(Route::Suppressed)
    }
    /// Observe real identity/phase/buttons/clock before coordinate conversion.
    /// `raw.logical_position` must be None. Conversion runs only with a current
    /// view. A denied held contact stays fenced until its own real terminal;
    /// predictions/internal cancellation cannot clear it. Eraser identity alone
    /// is not a held button. No source sample is fabricated for recovery.
    pub fn submit_native(
        &self,
        delivery: Delivery,
        raw: RawSample,
        convert: impl FnOnce(f64) -> std::result::Result<Option<(f64, f64)>, Invalid>,
    ) -> std::result::Result<Route, Invalid> {
        self.submit_native_with_button_edge(delivery, raw, None, convert)
    }
    /// Side-button proof belongs to this actual source record and identity; it
    /// never rewrites contact Phase or promotes history/prediction to an edge.
    pub fn submit_native_with_button_edge(
        &self,
        delivery: Delivery,
        raw: RawSample,
        edge: Option<PenButtonEdge>,
        convert: impl FnOnce(f64) -> std::result::Result<Option<(f64, f64)>, Invalid>,
    ) -> std::result::Result<Route, Invalid> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.submit_native(delivery, raw, edge, convert)
    }
}
impl State {
    fn cancel_pen_identity(&mut self, identity: Identity) {
        if self.receiver.captures_identity(identity) {
            self.pen_epoch = self.pen_epoch.saturating_add(1);
            self.receiver.cancel_pen_only();
        }
    }
    pub(super) fn submit_native(
        &mut self,
        delivery: Delivery,
        mut raw: RawSample,
        edge: Option<PenButtonEdge>,
        convert: impl FnOnce(f64) -> std::result::Result<Option<(f64, f64)>, Invalid>,
    ) -> std::result::Result<Route, Invalid> {
        let state = self;
        if state.native_touch.active() {
            state.native.observe(raw, false, edge)?;
            return Ok(Route::Suppressed);
        }
        let view = state.view;
        let allowed = match state
            .native
            .observe(raw, view.is_some_and(|view| view.focused), edge)
        {
            Ok(allowed) => allowed,
            Err(error) => {
                state.native.fence();
                state.receiver.cancel();
                return Err(error);
            }
        };
        if raw.phase == Phase::Leave
            && matches!(raw.provenance, Provenance::Actual | Provenance::Coalesced)
            && raw.buttons.0 & (Buttons::TIP | Buttons::BARREL | Buttons::BARREL_UPPER) == 0
        {
            // An idle range/window exit after real Up is not cancellation of a
            // completed handoff. Only an actually held matching stream retires.
            state.cancel_pen_identity(raw.identity);
            return Ok(Route::Suppressed);
        }
        if !allowed {
            if raw.provenance == Provenance::Predicted
                || (raw.provenance == Provenance::Coalesced
                    && !matches!(raw.phase, Phase::Move | Phase::Hover))
            {
                return Ok(Route::Suppressed);
            }
            // A denied terminal retires only its own receiver state; it cannot
            // cancel another pen. This is policy cancellation, never a raw Up.
            state.cancel_pen_identity(raw.identity);
            return Ok(Route::Suppressed);
        }
        let Some(view) = view else {
            return Ok(Route::Suppressed);
        };
        raw.logical_position = match convert(view.pixels_per_point) {
            Ok(position) => position,
            Err(error) => {
                state.native.fence();
                state.cancel_pen_identity(raw.identity);
                return Err(error);
            }
        };
        state
            .native_contacts
            .reconcile(state.receiver.capture_status().pen);
        let result = state.receiver.submit(
            raw,
            view.transform,
            view.permissions(raw.logical_position),
            delivery,
        );
        if let Ok(route) = result
            && let Err(error) = state.native_contacts.observe(raw, route, view.owner)
        {
            state.native.fence();
            state.receiver.cancel();
            return Err(error);
        }
        if !matches!(&result, Ok(route) if *route != Route::Suppressed) {
            state.native.deny(raw.identity);
        }
        result
    }
}

#[cfg(test)]
mod tests;
