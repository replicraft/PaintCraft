//! One source-observed stream. No clock sorting or GUI-event matching occurs.
use crate::*;
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{AllocationClass, BudgetAccount, BudgetScope, SharedBudget},
};
use paintcraft_input::tablet::{Buttons, Identity, Phase, Provenance, Time, Transform};
use std::collections::VecDeque;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseRoute {
    Client,
    NativeChrome,
    Suppressed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FingerRoute {
    /// Validated metadata only; this does not authorize GUI/gesture delivery.
    Eligible,
    Suppressed,
    /// Retire an earlier eligible contact; the physical record is unchanged.
    PolicyCancel,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PenRoute {
    Admitted,
    Suppressed,
    /// Actual source lifecycle control, to be delivered independently of ink admission.
    Control,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Collected {
    Mouse {
        record: CaptureRecord,
        route: MouseRoute,
        cancel_pen: bool,
    },
    Tablet {
        record: CaptureRecord,
        route: PenRoute,
    },
    Finger {
        record: CaptureRecord,
        route: FingerRoute,
    },
    Boundary(CaptureRecord),
    /// Verified source lifetime end; no source clock is reclaimed by Raw Leave.
    IdentityRetired(CaptureRecord),
    /// Receipt-time barrier, deliberately without a fabricated native ordinal.
    FrameworkBoundary(Boundary),
}
#[derive(Clone, Copy, Debug)]
struct Pen {
    identity: Identity,
    held: bool,
    fenced: bool,
}
/// A drained payload retains its separately reserved logical storage allowance.
#[derive(Debug)]
pub struct CollectedFrame {
    pub records: Vec<Collected>,
    pub failed: bool,
    _storage: BudgetAccount,
}
#[derive(Debug)]
pub struct Collector {
    queue: VecDeque<Collected>,
    limit: usize,
    installation: Option<u64>,
    next: u64,
    mouse: u32,
    mouse_route: MouseRoute,
    mouse_uncertain: bool,
    mouse_clock: Option<paintcraft_input::tablet::Clock>,
    // Physical motion/button/wheel order only. AppKit tracking-area events can
    // be delivered later with older native timestamps; retain those unchanged.
    mouse_time: Option<Time>,
    pens: [Option<Pen>; 64],
    fingers: finger::Table,
    // Clock lifetime survives contact-slot release; never silently rebase a
    // reused identity after Up. Reclaim only on a verified true lifetime end.
    times: [Option<(Identity, Time)>; 64],
    owner: Option<Identity>,
    failed: bool,
    closed: bool,
    _storage: BudgetAccount,
    frames: BudgetScope,
}
impl Collector {
    /// Start at a fresh idle acquisition boundary, not halfway through a native
    /// mouse contact. The AppKit record installer checks native mouse buttons.
    pub fn new(capacity: usize, budget: &SharedBudget) -> Result<Self> {
        if !(4..=65_536).contains(&capacity) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Native collector capacity",
            ));
        }
        let bytes = capacity * std::mem::size_of::<Collected>();
        let mut storage = budget.account(AllocationClass::PaintMetadata);
        storage.resize(
            (bytes
                + (std::mem::size_of::<[Option<Pen>; 64]>()
                    + std::mem::size_of::<finger::Table>()
                    + std::mem::size_of::<[Option<(Identity, Time)>; 64]>()
                    + std::mem::size_of::<Option<paintcraft_input::tablet::Clock>>()))
                as u64,
        )?;
        let frames = budget
            .reserve_capacity(bytes as u64)?
            .scope(AllocationClass::PaintMetadata);
        let mut queue = VecDeque::new();
        queue
            .try_reserve_exact(capacity)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Native collector storage"))?;
        storage.resize(
            (queue.capacity() * std::mem::size_of::<Collected>()
                + (std::mem::size_of::<[Option<Pen>; 64]>()
                    + std::mem::size_of::<finger::Table>()
                    + std::mem::size_of::<[Option<(Identity, Time)>; 64]>()
                    + std::mem::size_of::<Option<paintcraft_input::tablet::Clock>>()))
                as u64,
        )?;
        Ok(Self {
            queue,
            limit: capacity,
            installation: None,
            next: 0,
            mouse: 0,
            mouse_route: MouseRoute::Client,
            mouse_uncertain: false,
            mouse_clock: None,
            mouse_time: None,
            pens: [None; 64],
            fingers: finger::Table::default(),
            times: [None; 64],
            owner: None,
            failed: false,
            closed: false,
            _storage: storage,
            frames,
        })
    }
    pub fn mouse_buttons(&self) -> u32 {
        self.mouse
    }
    pub fn pen_owner(&self) -> Option<Identity> {
        self.owner
    }
    pub fn failed(&self) -> bool {
        self.failed
    }
    /// Called under the installation delivery gate. Storage and callback work
    /// are bounded; this never calls egui, the editor, or a native event pump.
    pub fn accept(&mut self, record: CaptureRecord) -> std::result::Result<(), RecordFault> {
        if self.failed || self.closed {
            return Err(RecordFault::Retired);
        }
        let result = self.accept_inner(record);
        if result.is_err() {
            self.fault();
        }
        result
    }
    fn accept_inner(&mut self, record: CaptureRecord) -> std::result::Result<(), RecordFault> {
        if record.ordinal != self.next
            || record.ordinal == u64::MAX
            || self
                .installation
                .is_some_and(|epoch| epoch != record.installation)
        {
            return Err(RecordFault::Invalid);
        }
        self.installation = Some(record.installation);
        self.next = self.next.checked_add(1).ok_or(RecordFault::Invalid)?;
        if self.queue.len() == self.limit {
            return Err(RecordFault::Capacity);
        }
        let output = match record.event {
            CaptureEvent::IdentityRetired { identity, time } => {
                self.retire_identity(identity, time, record.installation)?;
                Collected::IdentityRetired(record)
            }
            CaptureEvent::Finger { .. } => {
                let route = self.collect_finger(record)?;
                Collected::Finger { record, route }
            }
            CaptureEvent::Boundary { kind, .. } => {
                self.apply_boundary(kind);
                Collected::Boundary(record)
            }
            CaptureEvent::Mouse(mouse) => {
                validate_mouse(mouse, record.installation)?;
                if self
                    .mouse_clock
                    .is_some_and(|clock| clock != mouse.time.clock)
                {
                    return Err(RecordFault::Invalid);
                }
                self.mouse_clock = Some(mouse.time.clock);
                if !matches!(mouse.kind, MouseKind::Enter | MouseKind::Leave) {
                    if !ordered_time(self.mouse_time, mouse.time) {
                        return Err(RecordFault::Invalid);
                    }
                    self.mouse_time = Some(mouse.time);
                }
                let cancel_pen =
                    matches!(mouse.kind, MouseKind::Wheel { .. }) && self.owner.is_some();
                let route = match mouse.kind {
                    MouseKind::Button { number, pressed } => {
                        let bit = 1u32
                            .checked_shl(u32::from(number))
                            .ok_or(RecordFault::Invalid)?;
                        if pressed {
                            if self.mouse == 0 {
                                self.mouse_route = if self.mouse_uncertain || self.owner.is_some() {
                                    MouseRoute::Suppressed
                                } else if mouse.client {
                                    MouseRoute::Client
                                } else {
                                    MouseRoute::NativeChrome
                                };
                            }
                            self.mouse |= bit;
                        } else {
                            self.mouse &= !bit;
                        }
                        self.mouse_route
                    }
                    MouseKind::Wheel { .. } => {
                        // Navigation is a source-time boundary. The host applies
                        // the wheel after cancelling private pen/UI state.
                        if self.owner.is_some() {
                            self.fence_pens();
                        }
                        if mouse.client {
                            MouseRoute::Client
                        } else {
                            MouseRoute::NativeChrome
                        }
                    }
                    _ if self.mouse_uncertain => MouseRoute::Suppressed,
                    _ if self.mouse != 0 => self.mouse_route,
                    _ if self.owner.is_some() => MouseRoute::Suppressed,
                    _ if mouse.client => MouseRoute::Client,
                    _ => MouseRoute::NativeChrome,
                };
                Collected::Mouse {
                    record,
                    route,
                    cancel_pen,
                }
            }
            CaptureEvent::Tablet(packet) => {
                if !packet.raw.identity.kind.is_pen()
                    || packet.raw.sequence != record.ordinal
                    || packet.raw.time.epoch != record.installation
                    || packet.raw.provenance == Provenance::Predicted
                    || (packet.raw.provenance == Provenance::Boundary
                        && !matches!(packet.raw.phase, Phase::Cancel | Phase::Leave))
                {
                    return Err(RecordFault::Invalid);
                }
                let lifecycle = matches!(
                    packet.raw.phase,
                    Phase::Cancel | Phase::Leave | Phase::Enter
                );
                if !lifecycle || packet.position.is_some() {
                    let raw = packet.into_logical(1.0).map_err(|_| RecordFault::Invalid)?;
                    raw.at(Transform {
                        owner: 0,
                        epoch: 0,
                        origin: (0.0, 0.0),
                        points_per_pixel: 1.0,
                    })
                    .map_err(|_| RecordFault::Invalid)?;
                }
                let index = self
                    .pens
                    .iter()
                    .position(|p| p.is_some_and(|p| p.identity == packet.raw.identity))
                    .or_else(|| self.pens.iter().position(Option::is_none))
                    .ok_or(RecordFault::Capacity)?;
                let mut pen = self.pens[index].unwrap_or(Pen {
                    identity: packet.raw.identity,
                    held: false,
                    fenced: false,
                });
                let clock_slot = self
                    .times
                    .iter()
                    .position(|entry| entry.is_some_and(|(id, _)| id == packet.raw.identity))
                    .or_else(|| self.times.iter().position(Option::is_none))
                    .ok_or(RecordFault::Capacity)?;
                if !ordered_time(
                    self.times[clock_slot].map(|(_, time)| time),
                    packet.raw.time,
                ) {
                    return Err(RecordFault::Invalid);
                }
                self.times[clock_slot] = Some((packet.raw.identity, packet.raw.time));
                let terminal = packet.raw.buttons.0
                    & (Buttons::TIP | Buttons::BARREL | Buttons::BARREL_UPPER)
                    == 0
                    && matches!(packet.raw.phase, Phase::Up | Phase::Hover | Phase::Leave);
                let held = packet.raw.phase == Phase::Down
                    || (!terminal && pen.held)
                    || packet.raw.buttons.0
                        & (Buttons::TIP | Buttons::BARREL | Buttons::BARREL_UPPER)
                        != 0;
                let cancelled = matches!(packet.raw.phase, Phase::Cancel | Phase::Leave);
                if cancelled {
                    pen.fenced |= pen.held;
                    if self.owner == Some(pen.identity) {
                        self.owner = None;
                    }
                }
                let competing_mouse = self.mouse_uncertain
                    || (self.mouse != 0 && self.mouse_route != MouseRoute::Suppressed);
                if held
                    && (competing_mouse || self.owner.is_some_and(|owner| owner != pen.identity))
                {
                    pen.fenced = true;
                }
                let route = if cancelled {
                    PenRoute::Control
                } else if pen.fenced {
                    PenRoute::Suppressed
                } else {
                    PenRoute::Admitted
                };
                if route == PenRoute::Admitted && held {
                    self.owner = Some(pen.identity);
                }
                pen.held = held;
                if terminal {
                    pen.held = false;
                    pen.fenced = false;
                    if self.owner == Some(pen.identity) {
                        self.owner = None;
                    }
                }
                self.pens[index] = if pen.held || pen.fenced {
                    Some(pen)
                } else {
                    None
                };
                Collected::Tablet { record, route }
            }
        };
        self.queue.push_back(output);
        Ok(())
    }
    fn apply_boundary(&mut self, kind: Boundary) {
        match kind {
            Boundary::PointerIdle => {
                // An observed idle state retires the old physical hold without
                // inventing a terminal edge. Held pen tails stay fenced.
                self.mouse = 0;
                self.mouse_route = MouseRoute::Client;
                self.mouse_uncertain = false;
            }
            Boundary::FocusLost | Boundary::WindowClosed => {
                self.closed |= kind == Boundary::WindowClosed;
                // Only a locally observed hold can have a missing outside Up.
                // Unrelated mouse activity elsewhere does not create ownership.
                self.mouse_uncertain |= self.mouse != 0;
                self.fence_pens();
            }
            _ => self.fence_pens(),
        }
    }
    /// Keyboard/IME callbacks without native provenance are explicit barriers,
    /// never joined by comparing clocks or guessing the last NSEvent identity.
    pub fn framework_boundary(&mut self, kind: Boundary) -> std::result::Result<(), RecordFault> {
        if kind == Boundary::PointerIdle {
            self.fault();
            return Err(RecordFault::Invalid);
        }
        if self.failed || self.closed {
            return Err(RecordFault::Retired);
        }
        if self.queue.len() == self.limit {
            self.fault();
            return Err(RecordFault::Capacity);
        }
        self.apply_boundary(kind);
        self.queue.push_back(Collected::FrameworkBoundary(kind));
        Ok(())
    }
    fn fence_pens(&mut self) {
        self.fingers.fence();
        for pen in self.pens.iter_mut().flatten() {
            pen.fenced |= pen.held;
        }
        self.owner = None;
    }
    pub fn fault(&mut self) {
        self.failed = true;
        self.queue.clear();
        self.fence_pens();
    }
    /// Snapshot/discard only under with_delivery. Apply GUI/tool work after it.
    pub fn take(&mut self, health: DeliveryHealth) -> Result<CollectedFrame> {
        if health == DeliveryHealth::Failed {
            self.fault();
        }
        let mut storage = self.frames.account();
        storage.resize((self.queue.len() * std::mem::size_of::<Collected>()) as u64)?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(self.queue.len())
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Native collector handoff"))?;
        // Allocators may provide more capacity than requested. Account the
        // owned handoff's actual capacity before draining any admitted prefix.
        storage.resize((records.capacity() * std::mem::size_of::<Collected>()) as u64)?;
        records.extend(self.queue.drain(..));
        Ok(CollectedFrame {
            records,
            failed: self.failed,
            _storage: storage,
        })
    }
}
fn ordered_time(previous: Option<Time>, current: Time) -> bool {
    previous.is_none_or(|previous| {
        previous.clock == current.clock
            && previous.epoch == current.epoch
            && previous.micros <= current.micros
    })
}
fn validate_mouse(record: MouseRecord, epoch: u64) -> std::result::Result<(), RecordFault> {
    if record.time.epoch != epoch {
        return Err(RecordFault::Invalid);
    }
    let valid = match record.position {
        WindowPosition::PhysicalPixels(x, y) => x.is_finite() && y.is_finite(),
        WindowPosition::SurfacePoints {
            x,
            y,
            physical_scale,
        } => x.is_finite() && y.is_finite() && physical_scale.is_finite() && physical_scale > 0.0,
    };
    if !valid {
        return Err(RecordFault::Invalid);
    }
    if let MouseKind::Wheel {
        delta,
        precise,
        physical_scale,
        ..
    } = record.kind
        && (!delta.0.is_finite()
            || !delta.1.is_finite()
            || (precise && (!physical_scale.is_finite() || physical_scale <= 0.0)))
    {
        return Err(RecordFault::Invalid);
    }
    Ok(())
}
#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "collector/contracts.rs"]
mod contracts;

mod lifecycle;

#[cfg(test)]
#[path = "collector/tracking.rs"]
mod tracking;

mod finger;
