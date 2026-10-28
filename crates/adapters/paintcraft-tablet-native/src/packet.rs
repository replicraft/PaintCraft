use paintcraft_input::tablet::{Invalid, RawSample};
/// Where unowned tablet input resumes ordinary widget routing. Keyboard and IME
/// events are never part of this stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    SystemPointer,
    ReceiverPointer,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowPosition {
    PhysicalPixels(f64, f64),
    SurfacePoints { x: f64, y: f64, physical_scale: f64 },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativePacket {
    /// Native adapters leave logical_position empty. The host fills it under
    /// the same lock which captures the document owner and view transform.
    pub raw: RawSample,
    /// Independent real side-button edge; absence is not proof of a transition.
    pub button_edge: Option<paintcraft_input::tablet::PenButtonEdge>,
    pub position: Option<WindowPosition>,
    pub delivery: Delivery,
}
impl NativePacket {
    pub fn into_logical(self, pixels_per_point: f64) -> Result<RawSample, Invalid> {
        if self.raw.logical_position.is_some() {
            return Err(Invalid::Coordinates);
        }
        let mut raw = self.raw;
        let Some(position) = self.position else {
            return Ok(raw);
        };
        if !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
            return Err(Invalid::Coordinates);
        }
        let (x, y, scale) = match position {
            WindowPosition::PhysicalPixels(x, y) => (x, y, 1.0),
            WindowPosition::SurfacePoints {
                x,
                y,
                physical_scale,
            } => (x, y, physical_scale),
        };
        if !x.is_finite() || !y.is_finite() || !scale.is_finite() || scale <= 0.0 {
            return Err(Invalid::Coordinates);
        }
        let logical = (x * scale / pixels_per_point, y * scale / pixels_per_point);
        if !logical.0.is_finite() || !logical.1.is_finite() {
            return Err(Invalid::Coordinates);
        }
        raw.logical_position = Some(logical);
        Ok(raw)
    }
}
