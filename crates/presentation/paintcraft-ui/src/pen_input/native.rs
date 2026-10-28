//! Coordinate-free native contact bookkeeping survives invalid GUI layouts.
use super::*;
use paintcraft_input::tablet::{Buttons, Identity, Provenance, Time};
#[derive(Clone, Copy, Debug)]
struct Contact {
    identity: Identity,
    time: Time,
    sequence: u64,
    held: bool,
    down_seen: bool,
    fenced: bool,
}
/// Fixed metadata only; no event callback allocation or invented coordinates.
#[derive(Debug)]
pub(super) struct Metadata([Option<Contact>; 64]);
impl Default for Metadata {
    fn default() -> Self {
        Self([None; 64])
    }
}
impl Metadata {
    pub(super) fn fence(&mut self) {
        for contact in self.0.iter_mut().flatten() {
            contact.fenced |= contact.held;
        }
    }
    fn deny(&mut self, identity: Identity) {
        if let Some(contact) = self.0.iter_mut().flatten().find(|c| c.identity == identity) {
            contact.fenced |= contact.held;
        }
    }
    fn observe(&mut self, raw: RawSample, usable_view: bool) -> std::result::Result<bool, Invalid> {
        if raw.provenance == Provenance::Predicted {
            return Ok(false);
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
        let observed = matches!(raw.provenance, Provenance::Actual | Provenance::Coalesced);
        let buttons = raw.buttons.0 & (Buttons::TIP | Buttons::BARREL | Buttons::BARREL_UPPER);
        let terminal = observed
            && buttons == 0
            && matches!(raw.phase, Phase::Up | Phase::Hover | Phase::Leave);
        let held =
            !terminal && (raw.phase == Phase::Down || buttons != 0 || old.is_some_and(|c| c.held));
        let down_seen = raw.phase == Phase::Down
            || (raw.phase != Phase::Up && !terminal && old.is_some_and(|c| c.down_seen));
        let fenced = old.is_some_and(|c| c.fenced)
            || (!usable_view && held)
            || (matches!(raw.phase, Phase::Cancel | Phase::Leave) && held);
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
            });
        } else if let Some(index) = index {
            self.0[index] = None;
        }
        // Proximity or first observed held motion is not a new tip Down. A
        // real subsequent Down may start it; barrel-only hover remains usable.
        let tip_without_down = buttons & Buttons::TIP != 0 && !down_seen;
        Ok(usable_view && !fenced && !tip_without_down)
    }
}
impl Source {
    /// Observe a real native tail rejected by upstream first-device policy.
    /// This must still see its terminal while coordinate delivery is suppressed.
    pub fn suppress_native(&self, raw: RawSample) -> std::result::Result<Route, Invalid> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(error) = state.native.observe(raw, false) {
            state.native.fence();
            state.receiver.cancel();
            return Err(error);
        }
        if raw.provenance != Provenance::Predicted {
            state.receiver.cancel_identity(raw.identity);
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
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.submit_native(delivery, raw, convert)
    }
}
impl State {
    pub(super) fn submit_native(
        &mut self,
        delivery: Delivery,
        mut raw: RawSample,
        convert: impl FnOnce(f64) -> std::result::Result<Option<(f64, f64)>, Invalid>,
    ) -> std::result::Result<Route, Invalid> {
        let state = self;
        let view = state.view;
        let allowed = match state
            .native
            .observe(raw, view.is_some_and(|view| view.focused))
        {
            Ok(allowed) => allowed,
            Err(error) => {
                state.native.fence();
                state.receiver.cancel();
                return Err(error);
            }
        };
        if !allowed {
            if raw.provenance == Provenance::Predicted {
                return Ok(Route::Suppressed);
            }
            // A denied terminal retires only its own receiver state; it cannot
            // cancel another pen. This is policy cancellation, never a raw Up.
            state.receiver.cancel_identity(raw.identity);
            return Ok(Route::Suppressed);
        }
        let Some(view) = view else {
            return Ok(Route::Suppressed);
        };
        raw.logical_position = match convert(view.pixels_per_point) {
            Ok(position) => position,
            Err(error) => {
                state.native.fence();
                state.receiver.cancel_identity(raw.identity);
                return Err(error);
            }
        };
        let result = state.receiver.submit(
            raw,
            view.transform,
            view.permissions(raw.logical_position),
            delivery,
        );
        if !matches!(&result, Ok(route) if *route != Route::Suppressed) {
            state.native.deny(raw.identity);
        }
        result
    }
}

#[cfg(test)]
mod tests;
