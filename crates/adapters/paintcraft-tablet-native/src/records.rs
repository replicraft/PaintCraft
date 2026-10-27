//! Source-observed records for an owned native intake, before GUI batching.
use crate::{NativePacket, WindowPosition};
use paintcraft_input::{Modifiers, tablet::Time};
use std::sync::Arc;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordFault {
    Capacity,
    Invalid,
    Retired,
}
pub type RecordSink = Arc<dyn Fn(CaptureRecord) -> Result<(), RecordFault> + Send + Sync>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boundary {
    Keyboard,
    FocusLost,
    /// Source observed combined native buttons idle after focus loss. This is
    /// a recovery boundary, never a fabricated mouse Up or pen proximity.
    PointerIdle,
    WindowClosed,
    ImeReceipt,
    Reentrant,
    Navigation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelPhase {
    Start,
    Move,
    End,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MouseKind {
    Motion,
    Button {
        number: u8,
        pressed: bool,
    },
    Leave,
    Wheel {
        delta: (f64, f64),
        precise: bool,
        physical_scale: f64,
        phase: WheelPhase,
        momentum: bool,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MouseRecord {
    /// Native button edge numbers, not a fabricated mouse/device identity.
    /// AppKit does not expose an individual physical mouse ID here.
    pub kind: MouseKind,
    pub position: WindowPosition,
    /// Geometric client membership, independent of keyboard focus.
    pub client: bool,
    pub modifiers: Modifiers,
    pub time: Time,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CaptureEvent {
    Tablet(NativePacket),
    Mouse(MouseRecord),
    Boundary { kind: Boundary, time: Option<Time> },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureRecord {
    pub installation: u64,
    /// Checked order in this one native producer, not a sort of clock values.
    pub ordinal: u64,
    pub event: CaptureEvent,
}
