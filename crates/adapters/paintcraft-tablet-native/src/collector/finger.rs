//! Bounded physical touch metadata, before any Source/GUI admission policy.
use super::*;
use paintcraft_input::tablet::{DeviceKind, FingerConfidence};
#[derive(Clone, Copy, Debug)]
struct Contact {
    identity: Identity,
    held: bool,
    eligible: bool,
    fenced: bool,
    primary: bool,
}
#[derive(Debug, Default)]
pub(super) struct Table([Option<Contact>; 32]);
impl Table {
    pub(super) fn held(&self, identity: Identity) -> bool {
        self.0
            .iter()
            .flatten()
            .any(|c| c.identity == identity && (c.held || c.fenced))
    }
    pub(super) fn fence(&mut self) {
        for c in self.0.iter_mut().flatten() {
            c.fenced |= c.held;
            c.eligible = false;
        }
    }
}
impl Collector {
    pub(super) fn collect_finger(
        &mut self,
        record: CaptureRecord,
    ) -> std::result::Result<FingerRoute, RecordFault> {
        let CaptureEvent::Finger {
            identity,
            phase,
            time,
            provenance,
            position,
            confidence,
            in_contact,
            primary,
        } = record.event
        else {
            return Err(RecordFault::Invalid);
        };
        // Predictions cannot poison actual ownership or clear physical tails,
        // including malformed predictions whose identity/clock is unavailable.
        if provenance == Provenance::Predicted {
            return Ok(FingerRoute::Suppressed);
        }
        // Native touch lifecycle is always source-authored Actual. Internal
        // cancellation boundaries are separate CaptureEvent::Boundary records,
        // never authority for finger contact, primary, confidence or release.
        if provenance == Provenance::Boundary {
            return Err(RecordFault::Invalid);
        }
        if identity.kind != DeviceKind::Finger
            || record.installation == 0
            || record.installation == u64::MAX
            || time.epoch != record.installation
        {
            return Err(RecordFault::Invalid);
        }
        if let Some(position) = position {
            validate_mouse(
                MouseRecord {
                    kind: MouseKind::Motion,
                    position,
                    client: true,
                    modifiers: Default::default(),
                    time,
                },
                record.installation,
            )?;
        }
        let index = self
            .fingers
            .0
            .iter()
            .position(|c| c.is_some_and(|c| c.identity == identity));
        let previous = index.and_then(|i| self.fingers.0[i]);
        if provenance == Provenance::Coalesced {
            // Historical physical transitions must not be hidden by a later
            // high-confidence main sample. Reject the whole undrained batch.
            if !matches!(phase, Phase::Move | Phase::Hover)
                || (phase == Phase::Move && (!in_contact || position.is_none()))
                || (phase == Phase::Hover && in_contact)
                || (previous.is_none() && in_contact)
                || previous.is_some_and(|c| {
                    c.held != in_contact
                        || c.primary != primary
                        || (c.eligible && confidence != FingerConfidence::High)
                })
            {
                return Err(RecordFault::Invalid);
            }
            self.observe_finger_clock(identity, time)?;
            return Ok(if previous.is_some_and(|c| c.eligible && !c.fenced) {
                FingerRoute::Eligible
            } else {
                FingerRoute::Suppressed
            });
        }
        if (matches!(phase, Phase::Down | Phase::Move) && !in_contact)
            || (matches!(phase, Phase::Up | Phase::Hover) && in_contact)
        {
            return Err(RecordFault::Invalid);
        }
        // Down/movement require an observed point. Real terminal/lifecycle
        // records remain useful without a current GUI mapping or coordinate.
        if matches!(phase, Phase::Down | Phase::Move) && position.is_none() {
            return Err(RecordFault::Invalid);
        }
        self.observe_finger_clock(identity, time)?;
        let terminal = !in_contact
            && matches!(
                phase,
                Phase::Up | Phase::Cancel | Phase::Leave | Phase::Hover
            );
        let mut contact = previous.unwrap_or(Contact {
            identity,
            held: false,
            eligible: false,
            fenced: false,
            primary,
        });
        let lost_authority = contact.eligible
            && (matches!(phase, Phase::Cancel | Phase::Leave)
                || (in_contact
                    && (confidence != FingerConfidence::High || contact.primary != primary))
                || (terminal && phase != Phase::Up));
        let route = if lost_authority {
            contact.eligible = false;
            contact.fenced |= in_contact;
            FingerRoute::PolicyCancel
        } else if phase == Phase::Down {
            if contact.eligible || contact.fenced {
                return Err(RecordFault::Invalid);
            }
            contact.fenced = confidence != FingerConfidence::High;
            contact.eligible = !contact.fenced;
            if contact.eligible {
                FingerRoute::Eligible
            } else {
                FingerRoute::Suppressed
            }
        } else if contact.eligible && !contact.fenced {
            FingerRoute::Eligible
        } else {
            // Enter can report physical contact before its actual Down edge.
            // It does not authorize a gesture or invent a Down.
            if in_contact && phase != Phase::Enter {
                contact.fenced = true;
            }
            FingerRoute::Suppressed
        };
        contact.held = in_contact;
        contact.primary = primary;
        if terminal {
            if let Some(i) = index {
                self.fingers.0[i] = None;
            }
        } else if in_contact || contact.fenced || contact.eligible {
            let i = index
                .or_else(|| self.fingers.0.iter().position(Option::is_none))
                .ok_or(RecordFault::Capacity)?;
            self.fingers.0[i] = Some(contact);
        }
        Ok(route)
    }
    fn observe_finger_clock(
        &mut self,
        identity: Identity,
        time: Time,
    ) -> std::result::Result<(), RecordFault> {
        let i = self
            .times
            .iter()
            .position(|v| v.is_some_and(|(id, _)| id == identity))
            .or_else(|| self.times.iter().position(Option::is_none))
            .ok_or(RecordFault::Capacity)?;
        if !ordered_time(self.times[i].map(|(_, time)| time), time) {
            return Err(RecordFault::Invalid);
        }
        self.times[i] = Some((identity, time));
        Ok(())
    }
}
#[cfg(test)]
mod tests;
