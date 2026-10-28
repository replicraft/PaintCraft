//! Per-pointer touch lifecycle. Its identity is never borrowed from a pen or
//! synthesized from pressure, a GUI event index, or a coordinate match.
use super::model::{CANCELED, CAPTURE_CHANGED, CONTACT, DOWN, IN_RANGE, Info, Message, UP};
use crate::WindowPosition;
use paintcraft_input::tablet::{Clock, DeviceKind, Identity, Invalid, Phase, Provenance, Time};
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Finger {
    pub identity: Identity,
    pub phase: Phase,
    pub time: Time,
    pub provenance: Provenance,
    pub position: Option<WindowPosition>,
    /// The SDK only asserts high confidence when the bit is present.
    /// None is unspecified, never a fabricated low/palm or high classification.
    pub confidence: Option<bool>,
    pub in_contact: bool,
    /// Actual system-designated PRIMARY bit, never a first-local-contact guess.
    pub primary: bool,
}
#[derive(Clone, Copy, Debug)]
struct Contact {
    identity: Identity,
    time: Time,
    held: bool,
    down_seen: bool,
    client: bool,
    fenced: bool,
    recorded: bool,
}
#[derive(Clone)]
pub(super) struct State {
    contacts: [Option<Contact>; 64],
    epoch: u64,
    frequency: u64,
    retired: Option<(Identity, Time)>,
}
impl State {
    pub fn new(epoch: u64, frequency: u64) -> Self {
        Self {
            contacts: [None; 64],
            epoch,
            frequency,
            retired: None,
        }
    }
    pub fn retired(&self) -> Option<(Identity, Time)> {
        self.retired
    }
    pub fn fence(&mut self) {
        for contact in self.contacts.iter_mut().flatten() {
            contact.fenced |= contact.held;
        }
    }
    #[cfg(test)]
    pub fn remove_device(&mut self, device: u64) {
        for contact in &mut self.contacts {
            if contact.is_some_and(|c| c.identity.device == device) {
                *contact = None;
            }
        }
    }
    pub fn history(
        &mut self,
        history: &[Info],
        message: Message,
        now: u64,
        output: &mut Vec<Finger>,
    ) -> Result<bool, Invalid> {
        output.clear();
        if history.is_empty() || history.len() > 4096 || output.capacity() < history.len() {
            return Err(Invalid::Capacity);
        }
        let current = history[0];
        if history
            .iter()
            .any(|p| p.device != current.device || p.pointer != current.pointer)
        {
            return Err(Invalid::Owner);
        }
        if history.windows(2).any(|p| p[0].frame == p[1].frame) {
            return Err(Invalid::Order);
        }
        let mut next = self.clone();
        let mut owned = false;
        for (index, info) in history.iter().enumerate().rev() {
            let (client, event) = match next.sample(
                *info,
                if index == 0 { message } else { Message::Update },
                index != 0,
                now,
            ) {
                Ok(value) => value,
                Err(error) => {
                    output.clear();
                    return Err(error);
                }
            };
            owned |= client;
            if let Some(event) = event {
                output.push(event);
            }
        }
        *self = next;
        Ok(owned)
    }
    fn sample(
        &mut self,
        info: Info,
        message: Message,
        historical: bool,
        now: u64,
    ) -> Result<(bool, Option<Finger>), Invalid> {
        self.retired = None;
        super::model::validate_contact_flags(info.flags)?;
        if info.device == 0 || info.pointer == 0 {
            return Err(Invalid::Owner);
        }
        if !info.position.0.is_finite() || !info.position.1.is_finite() {
            return Err(Invalid::Coordinates);
        }
        let identity = Identity {
            device: info.device,
            pointer: u64::from(info.pointer),
            kind: DeviceKind::Finger,
        };
        let index = self
            .contacts
            .iter()
            .position(|c| c.is_some_and(|c| c.identity == identity))
            .or_else(|| self.contacts.iter().position(Option::is_none))
            .ok_or(Invalid::Capacity)?;
        let old = self.contacts[index];
        // POINTER_INFO permits a zero timestamp field. Without independent
        // presence evidence, refuse ambiguous pointer zero conservatively; do
        // not substitute receipt time or change the selected clock domain.
        let time = match old.map(|c| c.time.clock) {
            Some(Clock::WindowsPerformance) => {
                super::clock::performance(info.performance, self.frequency, self.epoch)
            }
            Some(Clock::WindowsTicks) if info.ticks != 0 => {
                super::clock::ticks(info.ticks, now, self.epoch)
            }
            None if info.performance != 0 => {
                super::clock::performance(info.performance, self.frequency, self.epoch)
            }
            None if info.ticks != 0 => super::clock::ticks(info.ticks, now, self.epoch),
            _ => None,
        }
        .ok_or(Invalid::Clock)?;
        if old.is_some_and(|c| c.time.micros > time.micros) {
            return Err(Invalid::Order);
        }
        let held = info.flags & CONTACT != 0;
        if historical
            && (info.flags & (DOWN | UP | CANCELED | CAPTURE_CHANGED) != 0
                || old.map_or(held, |c| c.held != held))
        {
            return Err(Invalid::Lifecycle);
        }
        let terminal = info.flags & (CANCELED | CAPTURE_CHANGED) != 0 || message == Message::Leave;
        let phase = if terminal {
            if message == Message::Leave {
                Phase::Leave
            } else {
                Phase::Cancel
            }
        } else if message == Message::Enter {
            Phase::Enter
        } else if info.flags & DOWN != 0 {
            if old.is_some_and(|c| c.down_seen) || !held {
                return Err(Invalid::Lifecycle);
            }
            Phase::Down
        } else if info.flags & UP != 0 {
            Phase::Up
        } else if held {
            if !old.is_some_and(|c| c.down_seen) {
                return Err(Invalid::Lifecycle);
            }
            Phase::Move
        } else {
            Phase::Hover
        };
        if !historical
            && !terminal
            && match message {
                Message::Down => phase != Phase::Down,
                Message::Up => phase != Phase::Up,
                Message::Update => !matches!(phase, Phase::Move | Phase::Hover),
                _ => false,
            }
        {
            return Err(Invalid::Lifecycle);
        }
        let client = old
            .filter(|c| c.held || terminal)
            .map_or(info.client, |c| c.client);
        let fenced = old.is_some_and(|c| c.fenced) || terminal;
        let delivered = client && !(fenced && held && !terminal);
        let recorded = old.is_some_and(|c| c.recorded) || delivered;
        if message == Message::Leave && !held && info.flags & IN_RANGE == 0 && recorded {
            self.retired = Some((identity, time));
        }
        self.contacts[index] = if message == Message::Leave && !held && info.flags & IN_RANGE == 0 {
            None
        } else {
            Some(Contact {
                identity,
                time,
                held,
                down_seen: phase == Phase::Down
                    || (phase != Phase::Up && old.is_some_and(|p| p.down_seen)),
                client,
                fenced: fenced && held,
                recorded,
            })
        };
        let event = Finger {
            identity,
            phase,
            time,
            provenance: if historical {
                Provenance::Coalesced
            } else {
                Provenance::Actual
            },
            confidence: (info.flags & super::model::CONFIDENCE != 0).then_some(true),
            in_contact: held,
            primary: info.flags & super::model::PRIMARY != 0,
            position: if terminal {
                None
            } else {
                Some(WindowPosition::PhysicalPixels(
                    info.position.0,
                    info.position.1,
                ))
            },
        };
        Ok((
            client,
            if !client || (fenced && held && !terminal) {
                None
            } else {
                Some(event)
            },
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn info(flags: u32, time: u64) -> Info {
        Info {
            device: 23,
            pointer: 4,
            frame: time as u32,
            flags: flags | IN_RANGE,
            performance: time,
            client: true,
            position: (1.25, 4.5),
            ..Info::default()
        }
    }
    #[test]
    fn two_devices_with_the_same_pointer_number_are_distinct_fingers() {
        let mut state = State::new(3, 1000);
        let (_, a) = state
            .sample(info(DOWN | CONTACT, 1), Message::Down, false, 1000)
            .unwrap();
        let mut other = info(DOWN | CONTACT, 2);
        other.device = 24;
        let (_, b) = state.sample(other, Message::Down, false, 1000).unwrap();
        let a = a.unwrap();
        let b = b.unwrap();
        assert_eq!(a.identity.kind, DeviceKind::Finger);
        assert_ne!(a.identity, b.identity);
        assert_eq!(a.position, Some(WindowPosition::PhysicalPixels(1.25, 4.5)));
    }
    #[test]
    fn complete_history_is_validated_before_any_finger_prefix_escapes() {
        let mut state = State::new(3, 1000);
        state
            .sample(info(DOWN | CONTACT, 1), Message::Down, false, 1000)
            .unwrap();
        let mut output = Vec::with_capacity(8);
        assert!(
            state
                .history(
                    &[info(UP, 3), info(CONTACT, 2)],
                    Message::Up,
                    1000,
                    &mut output
                )
                .unwrap()
        );
        assert_eq!(
            output.iter().map(|f| f.phase).collect::<Vec<_>>(),
            [Phase::Move, Phase::Up]
        );
        let mut bad = info(CONTACT, 4);
        bad.device = 99;
        assert!(
            state
                .history(&[info(CONTACT, 5), bad], Message::Update, 1000, &mut output)
                .is_err()
        );
        assert!(output.is_empty());
    }
    #[test]
    fn cancelled_tail_does_not_invent_a_release_or_new_touch() {
        let mut state = State::new(3, 1000);
        state
            .sample(info(DOWN | CONTACT, 1), Message::Down, false, 1000)
            .unwrap();
        state.fence();
        assert_eq!(
            state
                .sample(info(CONTACT, 2), Message::Update, false, 1000)
                .unwrap()
                .1,
            None
        );
        assert_eq!(
            state
                .sample(info(UP, 3), Message::Up, false, 1000)
                .unwrap()
                .1
                .unwrap()
                .phase,
            Phase::Up
        );
        assert_eq!(
            state
                .sample(info(DOWN | CONTACT, 4), Message::Down, false, 1000)
                .unwrap()
                .1
                .unwrap()
                .phase,
            Phase::Down
        );
        state.remove_device(23);
        assert_eq!(
            state
                .sample(info(DOWN | CONTACT, 1), Message::Down, false, 1000)
                .unwrap()
                .1
                .unwrap()
                .phase,
            Phase::Down
        );
    }
}

#[cfg(test)]
mod confidence_controls {
    use super::*;
    fn sample(flags: u32, count: u64) -> Info {
        Info {
            device: 4,
            pointer: 6,
            frame: count as u32,
            flags: flags | IN_RANGE,
            performance: count,
            client: true,
            ..Info::default()
        }
    }
    #[test]
    fn primary_release_does_not_promote_another_native_finger() {
        let mut state = State::new(19, 1000);
        let primary = state
            .sample(
                sample(DOWN | CONTACT | super::super::model::PRIMARY, 1),
                Message::Down,
                false,
                1000,
            )
            .unwrap()
            .1
            .unwrap();
        assert!(primary.primary);
        let mut secondary = sample(DOWN | CONTACT, 2);
        secondary.pointer = 7;
        let finger = state
            .sample(secondary, Message::Down, false, 1000)
            .unwrap()
            .1
            .unwrap();
        assert!(!finger.primary);
        state
            .sample(
                sample(UP | super::super::model::PRIMARY, 3),
                Message::Up,
                false,
                1000,
            )
            .unwrap();
        secondary.flags = CONTACT | IN_RANGE;
        secondary.performance = 4;
        secondary.frame = 4;
        let remaining = state
            .sample(secondary, Message::Update, false, 1000)
            .unwrap()
            .1
            .unwrap();
        assert!(!remaining.primary);
        assert!(remaining.in_contact);
        assert_eq!(remaining.phase, Phase::Move);
    }
    #[test]
    fn absent_confidence_never_claims_palm_or_high_confidence_and_retains_real_phase() {
        let mut state = State::new(19, 1000);
        let unknown = state
            .sample(sample(DOWN | CONTACT, 1), Message::Down, false, 1000)
            .unwrap()
            .1
            .unwrap();
        assert_eq!(unknown.confidence, None);
        assert_eq!(unknown.phase, Phase::Down);
        let high = state
            .sample(
                sample(CONTACT | super::super::model::CONFIDENCE, 2),
                Message::Update,
                false,
                1000,
            )
            .unwrap()
            .1
            .unwrap();
        assert_eq!(high.confidence, Some(true));
        let withdrawn = state
            .sample(sample(CONTACT, 3), Message::Update, false, 1000)
            .unwrap()
            .1
            .unwrap();
        assert_eq!(withdrawn.confidence, None);
        assert_eq!(withdrawn.phase, Phase::Move);
        assert_eq!(withdrawn.provenance, Provenance::Actual);
        // Downstream policy cancellation must be a separate control; acquisition
        // never labels a normal confidence-withdrawn Move as native Cancel.
        let native_cancel = state
            .sample(sample(CONTACT | CANCELED, 4), Message::Update, false, 1000)
            .unwrap()
            .1
            .unwrap();
        assert_eq!(native_cancel.phase, Phase::Cancel);
        assert_eq!(native_cancel.provenance, Provenance::Actual);
    }
}

#[cfg(test)]
mod retirement_control {
    use super::*;
    #[test]
    fn only_true_range_end_retires_an_observed_finger_clock() {
        let mut state = State::new(3, 1000);
        let mut info = Info {
            device: 23,
            pointer: 4,
            frame: 1,
            flags: DOWN | CONTACT | IN_RANGE,
            performance: 1,
            client: true,
            ..Info::default()
        };
        state.sample(info, Message::Down, false, 1000).unwrap();
        info.flags = UP | IN_RANGE;
        info.performance = 2;
        state.sample(info, Message::Up, false, 1000).unwrap();
        assert_eq!(state.retired(), None);
        info.flags = IN_RANGE;
        info.performance = 3;
        state.sample(info, Message::Leave, false, 1000).unwrap();
        assert_eq!(state.retired(), None);
        info.flags = 0;
        info.performance = 4;
        state.sample(info, Message::Leave, false, 1000).unwrap();
        assert_eq!(state.retired().unwrap().0.kind, DeviceKind::Finger);
    }
}

#[cfg(test)]
mod proximity_control {
    use super::*;
    #[test]
    fn contact_enter_does_not_invent_down_or_strand_the_following_real_down() {
        let mut state = State::new(19, 1000);
        let info = Info {
            device: 23,
            pointer: 4,
            frame: 1,
            flags: DOWN | CONTACT | IN_RANGE,
            performance: 1,
            client: true,
            ..Info::default()
        };
        let enter = state
            .sample(info, Message::Enter, false, 1000)
            .unwrap()
            .1
            .unwrap();
        assert_eq!(enter.phase, Phase::Enter);
        assert!(enter.in_contact);
        let down = state
            .sample(info, Message::Down, false, 1000)
            .unwrap()
            .1
            .unwrap();
        assert_eq!(down.phase, Phase::Down);
        assert!(down.in_contact);
    }
}

#[cfg(test)]
mod contradictory_edges {
    use super::*;
    #[test]
    fn malformed_touch_up_cannot_emit_a_valid_history_prefix_or_release_the_hold() {
        let mut state = State::new(19, 1000);
        let mut down = Info {
            device: 23,
            pointer: 4,
            frame: 1,
            flags: DOWN | CONTACT | IN_RANGE,
            performance: 1,
            client: true,
            ..Info::default()
        };
        state.sample(down, Message::Down, false, 1000).unwrap();
        let mut movement = down;
        movement.flags = CONTACT | IN_RANGE;
        movement.frame = 2;
        movement.performance = 2;
        let mut malformed = movement;
        malformed.flags |= UP;
        malformed.frame = 3;
        malformed.performance = 3;
        let mut output = Vec::with_capacity(4);
        assert!(
            state
                .history(&[malformed, movement], Message::Up, 1000, &mut output)
                .is_err()
        );
        assert!(output.is_empty());
        down.flags = UP | IN_RANGE;
        down.performance = 4;
        assert_eq!(
            state
                .sample(down, Message::Up, false, 1000)
                .unwrap()
                .1
                .unwrap()
                .phase,
            Phase::Up
        );
    }
}

#[cfg(test)]
mod native_clock_control {
    use super::*;
    #[test]
    fn touch_cannot_manufacture_an_absent_clock_or_switch_a_selected_domain() {
        let mut state = State::new(19, 1000);
        let mut input = Info {
            device: 23,
            pointer: 4,
            frame: 1,
            flags: IN_RANGE,
            client: true,
            ..Info::default()
        };
        assert_eq!(
            state.sample(input, Message::Enter, false, 1000),
            Err(Invalid::Clock)
        );
        input.ticks = 12;
        state.sample(input, Message::Enter, false, 1000).unwrap();
        input.ticks = 0;
        input.performance = 100;
        assert_eq!(
            state.sample(input, Message::Update, false, 1000),
            Err(Invalid::Clock)
        );
    }
}

#[cfg(test)]
mod coordinate_free_control {
    use super::*;
    #[test]
    fn known_idle_client_control_is_retained_but_native_only_has_no_new_permission() {
        let mut state = State::new(19, 1000);
        let mut input = Info {
            device: 23,
            pointer: 4,
            frame: 1,
            flags: IN_RANGE,
            performance: 1,
            client: true,
            ..Info::default()
        };
        state.sample(input, Message::Enter, false, 1000).unwrap();
        input.flags = CANCELED;
        input.client = false;
        input.performance = 2;
        input.position = (90_000.0, -40_000.0);
        let cancelled = state
            .sample(input, Message::Update, false, 1000)
            .unwrap()
            .1
            .unwrap();
        assert_eq!(cancelled.phase, Phase::Cancel);
        assert_eq!(cancelled.position, None);
        input.flags = 0;
        input.performance = 3;
        assert_eq!(
            state
                .sample(input, Message::Leave, false, 1000)
                .unwrap()
                .1
                .unwrap()
                .phase,
            Phase::Leave
        );
        input.pointer = 88;
        input.flags = CANCELED;
        input.performance = 4;
        assert_eq!(
            state.sample(input, Message::Update, false, 1000).unwrap().1,
            None
        );
    }
}
