//! Frontend-independent input, after viewport-to-document mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerEvent {
    Down { at: (i64, i64) },
    Move { at: (i64, i64) },
    Up,
    Cancel,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenSample {
    pub position: (f64, f64),
    pub pressure: f32,
    pub tilt: (f32, f32),
    pub timestamp_micros: u64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub command: bool,
}
