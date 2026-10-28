//! Checked pen semantics, independent of OS calls and GUI work.
use crate::{Delivery, NativePacket, WindowPosition};
use paintcraft_input::{
    Modifiers,
    tablet::{
        Buttons, Clock, DeviceKind, Identity, Invalid, Phase, Provenance, RawSample, Tilt, Time,
    },
};

// Windows SDK flags. The Windows binding control verifies these identities.
pub(super) const IN_RANGE: u32 = 2;
pub(super) const CONTACT: u32 = 4;
pub(super) const CONFIDENCE: u32 = 0x4000;
pub(super) const CANCELED: u32 = 0x8000;
pub(super) const DOWN: u32 = 0x10000;
#[cfg(test)]
pub(super) const UPDATE: u32 = 0x20000;
pub(super) const UP: u32 = 0x40000;
pub(super) const CAPTURE_CHANGED: u32 = 0x200000;
pub(super) const BARREL: u32 = 1;
pub(super) const INVERTED: u32 = 2;
pub(super) const ERASER: u32 = 4;
pub(super) const PRESSURE: u32 = 1;
pub(super) const ROTATION: u32 = 2;
pub(super) const TILT_X: u32 = 4;
pub(super) const TILT_Y: u32 = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Message {
    Down,
    Up,
    #[default]
    Update,
    Enter,
    Leave,
}

/// Values copied from an actual POINTER_PEN_INFO while its message is current.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Info {
    pub device: u64,
    pub pointer: u32,
    pub frame: u32,
    pub flags: u32,
    pub pen_flags: u32,
    pub mask: u32,
    pub pressure: u32,
    pub rotation: u32,
    pub tilt: (i32, i32),
    pub performance: u64,
    pub ticks: u32,
    pub position: (f64, f64),
    pub client: bool,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, Debug)]
struct Pointer {
    device: u64,
    pointer: u32,
    inverted: bool,
    recorded_ends: u8,
    contact: bool,
    down_seen: bool,
    held: bool,
    fenced: bool,
    client: bool,
    time: Time,
}

#[derive(Clone, Debug)]
pub(super) struct State {
    pointers: [Option<Pointer>; 64],
    pub epoch: u64,
    frequency: u64,
    retired: [Option<(Identity, Time)>; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Output {
    /// Native chrome keeps its entire contact; no client canvas/UI record.
    Native,
    /// Explicitly cancelled physical tail, never a new GUI or ink gesture.
    Suppressed,
    Packet(NativePacket),
}

impl State {
    pub fn new(epoch: u64, frequency: u64) -> Self {
        Self {
            pointers: [None; 64],
            epoch,
            frequency,
            retired: [None; 2],
        }
    }

    pub fn retired(&self) -> [Option<(Identity, Time)>; 2] {
        self.retired
    }

    pub fn fence(&mut self) {
        for pointer in self.pointers.iter_mut().flatten() {
            pointer.fenced |= pointer.held;
        }
    }

    pub fn remove_device(&mut self, device: u64) {
        for pointer in &mut self.pointers {
            if pointer.is_some_and(|p| p.device == device) {
                *pointer = None;
            }
        }
    }

    /// Validate the complete bounded history before the caller emits any record.
    /// Windows returns newest first; reversing that documented array is not a
    /// timestamp sort and cannot move phase edges across other retrieved MSGs.
    pub fn history(
        &mut self,
        newest_first: &[Info],
        message: Message,
        now_millis: u64,
        output: &mut Vec<Output>,
    ) -> Result<(), Invalid> {
        output.clear();
        if newest_first.is_empty()
            || newest_first.len() > 4096
            || output.capacity() < newest_first.len()
        {
            return Err(Invalid::Capacity);
        }
        let current = newest_first[0];
        if newest_first
            .iter()
            .any(|p| p.pointer != current.pointer || p.device != current.device)
        {
            return Err(Invalid::Owner);
        }
        if newest_first
            .windows(2)
            .any(|pair| pair[0].frame == pair[1].frame)
        {
            return Err(Invalid::Order);
        }
        let mut next = self.clone();
        for (index, info) in newest_first.iter().enumerate().rev() {
            match next.sample(
                *info,
                if index == 0 { message } else { Message::Update },
                index != 0,
                now_millis,
            ) {
                Ok(event) => output.push(event),
                Err(error) => {
                    output.clear();
                    return Err(error);
                }
            }
        }
        *self = next;
        Ok(())
    }

    pub fn sample(
        &mut self,
        info: Info,
        message: Message,
        historical: bool,
        now_millis: u64,
    ) -> Result<Output, Invalid> {
        self.retired = [None; 2];
        validate_contact_flags(info.flags)?;
        if info.device == 0 || info.pointer == 0 {
            return Err(Invalid::Owner);
        }
        // Only moves are coalesced into later messages. A delayed Down/Up must
        // never acquire a fictitious position in the producer's edge order.
        if historical && info.flags & (DOWN | UP | CANCELED | CAPTURE_CHANGED) != 0 {
            return Err(Invalid::Lifecycle);
        }
        let index = self
            .pointers
            .iter()
            .position(|p| p.is_some_and(|p| p.device == info.device && p.pointer == info.pointer))
            .or_else(|| self.pointers.iter().position(Option::is_none))
            .ok_or(Invalid::Capacity)?;
        let old = self.pointers[index];
        // POINTER_INFO permits a zero timestamp field. Without independent
        // presence evidence, refuse ambiguous pointer zero conservatively; do
        // not substitute receipt time or change the selected clock domain.
        let time = match old.map(|p| p.time.clock) {
            Some(Clock::WindowsPerformance) => {
                super::clock::performance(info.performance, self.frequency, self.epoch)
            }
            Some(Clock::WindowsTicks) if info.ticks != 0 => {
                super::clock::ticks(info.ticks, now_millis, self.epoch)
            }
            None if info.performance != 0 => {
                super::clock::performance(info.performance, self.frequency, self.epoch)
            }
            None if info.ticks != 0 => super::clock::ticks(info.ticks, now_millis, self.epoch),
            _ => None,
        }
        .ok_or(Invalid::Clock)?;
        if old.is_some_and(|p| p.time.micros > time.micros) {
            return Err(Invalid::Order);
        }
        let reported_inverted = info.pen_flags & (INVERTED | ERASER) != 0;
        let contact = info.flags & CONTACT != 0;
        let barrel = info.pen_flags & BARREL != 0;
        let held = contact || barrel;
        // Only positional moves may be coalesced behind the current MSG. A
        // barrel edge hidden in older history cannot be placed before unrelated
        // retrieved mouse/keyboard records by timestamp sorting.
        if historical && old.map_or(held, |p| p.held != held || (p.contact != contact)) {
            return Err(Invalid::Lifecycle);
        }
        if old.is_some_and(|p| p.held && contact && p.inverted != reported_inverted) {
            return Err(Invalid::Lifecycle);
        }
        // ERASER is a contact flag; an Up may omit it. Never change the captured
        // end identity while delivering that held contact's real release.
        let inverted = old
            .filter(|p| p.held || message == Message::Leave)
            .map_or(reported_inverted, |p| p.inverted);
        let cancelled = info.flags & (CANCELED | CAPTURE_CHANGED) != 0 || message == Message::Leave;
        let phase = if cancelled {
            if message == Message::Leave {
                Phase::Leave
            } else {
                Phase::Cancel
            }
        } else if message == Message::Enter {
            Phase::Enter
        } else if info.flags & DOWN != 0 {
            if old.is_some_and(|p| p.down_seen) || !contact {
                return Err(Invalid::Lifecycle);
            }
            Phase::Down
        } else if info.flags & UP != 0 {
            Phase::Up
        } else if contact {
            if !old.is_some_and(|p| p.down_seen) {
                return Err(Invalid::Lifecycle);
            }
            Phase::Move
        } else {
            Phase::Hover
        };
        if !historical
            && !cancelled
            && match message {
                Message::Down => phase != Phase::Down,
                Message::Up => phase != Phase::Up,
                Message::Update => !matches!(phase, Phase::Move | Phase::Hover),
                _ => false,
            }
        {
            return Err(Invalid::Lifecycle);
        }
        let client = old.filter(|p| p.held).map_or(info.client, |p| p.client);
        let fenced = old.is_some_and(|p| p.fenced) || cancelled;
        let raw = RawSample {
            identity: Identity {
                device: info.device,
                pointer: u64::from(info.pointer),
                kind: if inverted {
                    DeviceKind::Eraser
                } else {
                    DeviceKind::Pen
                },
            },
            sequence: 0, // The common emitter assigns the producer ordinal.
            time,
            phase,
            logical_position: None,
            buttons: Buttons(
                (if contact { Buttons::TIP } else { 0 })
                    | (if barrel { Buttons::BARREL } else { 0 })
                    | (if inverted { Buttons::ERASER } else { 0 }),
            ),
            modifiers: info.modifiers,
            pressure: if info.mask & PRESSURE != 0 {
                Some(info.pressure as f32 / 1024.0)
            } else {
                None
            },
            tilt: if info.mask & (TILT_X | TILT_Y) == (TILT_X | TILT_Y) {
                Some(Tilt::Degrees(info.tilt.0 as f32, info.tilt.1 as f32))
            } else {
                None
            },
            twist_degrees: if info.mask & ROTATION != 0 {
                Some(info.rotation as f32)
            } else {
                None
            },
            contact: None, // POINTER_PEN_INFO does not advertise contact geometry.
            provenance: if historical {
                Provenance::Coalesced
            } else {
                Provenance::Actual
            },
        };
        if info.mask & PRESSURE != 0 && info.pressure > 1024 {
            return Err(Invalid::Pressure);
        }
        if info.mask & ROTATION != 0 && info.rotation > 359 {
            return Err(Invalid::Twist);
        }
        if (info.mask & TILT_X != 0 && !(-90..=90).contains(&info.tilt.0))
            || (info.mask & TILT_Y != 0 && !(-90..=90).contains(&info.tilt.1))
        {
            return Err(Invalid::Tilt);
        }
        if !info.position.0.is_finite() || !info.position.1.is_finite() {
            return Err(Invalid::Coordinates);
        }
        let delivered = client && !(fenced && held && !cancelled);
        let recorded_ends = old.map_or(0, |p| p.recorded_ends)
            | if delivered {
                if inverted { 2 } else { 1 }
            } else {
                0
            };
        if message == Message::Leave && !held && info.flags & IN_RANGE == 0 {
            for (index, kind) in [DeviceKind::Pen, DeviceKind::Eraser]
                .into_iter()
                .enumerate()
            {
                if recorded_ends & (1 << index) != 0 {
                    self.retired[index] = Some((
                        Identity {
                            kind,
                            ..raw.identity
                        },
                        time,
                    ));
                }
            }
        }
        self.pointers[index] = if message == Message::Leave && !held && info.flags & IN_RANGE == 0 {
            None
        } else {
            Some(Pointer {
                device: info.device,
                pointer: info.pointer,
                inverted,
                recorded_ends,
                contact,
                down_seen: phase == Phase::Down
                    || (phase != Phase::Up && old.is_some_and(|p| p.down_seen)),
                held,
                fenced: fenced && held,
                client,
                time,
            })
        };
        if !client {
            return Ok(Output::Native);
        }
        if fenced && held && !cancelled {
            return Ok(Output::Suppressed);
        }
        Ok(Output::Packet(NativePacket {
            raw,
            position: if cancelled {
                None
            } else {
                Some(WindowPosition::PhysicalPixels(
                    info.position.0,
                    info.position.1,
                ))
            },
            delivery: Delivery::ReceiverPointer,
        }))
    }
}

/// Public Windows signature identifies promoted pen/touch mouse messages.
pub(super) fn promoted(extra: usize) -> bool {
    extra & 0xffff_ff00 == 0xff51_5700
}

/// Native edge bits describe contact transitions, not arbitrary widget states.
pub(super) fn validate_contact_flags(flags: u32) -> Result<(), Invalid> {
    if flags & (DOWN | UP) == (DOWN | UP)
        || (flags & UP != 0 && flags & CONTACT != 0)
        || (flags & DOWN != 0 && flags & CONTACT == 0)
    {
        Err(Invalid::Lifecycle)
    } else {
        Ok(())
    }
}
