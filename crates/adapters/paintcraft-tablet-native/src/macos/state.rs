#![deny(unsafe_code)]
//! Source-only tablet state. No user callback is invoked while its lock is held.
use crate::*;
use paintcraft_input::{
    Modifiers,
    tablet::{Buttons, Clock, DeviceKind, Identity, Phase, Provenance, RawSample, Tilt, Time},
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Point {
    pub device: u64,
    pub in_view: bool,
    pub micros: u64,
    pub position: WindowPosition,
    pub buttons: u16,
    pub modifiers: Modifiers,
    pub pressure: f32,
    pub tilt: (f32, f32),
    pub rotation: f32,
}
#[derive(Debug)]
struct Tool {
    identity: Identity,
    caps: u64,
    sequence: u64,
    contact: bool,
    fenced: bool,
    route: Option<bool>,
    last_consumed: bool,
    last_point: Option<Point>,
    last_raw: Option<RawSample>,
}
#[derive(Debug)]
pub(super) enum Action {
    Native,
    Suppress,
    Unclassified { consume: bool },
    Dispatch(NativePacket),
}
#[derive(Clone, Copy, Debug)]
struct Unidentified {
    device: u64,
    buttons: u16,
    last_point: Option<Point>,
    client: bool,
}
#[derive(Debug)]
pub(super) struct State {
    epoch: u64,
    tools: Vec<Tool>,
    unidentified: Vec<Unidentified>,
    unidentified_overflow: bool,
}
impl State {
    pub(super) fn new(epoch: u64) -> Result<Self> {
        let mut tools = Vec::new();
        tools
            .try_reserve_exact(64)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Tablet device capacity"))?;
        let mut unidentified = Vec::new();
        unidentified
            .try_reserve_exact(64)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Unidentified tablet capacity"))?;
        Ok(Self {
            epoch,
            tools,
            unidentified,
            unidentified_overflow: false,
        })
    }
    pub(super) fn proximity(
        &mut self,
        device: u64,
        pointer: u64,
        kind: DeviceKind,
        caps: u64,
    ) -> (bool, Option<NativePacket>) {
        let cancel = self.leave(device);
        if self.tools.len() >= 64 {
            return (false, cancel);
        }
        let previous = self
            .unidentified
            .iter()
            .position(|u| u.device == device)
            .map(|index| self.unidentified.remove(index));
        let held = previous.is_some_and(|u| u.buttons & 7 != 0);
        self.tools.push(Tool {
            identity: Identity {
                device,
                pointer,
                kind,
            },
            caps,
            sequence: 0,
            contact: false,
            fenced: (held && previous.is_some_and(|u| u.client))
                || (previous.is_none() && self.unidentified_overflow),
            route: previous.filter(|_| held).map(|u| u.client),
            last_consumed: previous.is_some_and(|u| u.client),
            last_point: previous.and_then(|u| u.last_point),
            last_raw: None,
        });
        (true, cancel)
    }
    pub(super) fn prepare(&mut self, point: Point) -> Action {
        let Some(tool) = self
            .tools
            .iter_mut()
            .find(|tool| tool.identity.device == point.device)
        else {
            return self.unidentified(point);
        };
        if tool.last_point.is_some_and(|last| {
            let mut source = point;
            // Scope/focus changes do not turn one copied hardware record into
            // a second native mouse-up or a different physical event.
            source.in_view = last.in_view;
            source == last
        }) {
            return if tool.last_consumed {
                Action::Suppress
            } else {
                Action::Native
            };
        }
        if tool.fenced {
            tool.last_point = Some(point);
            tool.last_consumed = true;
            if point.buttons & 7 == 0 {
                tool.fenced = false;
                tool.contact = false;
                tool.route = None;
            }
            return Action::Suppress;
        }
        let held = point.buttons & 7 != 0;
        let in_view = tool.route.unwrap_or(point.in_view);
        if held && tool.route.is_none() {
            tool.route = Some(in_view);
        }
        tool.last_point = Some(point);
        tool.last_consumed = in_view;
        if !in_view {
            tool.contact = point.buttons & Buttons::TIP != 0;
            tool.last_raw = None;
            if !held {
                tool.route = None;
                tool.contact = false;
            }
            return Action::Native;
        }
        let Some(sequence) = tool.sequence.checked_add(1) else {
            return Action::Suppress;
        };
        let contact = point.buttons & Buttons::TIP != 0;
        let phase = match (tool.contact, contact) {
            (false, true) => Phase::Down,
            (true, false) => Phase::Up,
            (true, true) => Phase::Move,
            _ => Phase::Hover,
        };
        // Public IOLLEvent.h bits determine optionality. Reported zero force is
        // retained, never replaced with a mouse pressure default.
        let pressure = (tool.caps & 0x400 != 0).then_some(point.pressure);
        let tilt =
            (tool.caps & 0x180 == 0x180).then_some(Tilt::Normalized(point.tilt.0, point.tilt.1));
        let twist = (tool.caps & 0x2000 != 0).then_some(point.rotation.rem_euclid(360.0));
        let buttons = point.buttons
            | if tool.identity.kind == DeviceKind::Eraser {
                Buttons::ERASER
            } else {
                0
            };
        let raw = RawSample {
            identity: tool.identity,
            sequence,
            time: Time {
                epoch: self.epoch,
                micros: point.micros,
                clock: Clock::AppKitUptime,
            },
            phase,
            logical_position: None,
            buttons: Buttons(buttons),
            modifiers: point.modifiers,
            pressure,
            tilt,
            twist_degrees: twist,
            contact: None,
            provenance: Provenance::Actual,
        };
        tool.sequence = sequence;
        tool.last_point = Some(point);
        tool.last_raw = Some(raw);
        tool.contact = contact;
        if !held {
            tool.route = None;
        }
        Action::Dispatch(NativePacket {
            raw,
            position: Some(point.position),
            delivery: Delivery::ReceiverPointer,
        })
    }
    // No RawSample can be constructed without genuine proximity identity.
    // Preserve the initial chrome/client route, including a copied release.
    fn unidentified(&mut self, point: Point) -> Action {
        self.unidentified_route(point.device, point.buttons, point.in_view, Some(point))
    }
    fn unidentified_route(
        &mut self,
        device: u64,
        buttons: u16,
        in_view: bool,
        point: Option<Point>,
    ) -> Action {
        let previous = self.unidentified.iter().position(|u| u.device == device);
        let client = if let Some(index) = previous {
            let old = self.unidentified[index];
            let same_source = old.last_point.zip(point).is_some_and(|(last, next)| {
                Point {
                    in_view: last.in_view,
                    ..next
                } == last
            });
            let client = if old.buttons & 7 != 0 || same_source {
                old.client
            } else {
                in_view
            };
            self.unidentified[index] = Unidentified {
                device,
                buttons,
                last_point: point,
                client,
            };
            client
        } else if self.unidentified.len() < 64 && !self.unidentified_overflow {
            self.unidentified.push(Unidentified {
                device,
                buttons,
                last_point: point,
                client: in_view,
            });
            in_view
        } else {
            // Cannot safely remember another physical tail. Fail closed for
            // unidentified tablet records until reinstall, never ordinary mice.
            self.unidentified_overflow = true;
            true
        };
        Action::Unclassified { consume: client }
    }
    pub(super) fn quarantine(
        &mut self,
        device: u64,
        buttons_held: bool,
        in_view: bool,
    ) -> (bool, Option<NativePacket>) {
        let Some(index) = self
            .tools
            .iter()
            .position(|tool| tool.identity.device == device)
        else {
            let action = self.unidentified_route(device, u16::from(buttons_held), in_view, None);
            // This is native fencing state only, never an emitted position.
            return (
                matches!(action, Action::Unclassified { consume: true }),
                None,
            );
        };
        let client = self.tools[index].route.unwrap_or(in_view);
        if !client {
            return (false, None);
        }
        let packet = self.cancel_one(index);
        let tool = &mut self.tools[index];
        tool.contact = buttons_held;
        tool.fenced = buttons_held;
        tool.route = buttons_held.then_some(true);
        (true, packet)
    }
    pub(super) fn leave(&mut self, device: u64) -> Option<NativePacket> {
        let index = self
            .tools
            .iter()
            .position(|tool| tool.identity.device == device)?;
        let tool = &self.tools[index];
        let buttons = tool
            .last_point
            .map_or(u16::from(tool.fenced || tool.contact), |p| p.buttons);
        let client = tool.route.unwrap_or(true);
        let last_point = tool.last_point;
        let packet = self.cancel_one(index);
        if buttons & 7 != 0 {
            self.unidentified_route(device, buttons, client, last_point);
        }
        self.tools.remove(index);
        packet
    }
    fn cancel_one(&mut self, index: usize) -> Option<NativePacket> {
        let tool = &mut self.tools[index];
        // An OS titlebar/button gesture never becomes our captured contact.
        if tool.route == Some(false) {
            return None;
        }
        let buttons_held = tool.last_point.is_some_and(|point| point.buttons & 7 != 0);
        let packet = tool
            .last_raw
            .take()
            .filter(|_| tool.contact || buttons_held)
            .map(|mut raw| {
                raw.phase = Phase::Cancel;
                raw.provenance = Provenance::Boundary;
                raw.sequence = raw.sequence.saturating_add(1);
                raw.buttons = Buttons(0);
                raw.pressure = None;
                raw.tilt = None;
                raw.twist_degrees = None;
                raw.contact = None;
                NativePacket {
                    raw,
                    position: None,
                    delivery: Delivery::ReceiverPointer,
                }
            });
        tool.fenced |= tool.contact || buttons_held;
        // Consume the cancelled physical tail until actual tip-up. Focus loss
        // must not turn the next still-down motion into a second Down.
        tool.last_point = None;
        packet
    }
    pub(super) fn cancel(&mut self) -> [Option<NativePacket>; 64] {
        std::array::from_fn(|index| {
            if index < self.tools.len() {
                self.cancel_one(index)
            } else {
                None
            }
        })
    }
}
