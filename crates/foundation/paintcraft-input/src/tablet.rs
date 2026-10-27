//! Ordered, device-aware input independent of UI frames or a platform event loop.
mod queue;
mod routing;
use crate::{Modifiers, PointerButton};
pub use queue::{Admission, Queue};
pub use routing::{Permissions, Router};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeviceKind {
    Mouse,
    Pen,
    Eraser,
    Finger,
}
impl DeviceKind {
    pub fn is_pen(self) -> bool {
        matches!(self, Self::Pen | Self::Eraser)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Down,
    Move,
    Up,
    Cancel,
    Hover,
    Enter,
    Leave,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provenance {
    Actual,
    Coalesced,
    Predicted,
    /// Internal cancellation placed at the last valid source sample; never ink.
    Boundary,
}
/// Different clocks must never be silently sorted into one hardware timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock {
    HostReceipt,
    BrowserPerformance,
    AppKitUptime,
    WindowsPerformance,
    WindowsTicks,
    XServer,
    Wayland,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Time {
    /// Source clock origin/generation (for example a browser time-origin token).
    pub epoch: u64,
    pub micros: u64,
    pub clock: Clock,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tilt {
    Degrees(f32, f32),
    Normalized(f32, f32),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Buttons(pub u16);
impl Buttons {
    pub const TIP: u16 = 1;
    pub const BARREL: u16 = 2;
    pub const BARREL_UPPER: u16 = 4;
    pub const ERASER: u16 = 32;
    pub fn ink_button(self) -> PointerButton {
        if self.0 & (Self::BARREL | Self::BARREL_UPPER) != 0 {
            PointerButton::Secondary
        } else {
            PointerButton::Primary
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Identity {
    pub device: u64,
    pub pointer: u64,
    pub kind: DeviceKind,
}
/// View captured at ingestion. A delayed sample never consults today's zoom/DPI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub owner: u128,
    pub epoch: u64,
    pub origin: (f64, f64),
    pub points_per_pixel: f64,
}
impl Transform {
    pub fn document(self, logical: (f64, f64)) -> Result<(f64, f64), Invalid> {
        if !self.points_per_pixel.is_finite()
            || self.points_per_pixel <= 0.0
            || !self.origin.0.is_finite()
            || !self.origin.1.is_finite()
            || !logical.0.is_finite()
            || !logical.1.is_finite()
        {
            return Err(Invalid::Coordinates);
        }
        let point = (
            (logical.0 - self.origin.0) / self.points_per_pixel,
            (logical.1 - self.origin.1) / self.points_per_pixel,
        );
        if !point.0.is_finite() || !point.1.is_finite() {
            return Err(Invalid::Coordinates);
        }
        Ok(point)
    }
}
/// Platform acquisition in window-logical points. Proximity/cancel may have no
/// position; hosts must not manufacture an origin for such lifecycle events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawSample {
    pub identity: Identity,
    pub sequence: u64,
    pub time: Time,
    pub phase: Phase,
    pub logical_position: Option<(f64, f64)>,
    pub buttons: Buttons,
    pub modifiers: Modifiers,
    pub pressure: Option<f32>,
    pub tilt: Option<Tilt>,
    pub twist_degrees: Option<f32>,
    pub contact: Option<(f32, f32)>,
    pub provenance: Provenance,
}
impl RawSample {
    pub fn at(self, transform: Transform) -> Result<Sample, Invalid> {
        let position = transform.document(self.logical_position.ok_or(Invalid::Coordinates)?)?;
        let sample = Sample {
            identity: self.identity,
            sequence: self.sequence,
            time: self.time,
            phase: self.phase,
            position,
            transform,
            buttons: self.buttons,
            modifiers: self.modifiers,
            pressure: self.pressure,
            tilt: self.tilt,
            twist_degrees: self.twist_degrees,
            contact: self.contact,
            provenance: self.provenance,
        };
        sample.validate()?;
        Ok(sample)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub identity: Identity,
    pub sequence: u64,
    pub time: Time,
    pub phase: Phase,
    /// Continuous document pixels, converted using transform at event capture.
    pub position: (f64, f64),
    pub transform: Transform,
    pub buttons: Buttons,
    pub modifiers: Modifiers,
    /// None is unavailable. Some(0) is reported zero and cannot become full force.
    pub pressure: Option<f32>,
    pub tilt: Option<Tilt>,
    pub twist_degrees: Option<f32>,
    pub contact: Option<(f32, f32)>,
    pub provenance: Provenance,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    Coordinates,
    Pressure,
    Tilt,
    Twist,
    Contact,
    Order,
    Clock,
    Capacity,
    Lifecycle,
    Owner,
}
impl Sample {
    pub fn validate(self) -> Result<(), Invalid> {
        if self.provenance == Provenance::Boundary && self.phase != Phase::Cancel {
            return Err(Invalid::Lifecycle);
        }
        if !self.position.0.is_finite() || !self.position.1.is_finite() {
            return Err(Invalid::Coordinates);
        }
        self.transform.document(self.transform.origin)?;
        if self
            .pressure
            .is_some_and(|p| !p.is_finite() || !(0.0..=1.0).contains(&p))
        {
            return Err(Invalid::Pressure);
        }
        if let Some(tilt) = self.tilt {
            let (x, y, limit) = match tilt {
                Tilt::Degrees(x, y) => (x, y, 90.0),
                Tilt::Normalized(x, y) => (x, y, 1.0),
            };
            if !x.is_finite() || !y.is_finite() || x.abs() > limit || y.abs() > limit {
                return Err(Invalid::Tilt);
            }
        }
        if self
            .twist_degrees
            .is_some_and(|v| !v.is_finite() || !(0.0..360.0).contains(&v))
        {
            return Err(Invalid::Twist);
        }
        if self
            .contact
            .is_some_and(|(w, h)| !w.is_finite() || !h.is_finite() || w < 0.0 || h < 0.0)
        {
            return Err(Invalid::Contact);
        }
        Ok(())
    }
    pub fn q8(self) -> Result<(i64, i64), Invalid> {
        self.validate()?;
        let (x, y) = (self.position.0 * 256.0, self.position.1 * 256.0);
        if x.abs() >= i64::MAX as f64 || y.abs() >= i64::MAX as f64 {
            return Err(Invalid::Coordinates);
        }
        Ok((x.round() as i64, y.round() as i64))
    }
}
#[cfg(test)]
mod tests;
