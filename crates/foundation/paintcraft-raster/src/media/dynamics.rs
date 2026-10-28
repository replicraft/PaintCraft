//! Explicit PaintCraft capped-dab dynamics over the existing media kernel.
//! Maximum union is a stroke opacity cap, not accumulating per-stamp flow.
use super::{BrushMedia, MaskSpan, Rect, Result, SharedBudget};

/// Independent traversal allowance: dabs use constant scratch, not one retained
/// record per dab. All attempts count, including invisible samples. Dense mask
/// storage and pixel work remain separately capped at 4Mi pixels / 64Mi visits.
pub const DABS_MAX: usize = 1_048_576;

pub(super) fn spacing(size: u32, media: BrushMedia, before: Attributes, after: Attributes) -> f64 {
    let width = f64::from(before.width.min(after.width)) / 65535.0;
    let (divisor, minimum, maximum) = match media {
        BrushMedia::CalligraphyPen => (32.0, 0.25, 4.0),
        BrushMedia::CalligraphyBrush => (20.0, 0.35, 6.0),
        _ => (8.0, 0.5, 8.0),
    };
    // The smaller endpoint bounds every linearly interpolated width. The nib's
    // historical divisor accounts for its minor axis. Scale the old minimum too
    // so a narrow tip cannot retain full-size gaps. A 1/16px floor bounds work
    // below pixel resolution; budgets refuse excessive traversal explicitly.
    // Full width reproduces the exact historical expression and dab phase.
    (f64::from(size) * width / divisor).clamp((minimum * width).max(1.0 / 16.0), maximum)
}

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

/// Q8 document positions; attributes are linearly interpolated at width-aware
/// bounded dab positions. Missing axes must be resolved by the captured
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
