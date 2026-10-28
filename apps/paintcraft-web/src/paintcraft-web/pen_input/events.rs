//! Pointer Events contact semantics. A chorded tip transition may be a Move,
//! while barrel-only Down/Up is hovering and must never create ink.
use paintcraft_input::{Modifiers, tablet::*};

/// Same metadata-first handoff for DOM and headless handler controls. Fenced
/// physical tails still reach the fixed metadata store so a real Up can retire
/// their identity, while neither coordinates nor UI/ink are delivered.
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
}
#[derive(Debug)]
pub(super) struct Normalizer {
    tools: [Option<Tool>; 8],
    sequence: u64,
    epoch: u64,
    cancellation_epoch: Option<u64>,
}
impl Normalizer {
    pub fn new(epoch: u64) -> Self {
        Self {
            tools: [None; 8],
            sequence: 0,
            epoch,
            cancellation_epoch: None,
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
            .flatten()
            .find(|tool| tool.id == id && tool.held)
        {
            tool.fenced = true;
        }
    }
    pub fn clear(&mut self) {
        for tool in &mut self.tools {
            if let Some(held) = tool.as_mut().filter(|tool| tool.held) {
                held.fenced = true;
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
                .flatten()
                .any(|tool| tool.id == id && tool.tip)
    }
    pub fn contact_fenced(&self, id: u64) -> bool {
        self.tools
            .iter()
            .flatten()
            .any(|tool| tool.id == id && tool.fenced)
    }
    pub fn sample(&mut self, event: Pointer, provenance: Provenance) -> Result<RawSample, Invalid> {
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
        let index = self
            .tools
            .iter()
            .position(|t| t.is_some_and(|t| t.id == event.id))
            .or_else(|| self.tools.iter().position(Option::is_none))
            .or_else(|| self.tools.iter().position(|t| t.is_some_and(|t| !t.held)))
            .ok_or(Invalid::Capacity)?;
        let old = self.tools[index].filter(|tool| tool.id == event.id);
        let tip = event.buttons & (Buttons::TIP | Buttons::ERASER) != 0;
        let held = event.buttons != 0;
        if old.is_none() && event.kind == Kind::Move && tip {
            return Err(Invalid::Lifecycle);
        }
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
        if raw.logical_position.is_some() {
            raw.at(Transform {
                owner: 0,
                epoch: 0,
                origin: (0.0, 0.0),
                points_per_pixel: 1.0,
            })?;
        }
        self.sequence = sequence;
        self.tools[index] = if matches!(phase, Phase::Cancel | Phase::Leave)
            || (old.is_some_and(|tool| tool.fenced) && !held)
        {
            None
        } else {
            Some(Tool {
                id: event.id,
                tip,
                held,
                inverted,
                fenced: old.is_some_and(|tool| tool.fenced),
            })
        };
        Ok(raw)
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
mod tests {
    use super::*;
    mod metadata;
    fn event(kind: Kind, buttons: u16) -> Pointer {
        Pointer {
            id: 4,
            kind,
            at: Some((12.25, 19.75)),
            stamp: 100.5,
            buttons,
            pressure: 0.25,
            tilt: (12.0, -8.0),
            twist: 31.0,
            contact: (2.5, 1.5),
            modifiers: Modifiers::default(),
        }
    }
    #[test]
    fn barrel_first_then_tip_move_is_real_contact_and_tip_up_keeps_ui_barrel() {
        let mut n = Normalizer::new(3);
        assert_eq!(
            n.sample(event(Kind::Down, Buttons::BARREL), Provenance::Actual)
                .unwrap()
                .phase,
            Phase::Hover
        );
        let down = n
            .sample(
                event(Kind::Move, Buttons::BARREL | Buttons::TIP),
                Provenance::Actual,
            )
            .unwrap();
        assert_eq!(down.phase, Phase::Down);
        assert!(!n.starting(4, Buttons::TIP));
        assert_eq!(down.pressure, Some(0.25));
        assert_eq!(down.logical_position, Some((12.25, 19.75)));
        assert_eq!(down.tilt, Some(Tilt::Degrees(12.0, -8.0)));
        assert_eq!(down.twist_degrees, Some(31.0));
        assert_eq!(down.time.micros, 100_500);
        assert_eq!(
            n.sample(event(Kind::Move, Buttons::BARREL), Provenance::Actual)
                .unwrap()
                .phase,
            Phase::Up
        );
        assert_eq!(
            n.sample(event(Kind::Up, 0), Provenance::Actual)
                .unwrap()
                .phase,
            Phase::Hover
        );
    }
    #[test]
    fn inverted_kind_survives_up_and_bad_end_flip_is_refused() {
        let mut n = Normalizer::new(3);
        let down = n
            .sample(event(Kind::Down, Buttons::ERASER), Provenance::Actual)
            .unwrap();
        assert_eq!(down.identity.kind, DeviceKind::Eraser);
        assert_eq!(
            n.sample(event(Kind::Move, Buttons::TIP), Provenance::Actual),
            Err(Invalid::Lifecycle)
        );
        let up = n.sample(event(Kind::Up, 0), Provenance::Actual).unwrap();
        assert_eq!(up.phase, Phase::Up);
        assert_eq!(up.identity.kind, DeviceKind::Eraser);
    }
    #[test]
    fn coalesced_semantics_and_invalid_source_are_not_replaced_by_mouse_defaults() {
        let mut n = Normalizer::new(3);
        n.sample(event(Kind::Down, Buttons::TIP), Provenance::Actual)
            .unwrap();
        let sample = n
            .sample(event(Kind::Move, Buttons::TIP), Provenance::Coalesced)
            .unwrap();
        assert_eq!(sample.provenance, Provenance::Coalesced);
        let mut bad = event(Kind::Move, Buttons::TIP);
        bad.pressure = f32::NAN;
        assert_eq!(n.sample(bad, Provenance::Actual), Err(Invalid::Pressure));
        bad = event(Kind::Move, Buttons::TIP);
        bad.stamp = -1.0;
        assert_eq!(n.sample(bad, Provenance::Actual), Err(Invalid::Clock));
    }
    #[test]
    fn compatibility_pairing_never_suppresses_genuine_mouse_or_an_unpaired_event() {
        let pair = Pair {
            kind: Kind::Move,
            stamp: 100.5_f64.to_bits(),
            buttons: 0,
            trusted: true,
        };
        let mut gate = Compatibility::default();
        gate.pen(pair);
        assert!(gate.matches(pair));
        assert!(!gate.matches(pair));
        gate.pen(pair);
        gate.mouse();
        assert!(!gate.matches(pair));
        gate.pen(pair);
        assert!(!gate.matches(Pair {
            trusted: false,
            ..pair
        }));
        gate.pen(pair);
        assert!(!gate.matches(Pair { buttons: 1, ..pair }));
    }
    #[test]
    fn retired_pointer_ids_are_reused_without_discarding_any_held_end_identity() {
        let mut n = Normalizer::new(3);
        let mut held = event(Kind::Down, Buttons::ERASER | Buttons::BARREL);
        held.id = 77;
        n.sample(held, Provenance::Actual).unwrap();
        for id in 0..40 {
            let mut down = event(Kind::Down, Buttons::TIP);
            down.id = id;
            assert_eq!(
                n.sample(down, Provenance::Actual).unwrap().phase,
                Phase::Down
            );
            let mut up = event(Kind::Up, 0);
            up.id = id;
            assert_eq!(n.sample(up, Provenance::Actual).unwrap().phase, Phase::Up);
            assert_eq!(
                n.sample(up, Provenance::Actual).unwrap().phase,
                Phase::Hover
            );
        }
        let mut tip_up = event(Kind::Move, Buttons::BARREL);
        tip_up.id = 77;
        let up = n.sample(tip_up, Provenance::Actual).unwrap();
        assert_eq!(up.phase, Phase::Up);
        assert_eq!(up.identity.kind, DeviceKind::Eraser);
        let mut barrel_up = event(Kind::Up, 0);
        barrel_up.id = 77;
        assert_eq!(
            n.sample(barrel_up, Provenance::Actual)
                .unwrap()
                .identity
                .kind,
            DeviceKind::Eraser
        );
        n.clear();
        assert_eq!(
            n.sample(event(Kind::Leave, 0), Provenance::Actual)
                .unwrap()
                .phase,
            Phase::Leave
        );
    }

    #[test]
    fn predicted_terminal_cannot_close_actual_contact() {
        let mut n = Normalizer::new(3);
        n.sample(event(Kind::Down, Buttons::TIP), Provenance::Actual)
            .unwrap();
        assert_eq!(
            n.sample(event(Kind::Up, 0), Provenance::Predicted),
            Err(Invalid::Lifecycle)
        );
        assert_eq!(
            n.sample(event(Kind::Leave, 0), Provenance::Predicted),
            Err(Invalid::Lifecycle)
        );
        assert_eq!(
            n.sample(event(Kind::Move, Buttons::TIP), Provenance::Actual)
                .unwrap()
                .phase,
            Phase::Move
        );
        assert_eq!(
            n.sample(event(Kind::Up, 0), Provenance::Actual)
                .unwrap()
                .phase,
            Phase::Up
        );
    }
    #[test]
    fn capture_focus_and_resize_retirement_preserve_tip_eraser_and_barrel_tails() {
        for buttons in [
            Buttons::TIP,
            Buttons::ERASER,
            Buttons::BARREL,
            Buttons::TIP | Buttons::BARREL | Buttons::BARREL_UPPER,
        ] {
            let mut n = Normalizer::new(3);
            n.sample(event(Kind::Down, buttons), Provenance::Actual)
                .unwrap();
            // Shared by managed pointercancel/lostcapture, blur, resize and teardown.
            n.clear();
            assert!(n.fenced(event(Kind::Move, buttons)));
            assert!(n.fenced(event(Kind::Up, Buttons::BARREL)));
            assert!(n.fenced(event(Kind::Move, Buttons::BARREL)));
            assert!(n.fenced(event(Kind::Up, 0)));
            assert!(!n.fenced(event(Kind::Down, Buttons::TIP)));
            assert_eq!(
                n.sample(event(Kind::Down, Buttons::TIP), Provenance::Actual)
                    .unwrap()
                    .phase,
                Phase::Down
            );
            assert_eq!(
                n.sample(event(Kind::Up, 0), Provenance::Actual)
                    .unwrap()
                    .phase,
                Phase::Up
            );
        }
    }
    #[test]
    fn orphan_held_motion_cannot_invent_a_contact_after_lost_identity() {
        let mut n = Normalizer::new(3);
        assert_eq!(
            n.sample(event(Kind::Move, Buttons::TIP), Provenance::Actual),
            Err(Invalid::Lifecycle)
        );
        n.sample(event(Kind::Up, 0), Provenance::Actual).unwrap();
        assert_eq!(
            n.sample(event(Kind::Down, Buttons::TIP), Provenance::Actual)
                .unwrap()
                .phase,
            Phase::Down
        );
    }
    #[test]
    fn host_retirement_epoch_fences_held_contact_across_toolbar_until_release() {
        for buttons in [
            Buttons::TIP,
            Buttons::ERASER,
            Buttons::BARREL,
            Buttons::TIP | Buttons::BARREL_UPPER,
        ] {
            let mut n = Normalizer::new(3);
            n.observe_cancellation(10);
            n.sample(event(Kind::Down, buttons), Provenance::Actual)
                .unwrap();
            // Escape/IME/owner/skip retirement all call the same Source.cancel.
            n.observe_cancellation(11);
            let mut over_toolbar = event(Kind::Move, buttons);
            over_toolbar.at = Some((2.0, 2.0));
            assert!(n.fenced(over_toolbar));
            n.observe_cancellation(11); // Repeated GUI pass must not release tail.
            assert!(n.fenced(event(Kind::Up, Buttons::BARREL)));
            assert!(n.fenced(event(Kind::Up, 0)));
            assert_eq!(
                n.sample(event(Kind::Down, Buttons::TIP), Provenance::Actual)
                    .unwrap()
                    .phase,
                Phase::Down
            );
        }
    }
    #[test]
    fn mouse_owned_denial_keeps_pen_and_hover_barrel_tail_fenced_after_mouse_release() {
        for buttons in [
            Buttons::TIP,
            Buttons::ERASER,
            Buttons::BARREL,
            Buttons::BARREL_UPPER,
            Buttons::TIP | Buttons::BARREL,
        ] {
            let mut n = Normalizer::new(3);
            let down = n
                .sample(event(Kind::Down, buttons), Provenance::Actual)
                .unwrap();
            // Source rejected this held stream because genuine mouse owned it.
            n.deny_contact(down.identity.pointer);
            // Later mouse release is not permission to synthesize a UI press.
            assert!(n.fenced(event(Kind::Move, buttons)));
            assert!(n.fenced(event(Kind::Up, Buttons::BARREL)));
            assert!(n.fenced(event(Kind::Move, Buttons::BARREL)));
            assert!(n.fenced(event(Kind::Up, 0)));
            assert_eq!(
                n.sample(event(Kind::Down, Buttons::TIP), Provenance::Actual)
                    .unwrap()
                    .phase,
                Phase::Down
            );
        }
    }
}
