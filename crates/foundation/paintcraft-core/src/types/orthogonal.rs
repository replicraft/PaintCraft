//! Exact raster orientations, using integer pixel centers (no resampling).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OrthogonalTransform {
    RotateRight,
    RotateLeft,
    Rotate180,
    FlipHorizontal,
    FlipVertical,
}
impl OrthogonalTransform {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "rotate-right" => Self::RotateRight,
            "rotate-left" => Self::RotateLeft,
            "rotate-180" => Self::Rotate180,
            "flip-horizontal" => Self::FlipHorizontal,
            "flip-vertical" => Self::FlipVertical,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::RotateRight => "rotate-right",
            Self::RotateLeft => "rotate-left",
            Self::Rotate180 => "rotate-180",
            Self::FlipHorizontal => "flip-horizontal",
            Self::FlipVertical => "flip-vertical",
        }
    }
    pub fn dimensions(self, width: u32, height: u32) -> (u32, u32) {
        match self {
            Self::RotateRight | Self::RotateLeft => (height, width),
            _ => (width, height),
        }
    }
    pub fn inverse(self) -> Self {
        match self {
            Self::RotateRight => Self::RotateLeft,
            Self::RotateLeft => Self::RotateRight,
            other => other,
        }
    }
    /// Source-local pixel to output-local pixel. Checked even for off-canvas
    /// coordinates supplied by typed callers; no floating point or rounding.
    pub fn pixel_checked(self, x: i64, y: i64, width: u32, height: u32) -> Option<(i64, i64)> {
        let (w, h) = (i64::from(width), i64::from(height));
        Some(match self {
            Self::RotateRight => (h.checked_sub(1)?.checked_sub(y)?, x),
            Self::RotateLeft => (y, w.checked_sub(1)?.checked_sub(x)?),
            Self::Rotate180 => (
                w.checked_sub(1)?.checked_sub(x)?,
                h.checked_sub(1)?.checked_sub(y)?,
            ),
            Self::FlipHorizontal => (w.checked_sub(1)?.checked_sub(x)?, y),
            Self::FlipVertical => (x, h.checked_sub(1)?.checked_sub(y)?),
        })
    }
}
