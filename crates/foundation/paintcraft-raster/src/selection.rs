use paintcraft_core::types::Rect;

/// The session's selection projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    /// No restriction on editing.
    Inactive,
    /// An active selection that covers nothing.
    Empty,
    /// Binary coverage over a half-open document rectangle.
    Rect(Rect),
    /// Immutable finite binary coverage; bounds alone never authorize painting.
    Mask(BinaryMask),
}

mod mask;
pub use mask::{
    BOUNDS_CACHE_WORK, BinaryMask, BoundarySegment, BoundsCache, MaskLimits, MaskOutline,
    OutlineLimits,
};
mod outline_cache;
pub use outline_cache::OutlineCache;

impl Selection {
    /// Exact finite field comparison for selection-only publication. Identity is
    /// a fast path; independently built masks use a bounded cancellable comparison.
    pub fn same_field_checked(
        &self,
        other: &Self,
        work: u64,
        check: &dyn Fn() -> paintcraft_core::types::Result<()>,
    ) -> paintcraft_core::types::Result<bool> {
        check()?;
        match (self, other) {
            (Self::Mask(a), Self::Mask(b)) => a.same_coverage_checked(b, work, check),
            (Self::Mask(a), Self::Rect(b)) | (Self::Rect(b), Self::Mask(a)) => {
                a.same_coverage_checked(&BinaryMask::uniform(*b, true)?, work, check)
            }
            _ => Ok(self == other),
        }
    }
}
