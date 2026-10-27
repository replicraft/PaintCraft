//! Safe canvas/UI arbitration for classified pen streams. No OS or GUI types.
use super::*;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// Unowned records remain with the platform/DOM's normal pointer translator.
    SystemPointer,
    /// The tablet adapter owns the full stream; emit ordinary UI pointer records.
    ReceiverPointer,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Canvas,
    Ui,
    System,
    Suppressed,
}
impl Route {
    pub fn consume(self) -> bool {
        self != Self::System
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiEvent {
    Move {
        at: (f64, f64),
        modifiers: Modifiers,
    },
    Button {
        at: (f64, f64),
        button: PointerButton,
        pressed: bool,
        modifiers: Modifiers,
    },
    Gone,
}
/// Source ordering is retained alongside each widget event. None is an internal
/// cancellation boundary, ordered before fresh source records; Some(0) remains
/// a valid source sequence and is never confused with that boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrderedUiEvent {
    pub sequence: Option<u64>,
    pub event: UiEvent,
}
#[derive(Debug, Default)]
pub struct Frame {
    pub samples: Vec<Sample>,
    pub ui: Vec<OrderedUiEvent>,
    /// Hosts send UI events to widgets but never back into legacy canvas tools.
    pub suppress_legacy_pointer: bool,
    /// Hosts cancel private tools and clear captured GUI pointer state before
    /// feeding queued release records, so cancellation cannot become a UI click.
    pub cancelled: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Canvas,
    Ui,
    Suppressed,
}
#[derive(Clone, Copy, Debug)]
struct Stream {
    identity: Identity,
    owner: u128,
    target: Target,
    contact: bool,
}
#[derive(Debug)]
pub struct Receiver {
    router: Router,
    ui: VecDeque<OrderedUiEvent>,
    ui_limit: usize,
    stream: Option<Stream>,
    ui_position: Option<(f64, f64)>,
    ui_primary: bool,
    ui_secondary: bool,
    release_primary: bool,
    release_secondary: bool,
    release_position: Option<(f64, f64)>,
    ui_modifiers: Modifiers,
    mouse_capture: bool,
    failed: bool,
    cancelled: bool,
    suppress: bool,
    frame: Option<u64>,
}
impl Receiver {
    /// Reserve source storage and three UI cancellation slots before installation.
    pub fn new(source_capacity: usize, ui_capacity: usize) -> Result<Self, Invalid> {
        if !(1..=196_608).contains(&ui_capacity) {
            return Err(Invalid::Capacity);
        }
        let mut ui = VecDeque::new();
        ui.try_reserve_exact(ui_capacity + 3)
            .map_err(|_| Invalid::Capacity)?;
        Ok(Self {
            router: Router::new(source_capacity)?,
            ui,
            ui_limit: ui_capacity,
            stream: None,
            ui_position: None,
            ui_primary: false,
            ui_secondary: false,
            release_primary: false,
            release_secondary: false,
            release_position: None,
            ui_modifiers: Modifiers::default(),
            mouse_capture: false,
            failed: false,
            cancelled: false,
            suppress: false,
            frame: None,
        })
    }
    pub fn storage_bytes(&self) -> usize {
        self.router.storage_bytes() + self.ui.capacity() * std::mem::size_of::<OrderedUiEvent>()
    }
    pub fn pen_active(&self) -> bool {
        self.router.pen_active()
    }
    /// Includes a completed but not-yet-drained stroke and held UI buttons.
    /// A host owner change with no classified work must not suppress mouse input.
    pub fn has_capture_or_samples(&self) -> bool {
        self.stream.is_some() || self.router.pending_samples() != 0 || !self.ui.is_empty()
    }
    /// Widget coordinates do not depend on document state or canvas geometry.
    pub fn has_canvas_capture_or_samples(&self) -> bool {
        self.router.pending_samples() != 0 || self.stream.is_some_and(|s| s.target != Target::Ui)
    }
    /// Genuine mouse capture wins over later pen hover/motion. Pen-held capture
    /// refuses a competing mouse gesture; hosts retain the same one-pointer rule.
    pub fn mouse_capture(&mut self, held: bool) -> bool {
        if held && self.stream.is_some() {
            return false;
        }
        self.mouse_capture = held;
        true
    }
    pub fn finger(&mut self, id: u64, phase: Phase) -> Result<bool, Invalid> {
        self.router.finger_owned(id, phase, self.stream.is_some())
    }
    /// The host samples transform and permissions under its captured-view lock.
    /// Native physical/surface coordinates must already be converted to GUI points.
    pub fn submit(
        &mut self,
        raw: RawSample,
        transform: Transform,
        permissions: Permissions,
        delivery: Delivery,
    ) -> Result<Route, Invalid> {
        if self.failed || self.mouse_capture {
            return Ok(Route::Suppressed);
        }
        if raw.provenance == Provenance::Predicted {
            if raw.logical_position.is_some() {
                raw.at(transform)?;
            }
            return Ok(Route::Suppressed);
        }
        if !raw.identity.kind.is_pen() {
            return Err(Invalid::Owner);
        }
        if self.stream.is_some_and(|s| s.identity != raw.identity) {
            return Ok(Route::Suppressed);
        }
        if !permissions.focused {
            self.cancel();
            return Ok(Route::Suppressed);
        }
        if transform.owner != permissions.owner
            || self.stream.is_some_and(|s| s.owner != permissions.owner)
        {
            self.cancel();
            return Err(Invalid::Owner);
        }
        if matches!(raw.phase, Phase::Cancel | Phase::Leave) {
            self.cancel();
            return Ok(Route::Suppressed);
        }
        let Some(at) = raw.logical_position else {
            return if raw.phase == Phase::Enter {
                Ok(Route::Suppressed)
            } else {
                self.cancel();
                Err(Invalid::Coordinates)
            };
        };
        let sample = match raw.at(transform) {
            Ok(sample) => sample,
            Err(error) => {
                self.cancel();
                return Err(error);
            }
        };
        let canvas = permissions.canvas_hit && !permissions.modal && !permissions.ime;
        // Eraser-end identity is not itself a contact/button press. Inverted
        // tools may advertise the eraser bit while hovering and after tip Up.
        let contact = raw.phase == Phase::Down
            || (raw.phase != Phase::Up
                && (self.stream.is_some_and(|s| s.contact) || raw.buttons.0 & Buttons::TIP != 0));
        let held = contact || raw.buttons.0 & (Buttons::BARREL | Buttons::BARREL_UPPER) != 0;
        if self.stream.is_none() && (held || raw.phase == Phase::Down) {
            self.router.fence_fingers();
            self.stream = Some(Stream {
                identity: raw.identity,
                owner: permissions.owner,
                target: if canvas { Target::Canvas } else { Target::Ui },
                contact: false,
            });
        }
        if self.stream.is_some_and(|s| s.target == Target::Canvas)
            && (permissions.modal || permissions.ime)
        {
            self.cancel();
            self.stream = Some(Stream {
                identity: raw.identity,
                owner: permissions.owner,
                target: Target::Suppressed,
                contact: false,
            });
        }
        let target = self.stream.map_or(Target::Ui, |s| s.target);
        if let Some(stream) = &mut self.stream {
            if raw.phase == Phase::Down {
                stream.contact = true;
            }
            if raw.phase == Phase::Up {
                stream.contact = false;
            }
        }
        let route = match target {
            Target::Canvas => {
                self.suppress = true;
                // A barrel pressed while hovering may own the canvas but cannot
                // create ink. Only a real contact Down reaches the source queue.
                if matches!(raw.phase, Phase::Down | Phase::Move | Phase::Up) {
                    if let Err(error) = self.router.submit(sample, permissions) {
                        self.cancel();
                        return Err(error);
                    }
                } else if self.router.pen_active() {
                    self.cancel();
                    return Ok(Route::Suppressed);
                }
                Route::Canvas
            }
            Target::Suppressed => {
                self.suppress = true;
                Route::Suppressed
            }
            Target::Ui => {
                self.suppress |= held || raw.phase == Phase::Up;
                if delivery == Delivery::ReceiverPointer {
                    self.ui_event(at, raw.buttons, raw.modifiers, contact, raw.sequence)?;
                    Route::Ui
                } else {
                    Route::System
                }
            }
        };
        if !held && self.stream.is_some_and(|s| !s.contact) {
            self.stream = None;
        }
        Ok(route)
    }
    fn ui_event(
        &mut self,
        at: (f64, f64),
        buttons: Buttons,
        modifiers: Modifiers,
        primary: bool,
        sequence: u64,
    ) -> Result<(), Invalid> {
        let secondary = buttons.0 & (Buttons::BARREL | Buttons::BARREL_UPPER) != 0;
        let mut pending = [None; 3];
        let mut count = 0;
        if self.ui_position != Some(at) {
            pending[count] = Some(UiEvent::Move { at, modifiers });
            count += 1;
        }
        for (old, new, button) in [
            (self.ui_primary, primary, PointerButton::Primary),
            (self.ui_secondary, secondary, PointerButton::Secondary),
        ] {
            if old != new {
                pending[count] = Some(UiEvent::Button {
                    at,
                    button,
                    pressed: new,
                    modifiers,
                });
                count += 1;
            }
        }
        if self.ui.len() + count > self.ui_limit {
            self.cancel();
            return Err(Invalid::Capacity);
        }
        self.ui
            .extend(pending.into_iter().flatten().map(|event| OrderedUiEvent {
                sequence: Some(sequence),
                event,
            }));
        self.ui_position = Some(at);
        self.ui_primary = primary;
        self.ui_secondary = secondary;
        self.ui_modifiers = modifiers;
        Ok(())
    }
    /// Focus/modal/capture loss cancels private ink and releases UI buttons at the
    /// last valid UI position. This does not fabricate a hardware source sample.
    pub fn cancel(&mut self) {
        self.router.cancel();
        // A cancellation retires every unpublished source record, including an
        // Up that already closed Queue ownership. The host first cancels its
        // private gesture on Frame.cancelled, then may handle fresh later input.
        self.router.discard_pending();
        self.stream = None;
        self.cancelled = true;
        self.suppress = true;
        self.release_primary |= self.ui_primary;
        self.release_secondary |= self.ui_secondary;
        let position = self.ui_position.take().or(self.release_position);
        // Unpublished presses cannot turn into widget clicks after cancellation.
        // Retain required releases across repeated cancels before frame delivery.
        self.ui.clear();
        if let Some(at) = position {
            self.release_position = Some(at);
            for (held, button) in [
                (self.release_primary, PointerButton::Primary),
                (self.release_secondary, PointerButton::Secondary),
            ] {
                if held {
                    self.ui.push_back(OrderedUiEvent {
                        sequence: None,
                        event: UiEvent::Button {
                            at,
                            button,
                            pressed: false,
                            modifiers: self.ui_modifiers,
                        },
                    });
                }
            }
            self.ui.push_back(OrderedUiEvent {
                sequence: None,
                event: UiEvent::Gone,
            });
        }
        self.ui_primary = false;
        self.ui_secondary = false;
        debug_assert!(self.ui.len() <= self.ui_limit + 3);
    }
    /// A native callback failure can occur after a partial enqueue, including Up.
    /// Discard all unpublished source records and stay fenced until reinstall.
    pub fn fault(&mut self) {
        self.cancel();
        self.router.discard_pending();
        self.failed = true;
    }
    pub fn reset(&mut self) {
        self.cancel();
        self.router.discard_pending();
        self.router.reset();
        self.failed = false;
        self.mouse_capture = false;
        self.frame = None;
    }
    /// Fallible once-per-frame handoff. Call under the native delivery-health gate
    /// so partial-enqueue failure observation cannot race source publication.
    pub fn take(
        &mut self,
        frame: u64,
        admit: impl FnOnce(usize) -> Result<(), Invalid>,
    ) -> Result<Frame, Invalid> {
        if self.frame == Some(frame) {
            return Ok(Frame::default());
        }
        let bytes = self.router.pending_samples() * std::mem::size_of::<Sample>()
            + self.ui.len() * std::mem::size_of::<OrderedUiEvent>();
        if let Err(error) = admit(bytes) {
            self.cancel();
            return Err(error);
        }
        let mut ui = Vec::new();
        if ui.try_reserve_exact(self.ui.len()).is_err() {
            self.cancel();
            return Err(Invalid::Capacity);
        }
        let samples = match self.router.take(frame) {
            Ok(samples) => samples,
            Err(error) => {
                self.cancel();
                return Err(error);
            }
        };
        ui.extend(self.ui.drain(..));
        self.release_primary = false;
        self.release_secondary = false;
        self.release_position = None;
        self.frame = Some(frame);
        Ok(Frame {
            samples,
            ui,
            suppress_legacy_pointer: std::mem::take(&mut self.suppress) || self.stream.is_some(),
            cancelled: std::mem::take(&mut self.cancelled),
        })
    }
}

#[cfg(test)]
mod tests;
