//! Acquisition-time pointer projection. Framework pointer promotion is replaced,
//! never matched by position or joined by callback receipt order.
use eframe::egui;
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{AllocationClass, BudgetAccount, BudgetScope, SharedBudget},
};
use paintcraft_input::tablet::receiver::{Delivery, Route, UiEvent};
use paintcraft_platform::tablet::{
    Boundary, CaptureEvent, CaptureRecord, Collected, Collector, DeliveryHealth, MouseKind,
    MouseRoute, PenRoute, RecordFault, WheelPhase, WindowPosition,
};
use paintcraft_ui::pen_input::{Batch, Owner, Source, View};

// Keep the resource-owning Batch inline: a per-record Box would add callback
// allocations. Every queue/frame slot is charged using this actual enum size.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Pointer {
    Gui(egui::Event),
    /// Carries its opaque source binding and admitted storage through replay.
    /// The consumer checks `is_current` immediately before each tool/UI use.
    Pen(Batch),
    Cancel {
        reset_gui: bool,
    },
}
#[derive(Debug)]
pub struct Tagged {
    pub ordinal: Option<u64>,
    pub pointer: Pointer,
    epoch: u64,
}
#[derive(Debug)]
pub struct Frame {
    pub entries: Vec<Tagged>,
    source: Source,
    _storage: BudgetAccount,
}
impl Frame {
    pub fn resets_gui_pointer(&self) -> bool {
        self.entries
            .iter()
            .any(|e| matches!(e.pointer, Pointer::Cancel { reset_gui: true }))
    }
    pub fn pen_is_current(&self, batch: &Batch, owner: Owner) -> bool {
        self.source.batch_is_current(batch, owner)
    }
}
#[derive(Debug)]
pub struct Projection {
    collector: Collector,
    source: Source,
    view: Option<View>,
    epoch: u64,
    mouse: u32,
    limit: usize,
    queue: Vec<Tagged>,
    _storage: BudgetAccount,
    frames: BudgetScope,
    failed: bool,
    span_first: Option<u64>,
    handoff: u64,
    source_epoch: Option<u64>,
}
impl Projection {
    pub fn new(limit: usize, budget: &SharedBudget) -> Result<Self> {
        if !(4..=4096).contains(&limit) {
            return Err(resource("Pointer projection capacity"));
        }
        let collector = Collector::new(limit, budget)?;
        let source = Source::new(limit + 1, 3 * limit, budget)?;
        let mut storage = budget.account(AllocationClass::PaintMetadata);
        storage.resize((limit * std::mem::size_of::<Tagged>()) as u64)?;
        let mut queue = Vec::new();
        queue
            .try_reserve_exact(limit)
            .map_err(|_| resource("Pointer projection reserve"))?;
        storage.resize((queue.capacity() * std::mem::size_of::<Tagged>()) as u64)?;
        let frames = budget
            .reserve_capacity((limit * frame_slot_bytes()) as u64)?
            .scope(AllocationClass::PaintMetadata);
        Ok(Self {
            collector,
            source,
            view: None,
            epoch: 0,
            mouse: 0,
            limit,
            queue,
            _storage: storage,
            frames,
            failed: false,
            span_first: None,
            handoff: 0,
            source_epoch: None,
        })
    }
    /// Called after final layout, outside any GUI/native delivery lock. The same
    /// projection lock serializes publication and actual acquisition callbacks.
    pub fn publish(&mut self, view: View) -> Result<()> {
        if self.view.is_some_and(|v| {
            v.owner.session != view.owner.session
                || v.owner.interface != view.owner.interface
                || v.transform != view.transform
                || v.canvas != view.canvas
                || v.pixels_per_point != view.pixels_per_point
                || v.focused != view.focused
                || v.blocked != view.blocked
                || v.ime != view.ime
        }) {
            self.epoch = self
                .epoch
                .checked_add(1)
                .ok_or_else(|| resource("Projection epoch"))?;
        }
        self.source
            .publish(view)
            .map_err(|_| resource("Invalid pointer view"))?;
        self.view = Some(view);
        Ok(())
    }
    pub fn capture(&mut self, record: CaptureRecord) -> std::result::Result<(), RecordFault> {
        if self.failed {
            return Err(RecordFault::Retired);
        }
        let result = self.capture_inner(record);
        if result.is_err() {
            self.fault();
        }
        result
    }
    fn capture_inner(&mut self, record: CaptureRecord) -> std::result::Result<(), RecordFault> {
        self.collector.accept(record)?;
        self.source_epoch = Some(record.installation);
        // Invoked inside the native emitter gate. A later failed emission makes
        // the entire queued projection inaccessible at take(DeliveryHealth::Failed).
        let frame = self
            .collector
            .take(DeliveryHealth::Ready)
            .map_err(|_| RecordFault::Capacity)?;
        for collected in &frame.records {
            match *collected {
                Collected::Tablet { record, route } => {
                    let CaptureEvent::Tablet(packet) = record.event else {
                        return Err(RecordFault::Invalid);
                    };
                    if route == PenRoute::Suppressed {
                        self.source
                            .suppress_native(packet.raw)
                            .map_err(|_| RecordFault::Invalid)?;
                        continue;
                    }
                    let route = self
                        .source
                        .submit_native_with_button_edge(
                            Delivery::ReceiverPointer,
                            packet.raw,
                            packet.button_edge,
                            |scale| packet.into_logical(scale).map(|raw| raw.logical_position),
                        )
                        .map_err(|_| RecordFault::Invalid)?;
                    self.span_first.get_or_insert(record.ordinal);
                    // Keep a canvas contact together until its real terminal or
                    // the next GUI handoff. A later receipt can then authorize
                    // the captured Down together with its queued Move/Up. UI
                    // contacts still deliver each edge immediately for widgets.
                    if route != Route::Canvas
                        || packet.raw.phase == paintcraft_input::tablet::Phase::Up
                    {
                        self.flush_pen()?;
                    }
                }
                Collected::Mouse {
                    record,
                    route,
                    cancel_pen,
                } => {
                    let CaptureEvent::Mouse(mouse) = record.event else {
                        return Err(RecordFault::Invalid);
                    };
                    if route != MouseRoute::Suppressed
                        && let MouseKind::Button { number, pressed } = mouse.kind
                    {
                        if pressed {
                            self.mouse |= 1 << number;
                        } else {
                            self.mouse &= !(1 << number);
                        }
                        // Only admitted physical holds own the receiver. The
                        // collector retains rejected tails independently; a
                        // later chord in one cannot acquire mouse authority.
                        // Native chrome still retains its original ownership.
                        self.source.mouse_capture(self.mouse != 0);
                    }
                    if cancel_pen {
                        self.source.cancel();
                        self.push(
                            Some(record.ordinal),
                            Pointer::Cancel {
                                reset_gui: self.mouse == 0,
                            },
                        )?;
                    }
                    if route != MouseRoute::Client {
                        continue;
                    }
                    // Match winit CursorEntered semantics: native tracking
                    // metadata must not fabricate physical pointer motion.
                    if mouse.kind == MouseKind::Enter {
                        continue;
                    }
                    let Some(view) = self.view else {
                        continue;
                    };
                    let at = position(mouse.position, view.pixels_per_point)?;
                    let m = modifiers(mouse.modifiers);
                    match mouse.kind {
                        MouseKind::Enter => unreachable!("tracking entry handled above"),
                        MouseKind::Motion => self.push(
                            Some(record.ordinal),
                            Pointer::Gui(egui::Event::PointerMoved(at)),
                        )?,
                        MouseKind::Button { number, pressed } => {
                            if let Some(button) = button(number) {
                                self.push(
                                    Some(record.ordinal),
                                    Pointer::Gui(egui::Event::PointerButton {
                                        pos: at,
                                        button,
                                        pressed,
                                        modifiers: m,
                                    }),
                                )?;
                            }
                        }
                        MouseKind::Leave => {
                            self.push(Some(record.ordinal), Pointer::Gui(egui::Event::PointerGone))?
                        }
                        MouseKind::Wheel {
                            delta,
                            precise,
                            physical_scale,
                            phase,
                            momentum,
                        } => {
                            let scale = if precise {
                                physical_scale / view.pixels_per_point
                            } else {
                                1.0
                            };
                            let phase = match phase {
                                WheelPhase::Start => egui::TouchPhase::Start,
                                WheelPhase::Move => egui::TouchPhase::Move,
                                WheelPhase::End => egui::TouchPhase::End,
                            };
                            self.push(
                                Some(record.ordinal),
                                Pointer::Gui(egui::Event::MouseWheel {
                                    unit: if precise {
                                        egui::MouseWheelUnit::Point
                                    } else {
                                        egui::MouseWheelUnit::Line
                                    },
                                    delta: egui::vec2(
                                        (delta.0 * scale) as f32,
                                        (delta.1 * scale) as f32,
                                    ),
                                    modifiers: m,
                                    phase,
                                    source: if momentum {
                                        egui::MouseWheelSource::Momentum
                                    } else if precise {
                                        egui::MouseWheelSource::Trackpad
                                    } else {
                                        egui::MouseWheelSource::Wheel
                                    },
                                }),
                            )?;
                        }
                    }
                }
                Collected::Boundary(record) => {
                    let CaptureEvent::Boundary { kind, .. } = record.event else {
                        return Err(RecordFault::Invalid);
                    };
                    if kind == Boundary::PointerIdle {
                        self.mouse = 0;
                        self.source.mouse_capture(false);
                    } else {
                        self.boundary(Some(record.ordinal), kind)?;
                    }
                }
                Collected::IdentityRetired(record) => {
                    let CaptureEvent::IdentityRetired { identity, .. } = record.event else {
                        return Err(RecordFault::Invalid);
                    };
                    self.source
                        .retire_native_identity(identity)
                        .map_err(|_| RecordFault::Invalid)?;
                }
                Collected::Finger { .. } => return Err(RecordFault::Unsupported),
                Collected::FrameworkBoundary(kind) => self.boundary(None, kind)?,
            }
        }
        Ok(())
    }
    fn flush_pen(&mut self) -> std::result::Result<(), RecordFault> {
        let Some(ordinal) = self.span_first.take() else {
            return Ok(());
        };
        let Some(view) = self.view else { return Ok(()) };
        self.handoff = self.handoff.checked_add(1).ok_or(RecordFault::Retired)?;
        let batch = self
            .source
            .take(self.handoff, view.owner, false)
            .map_err(|_| RecordFault::Capacity)?;
        self.push(Some(ordinal), Pointer::Pen(batch))
    }
    pub fn framework_boundary(&mut self, kind: Boundary) -> std::result::Result<(), RecordFault> {
        self.collector.framework_boundary(kind)?;
        // Same explicit conservative barrier as native boundaries; no invented
        // ordinal or attempted key/IME-to-native-event authentication.
        let frame = self
            .collector
            .take(DeliveryHealth::Ready)
            .map_err(|_| RecordFault::Capacity)?;
        for _ in &frame.records {
            self.boundary(None, kind)?;
        }
        Ok(())
    }
    fn boundary(
        &mut self,
        ordinal: Option<u64>,
        kind: Boundary,
    ) -> std::result::Result<(), RecordFault> {
        if kind == Boundary::ViewInvalidated {
            self.source.invalidate_view();
            self.view = None;
            self.span_first = None;
            self.epoch = self.epoch.checked_add(1).ok_or(RecordFault::Retired)?;
            return self.push(ordinal, Pointer::Cancel { reset_gui: true });
        }
        self.source.cancel();
        self.span_first = None;
        let window_retired = matches!(
            kind,
            Boundary::FocusLost | Boundary::WindowClosed | Boundary::Reentrant
        );
        let reset_gui = window_retired || self.mouse == 0;
        if window_retired {
            self.epoch = self.epoch.checked_add(1).ok_or(RecordFault::Retired)?;
            self.mouse = 0;
            self.source.mouse_capture(false);
        }
        self.push(ordinal, Pointer::Cancel { reset_gui })
    }
    fn push(
        &mut self,
        ordinal: Option<u64>,
        pointer: Pointer,
    ) -> std::result::Result<(), RecordFault> {
        if self.queue.len() == self.limit {
            return Err(RecordFault::Capacity);
        }
        self.queue.push(Tagged {
            ordinal,
            pointer,
            epoch: self.epoch,
        });
        Ok(())
    }
    pub(crate) fn interrupt_pending(&mut self, kind: Boundary) -> Result<()> {
        self.queue.clear();
        self.span_first = None;
        self.framework_boundary(kind)
            .map_err(|_| resource("Owned input interruption"))
    }
    pub fn fault(&mut self) {
        self.failed = true;
        self.queue.clear();
        self.collector.fault();
        self.source.fault();
    }
    pub fn take(&mut self, health: DeliveryHealth) -> Result<Frame> {
        if health == DeliveryHealth::Failed {
            self.fault();
        }
        if self.failed {
            return Err(resource("Pointer projection retired"));
        }
        // Do not split a later contact ahead of an earlier semantic edge's
        // receipt. With no earlier edge, hand off the held contact for preview.
        if self.queue.is_empty() {
            self.flush_pen()
                .map_err(|_| resource("Pointer canvas handoff"))?;
        }
        let mut storage = self.frames.account();
        storage.resize((self.queue.len() * frame_slot_bytes()) as u64)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(self.queue.len())
            .map_err(|_| resource("Pointer handoff"))?;
        storage.resize((entries.capacity() * frame_slot_bytes()) as u64)?;
        entries.append(&mut self.queue);
        Ok(Frame {
            entries,
            source: self.source.clone(),
            _storage: storage,
        })
    }
    pub fn append_gui(
        &self,
        frame: &Frame,
        owner: Owner,
        events: &mut Vec<egui::Event>,
    ) -> Result<()> {
        let count = frame
            .entries
            .iter()
            .filter(|entry| self.is_current(entry, owner))
            .map(|entry| match &entry.pointer {
                Pointer::Gui(_) => 1,
                Pointer::Pen(batch) => batch.frame.ui.len(),
                Pointer::Cancel { .. } => 0,
            })
            .sum::<usize>();
        if count > self.limit * 3 {
            return Err(resource("Pointer GUI projection slots"));
        }
        let old_capacity = events.capacity();
        events
            .try_reserve_exact(count)
            .map_err(|_| resource("Pointer GUI event reserve"))?;
        if events.capacity().saturating_sub(old_capacity) > frame.entries.capacity() * 3 {
            return Err(resource(
                "Pointer GUI allocator capacity exceeded its allowance",
            ));
        }
        for entry in &frame.entries {
            if !self.is_current(entry, owner) {
                continue;
            }
            match &entry.pointer {
                Pointer::Gui(event) => events.push(event.clone()),
                Pointer::Pen(batch) => {
                    events.extend(batch.frame.ui.iter().map(|r| ui_event(r.event)))
                }
                Pointer::Cancel { .. } => {}
            }
        }
        Ok(())
    }
    pub(crate) fn flush_deferred_pen(&mut self) -> Result<()> {
        self.flush_pen()
            .map_err(|_| resource("Deferred canvas handoff"))
    }
    pub(crate) fn queued_mut(&mut self) -> &mut [Tagged] {
        &mut self.queue
    }
    pub(crate) fn source_epoch(&self) -> Option<u64> {
        self.source_epoch
    }
    pub(crate) fn source(&self) -> Source {
        self.source.clone()
    }
    pub(crate) fn view(&self) -> Option<View> {
        self.view
    }
    pub fn owner(&self) -> Option<Owner> {
        self.view.map(|view| view.owner)
    }
    pub fn is_current(&self, tagged: &Tagged, owner: Owner) -> bool {
        !self.failed
            && match &tagged.pointer {
                Pointer::Pen(batch) => self.source.batch_is_current(batch, owner),
                Pointer::Gui(_) => {
                    tagged.epoch == self.epoch
                        && self.view.is_some_and(|v| {
                            v.owner.session == owner.session
                                && v.owner.interface == owner.interface
                                && v.focused
                        })
                }
                Pointer::Cancel { .. } => true,
            }
    }
}
fn frame_slot_bytes() -> usize {
    std::mem::size_of::<Tagged>()
        + 3 * std::mem::size_of::<egui::Event>()
        + std::mem::size_of::<usize>()
}
fn resource(message: &'static str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn modifiers(m: paintcraft_input::Modifiers) -> egui::Modifiers {
    egui::Modifiers {
        alt: m.alt,
        ctrl: m.control,
        shift: m.shift,
        mac_cmd: m.command,
        command: m.command,
    }
}
fn button(number: u8) -> Option<egui::PointerButton> {
    Some(match number {
        0 => egui::PointerButton::Primary,
        1 => egui::PointerButton::Secondary,
        2 => egui::PointerButton::Middle,
        3 => egui::PointerButton::Extra1,
        4 => egui::PointerButton::Extra2,
        _ => return None,
    })
}
fn position(p: WindowPosition, ppp: f64) -> std::result::Result<egui::Pos2, RecordFault> {
    let (x, y, scale) = match p {
        WindowPosition::PhysicalPixels(x, y) => (x, y, 1.0),
        WindowPosition::SurfacePoints {
            x,
            y,
            physical_scale,
        } => (x, y, physical_scale),
    };
    let p = egui::pos2((x * scale / ppp) as f32, (y * scale / ppp) as f32);
    if p.is_finite() {
        Ok(p)
    } else {
        Err(RecordFault::Invalid)
    }
}
pub fn ui_event(event: UiEvent) -> egui::Event {
    match event {
        UiEvent::Move { at, .. } => egui::Event::PointerMoved(egui::pos2(at.0 as f32, at.1 as f32)),
        UiEvent::Button {
            at,
            button: b,
            pressed,
            modifiers: m,
        } => egui::Event::PointerButton {
            pos: egui::pos2(at.0 as f32, at.1 as f32),
            button: match b {
                paintcraft_input::PointerButton::Primary => egui::PointerButton::Primary,
                paintcraft_input::PointerButton::Secondary => egui::PointerButton::Secondary,
            },
            pressed,
            modifiers: modifiers(m),
        },
        UiEvent::Gone => egui::Event::PointerGone,
    }
}

#[cfg(test)]
#[path = "projection/tests.rs"]
mod tests;
