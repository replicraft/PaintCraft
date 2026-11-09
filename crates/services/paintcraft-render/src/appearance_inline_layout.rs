//! Public pure defining-type guard for the first finite rooted-frame consumer.
use crate::appearance::PointProgram;
use paintcraft_core::types::{Error, Result, Rgba, coordinates::SourcePixel};
use std::mem::size_of;

const fn point_inline_layout_supported() -> bool {
    matches!(size_of::<usize>(), 4 | 8)
        && size_of::<PointProgram<'static>>() <= 32
        && size_of::<SourcePixel>() <= 8
        && size_of::<Rgba>() <= 4
        && size_of::<Error>() <= 96
        && size_of::<Result<PointProgram<'static>>>() <= 112
        && size_of::<Result<SourcePixel>>() <= 112
        && size_of::<Result<Rgba>>() <= 112
}

/// Check the actual defining point, mask-query and spatial inline types against
/// the frozen finite-frame upper bounds without constructing any such values.
/// The caller prepays its 64-unit layout phase before calling appearance helpers.
/// False requires refusal; it never authorizes a new scope, grant or fallback.
/// This is not an ownership, capacity, arbitrary-diagnostic heap or RSS proof.
/// Existing qualified effect kernels retain their source-action work model.
pub const fn rooted_frame_inline_layout_supported() -> bool {
    point_inline_layout_supported()
        && paintcraft_raster::coverage_mask::uniform_region_inline_layout_supported()
        && crate::spatial::rooted_frame_inline_layout_supported()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_point_types_fit_the_frozen_complete_result_bounds() {
        assert!(point_inline_layout_supported());
        assert!(size_of::<PointProgram<'static>>() <= 32);
        assert!(size_of::<Result<PointProgram<'static>>>() <= 112);
        assert!(size_of::<Result<SourcePixel>>() <= 112);
        assert!(size_of::<Result<Rgba>>() <= 112);
    }

    #[test]
    fn public_guard_joins_the_actual_owning_module_guards() {
        assert!(rooted_frame_inline_layout_supported());
        assert!(paintcraft_raster::coverage_mask::uniform_region_inline_layout_supported());
        assert!(crate::spatial::rooted_frame_inline_layout_supported());
    }
}
