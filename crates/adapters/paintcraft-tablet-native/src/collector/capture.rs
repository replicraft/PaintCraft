//! Physical qualification is separate from later Source/GUI permissions.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureClass {
    Idle,
    Mouse,
    Pen(Identity),
    Touch,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FingerDenial {
    CompetingPointer,
    AwaitingMouseIdle,
    UnspecifiedConfidence,
    OrphanContact,
    RetiredContext,
    PrimaryChanged,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Idle {
    identity: Identity,
    time: Time,
    ordinal: u64,
}
impl Collector {
    /// Ownership, not readiness: an uncertain lost mouse hold separately fences
    /// admission until a real native idle observation. No guessed mouse Up.
    pub fn capture_class(&self) -> CaptureClass {
        if self.mouse_uncertain {
            CaptureClass::Idle
        } else if self.mouse != 0 && self.mouse_route != MouseRoute::Suppressed {
            CaptureClass::Mouse
        } else if let Some(identity) = self.owner {
            CaptureClass::Pen(identity)
        } else if self.fingers.active() {
            CaptureClass::Touch
        } else {
            CaptureClass::Idle
        }
    }
    pub(super) fn competing_finger_denial(&self) -> Option<FingerDenial> {
        if self.mouse_uncertain {
            Some(FingerDenial::AwaitingMouseIdle)
        } else if matches!(
            self.capture_class(),
            CaptureClass::Mouse | CaptureClass::Pen(_)
        ) {
            Some(FingerDenial::CompetingPointer)
        } else {
            None
        }
    }
    pub(super) fn pen_qualifies(
        &self,
        packet: NativePacket,
        clock: usize,
        pen: Pen,
    ) -> std::result::Result<bool, RecordFault> {
        let raw = packet.raw;
        if let Some(edge) = packet.button_edge
            && (edge.identity != raw.identity
                || raw.provenance != Provenance::Actual
                || (raw.buttons.0 & edge.button.mask() != 0) != edge.pressed)
        {
            return Err(RecordFault::Invalid);
        }
        if raw.provenance == Provenance::Actual
            && raw.phase == Phase::Down
            && raw.buttons.0 & Buttons::TIP == 0
        {
            return Err(RecordFault::Invalid);
        }
        if pen.fenced {
            return Ok(false);
        }
        if pen.start.is_some() {
            return Ok(true);
        }
        if raw.provenance != Provenance::Actual {
            return Ok(false);
        }
        let tip_down = raw.phase == Phase::Down && raw.buttons.0 & Buttons::TIP != 0;
        let side_press = packet.button_edge.is_some_and(|edge| edge.pressed);
        let prior_idle = self.idle[clock].is_some_and(|idle| {
            idle.identity == raw.identity
                && idle.time.clock == raw.time.clock
                && idle.time.epoch == raw.time.epoch
                && idle.time.micros <= raw.time.micros
                && idle.ordinal < raw.sequence
        });
        Ok(tip_down
            || side_press
            || (prior_idle && raw.buttons.0 & (Buttons::BARREL | Buttons::BARREL_UPPER) != 0))
    }
    pub(super) fn observe_pen_idle(
        &mut self,
        raw: paintcraft_input::tablet::RawSample,
        clock: usize,
        held: bool,
    ) {
        if raw.provenance != Provenance::Actual {
            return;
        }
        if held || raw.phase == Phase::Leave {
            self.idle[clock] = None;
        } else if raw.buttons.0 & (Buttons::TIP | Buttons::BARREL | Buttons::BARREL_UPPER) == 0
            && matches!(raw.phase, Phase::Up | Phase::Hover | Phase::Enter)
        {
            self.idle[clock] = Some(Idle {
                identity: raw.identity,
                time: raw.time,
                ordinal: raw.sequence,
            });
        }
    }
    pub(super) fn fence_orphans(&mut self) {
        for pen in self.pens.iter_mut().flatten() {
            if pen.held && pen.orphan {
                pen.fenced = true;
                pen.orphan = false;
            }
        }
    }
}
#[cfg(test)]
mod tests;
