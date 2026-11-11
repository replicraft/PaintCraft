//! Closed scalar geometry shared by actual RGBA/binary/A8 materializers.
use paintcraft_core::types::{Rect, Result, rotation::Rotation, stretch_skew::StretchSkew};
#[derive(Clone, Copy, Debug)]
pub(crate) enum Geometry {
    StretchSkew(StretchSkew),
    Rotation(Rotation),
}
impl Geometry {
    pub(crate) fn source(self) -> Rect {
        match self {
            Self::StretchSkew(g) => g.source(),
            Self::Rotation(g) => g.source(),
        }
    }
    pub(crate) fn bounds(self, r: Rect) -> Result<Rect> {
        match self {
            Self::StretchSkew(g) => g.bounds(r),
            Self::Rotation(g) => g.bounds(r),
        }
    }
    pub(crate) fn source_pixel(self, x: i64, y: i64) -> Result<(i64, i64)> {
        match self {
            Self::StretchSkew(g) => g.source_pixel(x, y),
            Self::Rotation(g) => g.source_pixel(x, y),
        }
    }
}
