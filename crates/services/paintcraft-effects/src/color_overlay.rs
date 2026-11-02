//! Unpriced standalone style primitive on straight encoded RGB.
//! Fixed bounded scalar stack, caller-owned output; no heap/LUT/float/unsafe
//! in successful arithmetic. Callback/error allocations and scalar stack are
//! outside that claim. No document, registration, admission or runtime grant.
use paintcraft_core::types::styles::{ColorOverlayRgbV1, EffectBlendMode};
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba};

/// Exactly one rational final round per channel; incoming alpha is copied,
/// including hidden RGB. Own masks/opacity/layer blending are later stages.
pub fn evaluate_pixel(policy: ColorOverlayRgbV1, incoming: Rgba) -> Result<Rgba> {
    let mut out = incoming;
    let t = u32::from(policy.opacity());
    for (slot, color) in out.0[..3].iter_mut().zip(policy.color()) {
        let x = u32::from(*slot);
        let c = u32::from(color);
        // b/255 is the exact effect candidate; no byte rounding here.
        let b = match policy.blend() {
            EffectBlendMode::Normal => c * 255,
            EffectBlendMode::Multiply => x * c,
            EffectBlendMode::Screen => (x + c) * 255 - x * c,
        };
        // b<=65025, numerator<=16581375; u32 covers every legal policy.
        let numerator = x * (255 - t) * 255 + b * t;
        *slot = ((numerator + 32512) / 65025) as u8;
    }
    Ok(out)
}

/// Length refusal precedes callbacks/writes; initial, <=64-tuple chunks and
/// final callbacks match the existing pure-effects private-output convention.
/// Empty rows check initial+final. Mid/final cancellation can leave private
/// output partly/fully written; caller must discard it. Source is immutable.
/// No quota/counter/price is invented by this unpriced primitive.
pub fn evaluate_row(
    policy: ColorOverlayRgbV1,
    source: &[Rgba],
    output: &mut [Rgba],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    if source.len() != output.len() {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Color Overlay row lengths differ",
        ));
    }
    check()?;
    for (src, dst) in source.chunks(64).zip(output.chunks_mut(64)) {
        check()?;
        for (incoming, slot) in src.iter().zip(dst) {
            *slot = evaluate_pixel(policy, *incoming)?;
        }
    }
    check()
}

#[cfg(test)]
mod oracles;
#[cfg(test)]
mod tests;
