//! Scalar reference evaluation on straight encoded-RGB bytes, not linear light.
//! No document publication, allocation, GPU execution or editable stack is implied.
//! Independently implemented arithmetic; the Palette design/source review is
//! recorded in docs/architecture/PALETTE_INTEGRATION.md (no source copied here).
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba, filters::Descriptor};

mod coverage;
mod exact;
pub mod exposure;
pub mod levels;
mod point;
pub mod spatial;
pub use coverage::mix_point_coverage;

/// Apply ordered point descriptors. Every stage rounds independently and preserves
/// alpha, including RGB hidden under alpha zero. This convenience function is
/// synchronous and has no cancellation checkpoints; use evaluate_row for work.
/// Spatial descriptors, including radius-zero Box, and prepared-only Levels return typed unsupported.
pub fn evaluate_pixel(filters: &[Descriptor], mut pixel: Rgba) -> Result<Rgba> {
    for filter in filters {
        pixel = point::evaluate(*filter, pixel)?;
    }
    Ok(pixel)
}

/// Evaluate into caller-owned private storage, with no image-sized allocation.
/// Length mismatch/cancellation before work leaves output intact. Cancellation
/// during work may leave a partial row: callers must discard it, never publish
/// it. Checks bound both row and stack work: before each 64-pixel chunk and
/// each descriptor within that chunk, plus initial/final checks. Safe Rust
/// requires source/output disjoint borrows; unsafe-created aliases are unsupported.
/// The entire descriptor list is checked before callbacks or writes. A spatial
/// descriptor never evaluates a partial point prefix into the output.
pub fn evaluate_row(
    filters: &[Descriptor],
    source: &[Rgba],
    output: &mut [Rgba],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    if source.len() != output.len() {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Filter row lengths differ",
        ));
    }
    for filter in filters {
        point::require_point(*filter)?;
    }
    check()?;
    for (source, output) in source.chunks(64).zip(output.chunks_mut(64)) {
        check()?;
        output.copy_from_slice(source);
        for filter in filters {
            check()?;
            for pixel in output.iter_mut() {
                *pixel = point::evaluate(*filter, *pixel)?;
            }
        }
    }
    check()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod point_oracles;
#[cfg(test)]
mod spatial_boundary_tests;

#[cfg(test)]
mod catalog_boundary_tests;
