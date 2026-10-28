//! Explicit PaintCraft capped-dab dynamics over the existing media kernel.
//! Maximum union is a stroke opacity cap, not accumulating per-stamp flow.
use super::{BrushMedia, MaskSpan, Rect, Result, SharedBudget};

/// All attempted dabs count, including invisible pressure/opacity samples.
pub const DABS_MAX: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attributes {
    /// Q16 fractions of captured diameter and captured color alpha.
    pub width: u16,
    pub opacity: u16,
    /// Nib rotation relative to its historical orientation, in 1/256 degrees.
    /// Positive rotation follows positive document Y (visually clockwise).
    pub rotation_q8: i32,
}
impl Default for Attributes {
    fn default() -> Self {
        Self {
            width: 65535,
            opacity: 65535,
            rotation_q8: 0,
        }
    }
}
impl Attributes {
    pub(super) fn valid(self) -> bool {
        (-46080..=46080).contains(&self.rotation_q8)
    }
    pub(super) fn rotation(self) -> Option<(f64, f64)> {
        if self.rotation_q8 == 0 {
            return None;
        }
        let angle = (f64::from(self.rotation_q8) / 256.0).to_radians();
        Some(angle.sin_cos())
    }
    pub(super) fn interpolate(self, next: Self, t: f64) -> Self {
        let scalar =
            |a: u16, b: u16| (f64::from(a) + (f64::from(b) - f64::from(a)) * t).round() as u16;
        // Choose the shortest rotation, with +180 degrees as the fixed tie.
        let delta = (next.rotation_q8 - self.rotation_q8 + 46079).rem_euclid(92160) - 46079;
        let rotation = self.rotation_q8 + (f64::from(delta) * t).round() as i32;
        Self {
            width: scalar(self.width, next.width),
            opacity: scalar(self.opacity, next.opacity),
            rotation_q8: (rotation + 46080).rem_euclid(92160) - 46080,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub at: (i64, i64),
    pub attributes: Attributes,
}

pub const fn supports_orientation(media: BrushMedia) -> bool {
    matches!(
        media,
        BrushMedia::CalligraphyBrush | BrushMedia::CalligraphyPen | BrushMedia::Marker
    )
}

/// Q8 document positions; attributes are linearly interpolated at the existing
/// media's bounded dab positions. Missing axes must be resolved by the captured
/// source policy before this call. The mask scratch is admitted before allocation
/// and its actual capacity is reconciled before writes. The caller must admit
/// and retain emitted spans; an error discards its private prefix.
pub fn visit_checked(
    points: &[Point],
    size: u32,
    media: BrushMedia,
    clip: Rect,
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
    emit: &mut dyn FnMut(MaskSpan) -> Result<()>,
) -> Result<()> {
    check()?;
    if !supports_orientation(media) && points.iter().any(|p| p.attributes.rotation_q8 != 0) {
        return Err(paintcraft_core::types::Error::new(
            paintcraft_core::types::ErrorCode::InputSchema,
            "selected brush does not have an orientable nib",
        ));
    }
    super::visit(
        points.len(),
        &|i| points[i],
        size,
        256,
        media,
        clip,
        Some(budget),
        check,
        emit,
    )
}
