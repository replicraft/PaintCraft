//! Frontend-independent input, after viewport-to-document mapping.
pub mod tablet;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Secondary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerEvent {
    /// Legacy primary-button event.
    Down {
        at: (i64, i64),
    },
    ButtonDown {
        at: (i64, i64),
        button: PointerButton,
    },
    ButtonUp {
        button: PointerButton,
    },
    Move {
        at: (i64, i64),
    },
    Up,
    Cancel,
}
/// Rich optional-attribute pen input; missing force is never fabricated zero.
pub type PenSample = tablet::Sample;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub command: bool,
}
