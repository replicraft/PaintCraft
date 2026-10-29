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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureStatus {
    pub pen: bool,
    pub mouse: bool,
    pub failed: bool,
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
    /// GUI reset is separate from private pen retirement. Pen-only cancellation
    /// while genuine mouse owns capture must not release its widget buttons.
    pub reset_gui_pointer: bool,
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
    buttons: Buttons,
    ui_prefix: usize,
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
    reset_gui_pointer: bool,
    suppress: bool,
    frame: Option<u64>,
    cancellation_epoch: u64,
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
            reset_gui_pointer: false,
            suppress: false,
            frame: None,
            cancellation_epoch: 0,
        })
    }
    pub fn storage_bytes(&self) -> usize {
        self.router.storage_bytes() + self.ui.capacity() * std::mem::size_of::<OrderedUiEvent>()
    }
    pub fn pen_active(&self) -> bool {
        self.router.pen_active()
    }
    pub fn cancellation_epoch(&self) -> u64 {
        self.cancellation_epoch
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
    /// Physical capture only: completed queued samples are not a new contact.
    pub fn capture_status(&self) -> CaptureStatus {
        CaptureStatus {
            pen: self.stream.is_some(),
            mouse: self.mouse_capture,
            failed: self.failed,
        }
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
        if raw.provenance == Provenance::Coalesced
            && (self.stream.is_none() || !matches!(raw.phase, Phase::Move | Phase::Hover))
        {
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
        let effective_buttons = if raw.provenance == Provenance::Coalesced {
            self.stream.map_or(Buttons(0), |stream| stream.buttons)
        } else {
            raw.buttons
        };
        let contact = raw.phase == Phase::Down
            || (raw.phase != Phase::Up
                && (self.stream.is_some_and(|s| s.contact)
                    || effective_buttons.0 & Buttons::TIP != 0));
        let held = contact || effective_buttons.0 & (Buttons::BARREL | Buttons::BARREL_UPPER) != 0;
        if self.stream.is_none() && (held || raw.phase == Phase::Down) {
            self.router.fence_fingers();
            self.stream = Some(Stream {
                identity: raw.identity,
                owner: permissions.owner,
                target: if canvas { Target::Canvas } else { Target::Ui },
                contact: false,
                buttons: raw.buttons,
                ui_prefix: self.ui.len(),
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
                buttons: raw.buttons,
                ui_prefix: self.ui.len(),
            });
        }
        let target = self.stream.map_or(Target::Ui, |s| s.target);
        if raw.provenance == Provenance::Actual
            && let Some(stream) = &mut self.stream
        {
            stream.buttons = raw.buttons;
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
                    if raw.provenance == Provenance::Coalesced {
                        // Historical geometry may move an already-owned UI
                        // contact, but only Actual records change its buttons.
                        if self.ui.len() == self.ui_limit {
                            self.cancel();
                            return Err(Invalid::Capacity);
                        }
                        self.ui.push_back(OrderedUiEvent {
                            sequence: Some(raw.sequence),
                            event: UiEvent::Move {
                                at,
                                modifiers: raw.modifiers,
                            },
                        });
                    } else {
                        self.ui_event(at, raw.buttons, raw.modifiers, contact, raw.sequence)?;
                    }
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
    /// Retire only the matching classified stream, without fabricating a raw
    /// terminal or cancelling a different device's contact.
    pub fn captures_identity(&self, identity: Identity) -> bool {
        self.stream
            .is_some_and(|stream| stream.identity == identity)
    }
    pub fn cancel_identity(&mut self, identity: Identity) -> bool {
        if self.captures_identity(identity) {
            self.cancel();
            true
        } else {
            false
        }
    }
    /// Focus/modal/capture loss cancels private ink and releases UI buttons at the
    /// last valid UI position. This does not fabricate a hardware source sample.
    pub fn cancel(&mut self) {
        self.cancellation_epoch = self.cancellation_epoch.saturating_add(1);
        self.cancel_inner(true, true);
    }
    /// Source separately revokes its pen-only admission epoch. The global
    /// cancellation token and genuine mouse ownership remain unchanged.
    pub fn reset_gui_pointer_pending(&self) -> bool {
        self.reset_gui_pointer
    }
    pub fn cancel_pen_only(&mut self) {
        let hold = self.stream.is_some() || self.ui_primary || self.ui_secondary;
        let ui_hold = self.ui_primary
            || self.ui_secondary
            || self.release_primary
            || self.release_secondary
            || self.stream.is_some_and(|s| s.target == Target::Ui);
        self.cancel_inner(!self.mouse_capture && ui_hold, !self.mouse_capture && hold);
    }
    /// Exact active-only queue retirement for an opaque native host lifetime.
    /// No global cancellation epoch/reset flag is emitted. The host must use
    /// its matching cancellation lease to retire already-presented GUI state.
    pub fn retire_active_pen(&mut self) {
        let Some(stream) = self.stream.take() else {
            return;
        };
        self.router.retire_active_pen();
        if stream.target == Target::Ui {
            self.ui.truncate(stream.ui_prefix);
            self.ui_primary = false;
            self.ui_secondary = false;
            self.ui_position = None;
            self.release_primary = false;
            self.release_secondary = false;
            self.release_position = None;
        }
    }
    fn cancel_inner(&mut self, reset_gui: bool, suppress: bool) {
        self.router.cancel();
        // A cancellation retires every unpublished source record, including an
        // Up that already closed Queue ownership. The host first cancels its
        // private gesture on Frame.cancelled, then may handle fresh later input.
        self.router.discard_pending();
        self.stream = None;
        self.cancelled = true;
        self.reset_gui_pointer |= reset_gui;
        self.suppress = suppress || self.reset_gui_pointer;
        if !self.reset_gui_pointer {
            self.ui.clear();
            self.ui_primary = false;
            self.ui_secondary = false;
            self.release_primary = false;
            self.release_secondary = false;
            self.release_position = None;
            self.ui_position = None;
            return;
        }
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
        if let Some(stream) = &mut self.stream {
            stream.ui_prefix = 0;
        }
        self.release_primary = false;
        self.release_secondary = false;
        self.release_position = None;
        self.frame = Some(frame);
        Ok(Frame {
            samples,
            ui,
            suppress_legacy_pointer: std::mem::take(&mut self.suppress) || self.stream.is_some(),
            cancelled: std::mem::take(&mut self.cancelled),
            reset_gui_pointer: std::mem::take(&mut self.reset_gui_pointer),
        })
    }
}

#[cfg(test)]
mod tests;
