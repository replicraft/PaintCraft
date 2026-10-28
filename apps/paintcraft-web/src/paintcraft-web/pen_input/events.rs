//! Pointer Events contact semantics. A chorded tip transition may be a Move,
//! while barrel-only Down/Up is hovering and must never create ink.
use paintcraft_input::{Modifiers, tablet::*};

/// Same metadata-first handoff for DOM and headless handler controls. Fenced
/// physical tails still reach the fixed metadata store so a real Up can retire
/// their identity, while neither coordinates nor UI/ink are delivered.
#[cfg(test)]
pub(super) fn deliver(
    source: &paintcraft_ui::pen_input::Source,
    raw: RawSample,
    fenced: bool,
    convert: impl FnOnce(f64) -> Result<Option<(f64, f64)>, Invalid>,
) -> Result<receiver::Route, Invalid> {
    if fenced {
        source.suppress_native(raw)
    } else {
        source.submit_native(receiver::Delivery::ReceiverPointer, raw, convert)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Disposition {
    Deliver,
    Orphan,
    Suppressed,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Packet {
    pub raw: RawSample,
    pub disposition: Disposition,
    pub edge: Option<PenButtonEdge>,
}
pub(super) fn deliver_packet(
    source: &paintcraft_ui::pen_input::Source,
    packet: Packet,
    convert: impl FnOnce(f64) -> Result<Option<(f64, f64)>, Invalid>,
) -> Result<receiver::Route, Invalid> {
    if packet.disposition == Disposition::Suppressed {
        source.suppress_native(packet.raw)
    } else {
        source.submit_native_with_button_edge(
            receiver::Delivery::ReceiverPointer,
            packet.raw,
            packet.edge,
            convert,
        )
    }
}
pub(super) fn deliver_locked_packet(
    source: &paintcraft_ui::pen_input::Source,
    packet: Packet,
    locked: bool,
    convert: impl FnOnce(f64) -> Result<Option<(f64, f64)>, Invalid>,
) -> Result<receiver::Route, Invalid> {
    match deliver_packet(source, packet, convert) {
        Err(error) if locked && error != Invalid::Capacity => Ok(receiver::Route::Suppressed),
        result => result,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Down,
    Move,
    Up,
    Leave,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Pointer {
    pub id: u64,
    pub kind: Kind,
    /// Production intake defers geometry until Source has observed metadata.
    pub at: Option<(f64, f64)>,
    pub stamp: f64,
    pub buttons: u16,
    /// Actual DOM changed-button fact (-1 means no edge), not held bits.
    pub button: i16,
    pub pressure: f32,
    pub tilt: (f32, f32),
    pub twist: f32,
    pub contact: (f32, f32),
    pub modifiers: Modifiers,
}
#[derive(Clone, Copy, Debug)]
struct Tool {
    id: u64,
    tip: bool,
    held: bool,
    inverted: bool,
    fenced: bool,
    orphan: bool,
    actual: bool,
    actual_buttons: u16,
}
#[derive(Clone, Copy)]
enum Slot {
    Active(usize),
    Denied(usize),
}
#[derive(Debug)]
pub(super) struct Normalizer {
    tools: [Option<Tool>; 8],
    denied: [Option<Tool>; 8],
    restart_required: bool,
    sequence: u64,
    epoch: u64,
    cancellation_epoch: Option<u64>,
    _storage: Option<paintcraft_core::types::resources::BudgetAccount>,
}
impl Normalizer {
    pub fn new(epoch: u64) -> Self {
        Self {
            tools: [None; 8],
            denied: [None; 8],
            restart_required: false,
            sequence: 0,
            epoch,
            cancellation_epoch: None,
            _storage: None,
        }
    }
    /// The account travels with the listener-shared value, not its Guard: abort
    /// may remove registrations before the runner drops its closure clones.
    pub fn admitted(
        epoch: u64,
        budget: &paintcraft_core::types::resources::SharedBudget,
    ) -> paintcraft_core::types::Result<Self> {
        use paintcraft_core::types::resources::AllocationClass;
        let header = (2 * std::mem::size_of::<usize>())
            .next_multiple_of(std::mem::align_of::<std::cell::RefCell<Self>>());
        let mut storage = budget.account(AllocationClass::PaintMetadata);
        storage.resize((std::mem::size_of::<std::cell::RefCell<Self>>() + header) as u64)?;
        let mut value = Self::new(epoch);
        value._storage = Some(storage);
        Ok(value)
    }
    pub fn restart_required(&self) -> bool {
        self.restart_required
    }
    /// Once storage is exhausted, unknown tails and pen-local parse failures
    /// are ignored, not repeatedly escalated into global mouse cancellation.
    /// Known valid records still carry their own real terminal to Source.
    pub fn intake(
        &mut self,
        event: Pointer,
        provenance: Provenance,
    ) -> Result<Option<Packet>, Invalid> {
        let locked = self.restart_required;
        let known = self
            .tools
            .iter()
            .chain(self.denied.iter())
            .flatten()
            .any(|tool| tool.id == event.id);
        if locked && !known {
            return Ok(None);
        }
        match self.normalize(event, provenance) {
            Ok(raw) => Ok(Some(raw)),
            Err(_) if locked => Ok(None),
            Err(error) => Err(error),
        }
    }
    fn locate(&mut self, id: u64) -> Result<Slot, Invalid> {
        if let Some(i) = self
            .tools
            .iter()
            .position(|tool| tool.is_some_and(|tool| tool.id == id))
        {
            return Ok(Slot::Active(i));
        }
        if let Some(i) = self
            .denied
            .iter()
            .position(|tool| tool.is_some_and(|tool| tool.id == id))
        {
            return Ok(Slot::Denied(i));
        }
        if self.restart_required {
            return Err(Invalid::Capacity);
        }
        if let Some(i) = self
            .tools
            .iter()
            .position(Option::is_none)
            .or_else(|| self.tools.iter().position(|t| t.is_some_and(|t| !t.held)))
        {
            return Ok(Slot::Active(i));
        }
        if let Some(i) = self.denied.iter().position(Option::is_none) {
            return Ok(Slot::Denied(i));
        }
        // We cannot retain another denied identity. No unrelated terminal, host
        // cancellation, or freed slot may reopen an untracked physical tail.
        self.restart_required = true;
        self.clear();
        Err(Invalid::Capacity)
    }
    fn tool(&self, slot: Slot) -> Option<Tool> {
        match slot {
            Slot::Active(i) => self.tools[i],
            Slot::Denied(i) => self.denied[i],
        }
    }
    fn store(&mut self, slot: Slot, tool: Option<Tool>) {
        match slot {
            Slot::Active(i) => self.tools[i] = tool,
            Slot::Denied(i) => self.denied[i] = tool,
        }
    }
    /// The host may retire after this tool left the source queue. Observe its
    /// token before normalizing the next physical event, not just DOM cancels.
    pub fn observe_cancellation(&mut self, epoch: u64) {
        if self
            .cancellation_epoch
            .replace(epoch)
            .is_some_and(|old| old != epoch)
        {
            self.clear();
        }
    }
    pub fn deny_contact(&mut self, id: u64) {
        if let Some(tool) = self
            .tools
            .iter_mut()
            .chain(self.denied.iter_mut())
            .flatten()
            .find(|tool| tool.id == id && tool.held)
        {
            tool.fenced = true;
            tool.orphan = false;
        }
    }
    pub fn clear(&mut self) {
        for tool in self.tools.iter_mut().chain(self.denied.iter_mut()) {
            if let Some(held) = tool.as_mut().filter(|tool| tool.held) {
                held.fenced = true;
                held.orphan = false;
            } else {
                *tool = None;
            }
        }
    }
    /// Physical tails survive loss of GUI capture/focus. A zero-button event is
    /// terminal evidence; it is suppressed too, then a fresh contact may begin.
    pub fn fenced(&mut self, event: Pointer) -> bool {
        if let Some(slot) = self
            .tools
            .iter_mut()
            .chain(self.denied.iter_mut())
            .find(|slot| slot.is_some_and(|tool| tool.id == event.id && tool.fenced))
        {
            if event.buttons == 0 {
                *slot = None;
            }
            return true;
        }
        false
    }
    pub fn starting(&self, id: u64, buttons: u16) -> bool {
        buttons & (Buttons::TIP | Buttons::ERASER) != 0
            && !self
                .tools
                .iter()
                .chain(self.denied.iter())
                .flatten()
                .any(|tool| tool.id == id && tool.tip)
    }
    #[cfg(test)]
    pub fn sample(&mut self, event: Pointer, provenance: Provenance) -> Result<RawSample, Invalid> {
        self.normalize(event, provenance).map(|packet| packet.raw)
    }
    #[cfg(test)]
    pub fn ingest(
        &mut self,
        event: Pointer,
        provenance: Provenance,
    ) -> Result<(RawSample, bool), Invalid> {
        self.normalize(event, provenance)
            .map(|packet| (packet.raw, packet.disposition != Disposition::Deliver))
    }
    fn normalize(&mut self, event: Pointer, provenance: Provenance) -> Result<Packet, Invalid> {
        if provenance == Provenance::Predicted {
            return Err(Invalid::Lifecycle);
        }
        if event.at.is_some() && self.fenced(event) {
            return Err(Invalid::Lifecycle);
        }
        if !event.stamp.is_finite() || event.stamp < 0.0 || event.stamp >= u64::MAX as f64 / 1000.0
        {
            return Err(Invalid::Clock);
        }
        let slot = self.locate(event.id)?;
        let old = self.tool(slot).filter(|tool| tool.id == event.id);
        let tip = event.buttons & (Buttons::TIP | Buttons::ERASER) != 0;
        let held = event.buttons != 0;
        let actual = provenance == Provenance::Actual;
        let side_press = actual
            && matches!(event.button, 1 | 2)
            && event.buttons
                & if event.button == 2 {
                    Buttons::BARREL
                } else {
                    Buttons::BARREL_UPPER
                }
                != 0;
        let contact_press =
            actual && tip && (event.kind == Kind::Down || matches!(event.button, 0 | 5));
        let observed_rise = actual
            && old.is_some_and(|tool| {
                tool.actual
                    && (event.buttons
                        & !tool.actual_buttons
                        & (Buttons::BARREL | Buttons::BARREL_UPPER))
                        != 0
            });
        let recover = side_press || contact_press || observed_rise;
        let history_rise = !actual && held && old.is_none_or(|tool| tool.actual_buttons == 0);
        let orphan = ((old.is_none_or(|tool| !tool.actual) || history_rise) && held && !recover)
            || old.is_some_and(|tool| tool.orphan && !recover);
        let strong = self.restart_required
            || matches!(slot, Slot::Denied(_))
            || old.is_some_and(|tool| tool.fenced && !tool.orphan);
        let suppressed = strong || orphan;
        let inverted = old
            .filter(|t| t.held)
            .map_or(event.buttons & Buttons::ERASER != 0, |t| t.inverted);
        // Changing ends while physically held is not a new valid stroke.
        if old
            .is_some_and(|t| t.held && tip && t.inverted != (event.buttons & Buttons::ERASER != 0))
        {
            return Err(Invalid::Lifecycle);
        }
        let phase = match event.kind {
            _ if !actual => {
                if old.is_some_and(|tool| tool.tip) {
                    Phase::Move
                } else {
                    Phase::Hover
                }
            }
            _ if orphan && tip => Phase::Move,
            Kind::Leave if !old.is_some_and(|t| t.tip) => Phase::Leave,
            _ if tip && !old.is_some_and(|t| t.tip) => Phase::Down,
            _ if !tip && old.is_some_and(|t| t.tip) => Phase::Up,
            _ if tip => Phase::Move,
            _ => Phase::Hover,
        };
        let sequence = self
            .sequence
            .checked_add(1)
            .filter(|v| *v != u64::MAX)
            .ok_or(Invalid::Order)?;
        let raw = RawSample {
            identity: Identity {
                device: 0,
                pointer: event.id,
                kind: if inverted {
                    DeviceKind::Eraser
                } else {
                    DeviceKind::Pen
                },
            },
            sequence,
            time: Time {
                epoch: self.epoch,
                micros: (event.stamp * 1000.0).floor() as u64,
                clock: Clock::BrowserPerformance,
            },
            phase,
            logical_position: event.at,
            buttons: Buttons(event.buttons),
            modifiers: event.modifiers,
            pressure: Some(event.pressure),
            tilt: Some(Tilt::Degrees(event.tilt.0, event.tilt.1)),
            twist_degrees: Some(event.twist),
            contact: Some(event.contact),
            provenance,
        };
        let edge = if actual {
            let button = match event.button {
                2 => Some(PenButton::Barrel),
                1 => Some(PenButton::UpperBarrel),
                _ => None,
            };
            button.map(|button| PenButtonEdge {
                identity: raw.identity,
                button,
                pressed: event.buttons & button.mask() != 0,
            })
        } else {
            None
        };
        if raw.logical_position.is_some() {
            raw.at(Transform {
                owner: 0,
                epoch: 0,
                origin: (0.0, 0.0),
                points_per_pixel: 1.0,
            })?;
        }
        self.sequence = sequence;
        let retained_held = if actual {
            held
        } else {
            held || old.is_some_and(|tool| tool.held)
        };
        self.store(
            slot,
            if actual
                && ((!held && matches!(phase, Phase::Cancel | Phase::Leave))
                    || (suppressed && !held))
            {
                None
            } else {
                Some(Tool {
                    id: event.id,
                    tip: if actual {
                        tip
                    } else {
                        old.is_some_and(|tool| tool.tip)
                    },
                    held: retained_held,
                    inverted,
                    fenced: suppressed,
                    orphan: orphan && !strong,
                    actual: actual || old.is_some_and(|tool| tool.actual),
                    actual_buttons: if actual {
                        event.buttons
                    } else {
                        old.map_or(0, |tool| tool.actual_buttons)
                    },
                })
            },
        );
        if event.at.is_some() && (orphan || matches!(slot, Slot::Denied(_))) {
            return Err(if orphan {
                Invalid::Lifecycle
            } else {
                Invalid::Capacity
            });
        }
        Ok(Packet {
            raw,
            edge,
            disposition: if strong {
                Disposition::Suppressed
            } else if orphan {
                Disposition::Orphan
            } else {
                Disposition::Deliver
            },
        })
    }
}

/// Pair only actual compatibility events, never coordinates or arbitrary mouse
/// movement. A genuine mouse PointerEvent clears the candidate before its own
/// mapped mouse event. The timestamp identity is separately browser-qualified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Pair {
    pub kind: Kind,
    pub stamp: u64,
    pub buttons: u16,
    pub trusted: bool,
}
#[derive(Debug, Default)]
pub(super) struct Compatibility(Option<Pair>);
impl Compatibility {
    pub fn pen(&mut self, pair: Pair) {
        self.0 = Some(pair);
    }
    pub fn mouse(&mut self) {
        self.0 = None;
    }
    pub fn matches(&mut self, pair: Pair) -> bool {
        let matched = self.0 == Some(pair);
        self.0 = None;
        matched
    }
}

#[cfg(test)]
mod tests;
