//! Source-observed records for an owned native intake, before GUI batching.
use crate::{NativePacket, WindowPosition};
use paintcraft_input::{
    Modifiers,
    tablet::{Identity, Phase, Provenance, Time},
};
use std::sync::Arc;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordFault {
    Capacity,
    Invalid,
    Retired,
    /// A source record is well-defined but its consumer is not yet qualified.
    Unsupported,
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
    /// Native scale/backing/window geometry changed before a completed GUI
    /// layout. Consumers must invalidate the old view immediately, not alias
    /// this to navigation or wait for a framework receipt.
    ViewInvalidated,
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
    Boundary {
        kind: Boundary,
        time: Option<Time>,
    },
    /// Explicit native lifetime end, not ordinary in-range window Leave.
    /// Producer verified !INRANGE and no held buttons. Consumer must also verify
    /// exact identity/source epoch/ordered clock and no captured tail before
    /// reclaiming only that identity's history. No generic eviction is allowed.
    IdentityRetired {
        identity: Identity,
        time: Time,
    },
    /// Actual native touch lifecycle; identity.kind must be Finger. No pen
    /// pressure or invented axes. Unmapped terminal/control may lack position.
    /// Predictions cannot claim ownership or clear physical tail fences.
    Finger {
        identity: Identity,
        phase: Phase,
        time: Time,
        provenance: Provenance,
        position: Option<WindowPosition>,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureRecord {
    pub installation: u64,
    /// Checked order in this one native producer, not a sort of clock values.
    pub ordinal: u64,
    pub event: CaptureEvent,
}
