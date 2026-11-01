//! Exact encoded-sRGB composition of one straight RGBA8 source and backdrop.
use super::Rgba;

/// Named layer composition policy, separate from raster paint/replace operations.
/// This standalone numerical type does not register document or transport support.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LayerBlendMode {
    #[default]
    Normal = 0,
    Multiply = 1,
    Screen = 2,
}

impl LayerBlendMode {
    /// Compose already-evaluated source over the actual lower-layer backdrop.
    /// Callers apply filter-prefix, own-mask alpha rounding and layer-opacity
    /// alpha rounding BEFORE this boundary; this function does not repeat them.
    /// Each invocation writes one straight RGBA8 result, including its alpha.
    pub fn composite(self, source: Rgba, backdrop: Rgba) -> Rgba {
        if self == Self::Normal {
            return source.source_over(backdrop);
        }
        let a = u32::from(source.a());
        if a == 0 {
            // Preserve the existing zero-source-alpha/hidden-backdrop policy.
            return backdrop;
        }
        let b = u32::from(backdrop.a());
        let inverse_a = 255 - a;
        let inverse_b = 255 - b;
        let total = a * 255 + b * inverse_a;
        let denominator = total * 255;
        let overlap = a * b;
        let mut out = [0; 4];
        for (channel, result) in out.iter_mut().take(3).enumerate() {
            let s = u32::from(source.0[channel]);
            let d = u32::from(backdrop.0[channel]);
            // These are 255 times the unrounded blended byte. Keeping this
            // fractional contribution avoids an extra RGB quantization boundary.
            let mixed = match self {
                Self::Normal => s * 255,
                Self::Multiply => d * s,
                Self::Screen => 65_025 - (255 - d) * (255 - s),
            };
            let uncovered = d * b * inverse_a + s * a * inverse_b;
            let numerator = uncovered * 255 + overlap * mixed;
            // For byte inputs: total <= 65025, mixed <= 65025, and nonnegative
            // source-over weights imply numerator <= 255*denominator <= 255^4.
            // Adding denominator/2 stays <=4_236_541_312, strictly below u32::MAX.
            // The quotient is within 0..255. No float, heap, saturation or early
            // channel rounding is used; denominator>0 because a>0.
            *result = ((numerator + denominator / 2) / denominator) as u8;
        }
        out[3] = ((total + 127) / 255) as u8;
        Rgba(out)
    }
}

#[cfg(test)]
mod tests;
