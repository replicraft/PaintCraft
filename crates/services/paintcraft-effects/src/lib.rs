//! Scalar reference evaluation on straight encoded-RGB bytes, not linear light.
//! No document publication, allocation, GPU execution or editable stack is implied.
//! Independently implemented arithmetic; the Palette design/source review is
//! recorded in docs/architecture/PALETTE_INTEGRATION.md (no source copied here).
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba, filters::Descriptor};

/// Apply ordered descriptors. Every stage rounds independently and preserves
/// alpha, including RGB hidden under alpha zero. This convenience function is
/// synchronous and has no cancellation checkpoints; use evaluate_row for work.
pub fn evaluate_pixel(filters: &[Descriptor], mut pixel: Rgba) -> Rgba {
    for filter in filters {
        for channel in &mut pixel.0[..3] {
            *channel = match *filter {
                Descriptor::InvertRgbV1 => 255 - *channel,
                Descriptor::BrightnessContrastV1(parameters) => {
                    let b = i64::from(parameters.brightness());
                    let c = i64::from(parameters.contrast());
                    let v = i64::from(*channel);
                    // Exact rational form of (v-127.5)*(100+c)/(100-c)
                    // +127.5+255*b/100. All validated domains fit i64.
                    let denominator = 200 * (100 - c);
                    let numerator = (2 * v - 255) * 100 * (100 + c)
                        + 255 * 100 * (100 - c)
                        + 510 * b * (100 - c);
                    let clamped = numerator.clamp(0, 255 * denominator);
                    ((clamped + denominator / 2) / denominator) as u8
                }
            };
        }
    }
    pixel
}

/// Evaluate into caller-owned private storage, with no image-sized allocation.
/// Length mismatch/cancellation before work leaves output intact. Cancellation
/// during work may leave a partial row: callers must discard it, never publish
/// it. Checks bound both row and stack work: before each 64-pixel chunk and
/// each descriptor within that chunk, plus initial/final checks. Safe Rust
/// requires source/output disjoint borrows; unsafe-created aliases are unsupported.
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
    check()?;
    for (source, output) in source.chunks(64).zip(output.chunks_mut(64)) {
        check()?;
        output.copy_from_slice(source);
        for filter in filters {
            check()?;
            for pixel in output.iter_mut() {
                *pixel = evaluate_pixel(std::slice::from_ref(filter), *pixel);
            }
        }
    }
    check()
}

#[cfg(test)]
mod tests;
