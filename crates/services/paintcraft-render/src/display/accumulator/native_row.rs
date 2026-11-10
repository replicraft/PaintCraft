//! Bounded preflight followed by infallible private sum updates.
use super::*;
use std::mem::size_of;

const CONTROL_BYTES: usize = 512;
const fn inline_control_bytes() -> usize {
    4 * size_of::<&[u8]>()
        + 2 * size_of::<Result<()>>()
        + size_of::<Error>()
        + size_of::<[u64; 8]>()
        + size_of::<[usize; 8]>()
        + size_of::<[u32; 8]>()
}
const fn layout_supported() -> bool {
    size_of::<DisplayAccumulator>() == 40
        && size_of::<Result<()>>() <= 112
        && size_of::<Error>() <= 96
        && inline_control_bytes() <= CONTROL_BYTES
}
fn geometry_error() -> Error {
    Error::new(
        ErrorCode::InputSchema,
        "Display native row geometry is invalid",
    )
}

impl DisplayAccumulator {
    /// Add one already-composed native row to its exact touched display cells.
    ///
    /// `rgba` contains1..=64 pixels. `step` is a power of two no larger than
    ///2^20; `offset < step` locates the first pixel within the first cell.
    /// `dest` contains exactly `(offset + pixels - 1) / step + 1` cells.
    /// Every destination count is checked before any accumulator is changed.
    /// Successful calls allocate nothing and retain exact unrounded sums.
    ///
    /// # Errors
    /// Returns InputSchema for malformed geometry and ResourceLimit for a
    /// destination sample overflow or unsupported inline control layout.
    /// On every error, all destination counts and sums remain unchanged.
    pub fn add_native_row(dest: &mut [Self], rgba: &[u8], step: u32, offset: u32) -> Result<()> {
        if !layout_supported() {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Display native row controls",
            ));
        }
        if rgba.is_empty()
            || rgba.len() > 256
            || !rgba.len().is_multiple_of(4)
            || !step.is_power_of_two()
            || step > (1 << 20)
            || offset >= step
        {
            return Err(geometry_error());
        }
        let pixels = rgba.len() / 4;
        let step = step as usize;
        let offset = offset as usize;
        // These additions fit even on wasm32: offset<=2^20-1, pixels<=64.
        if dest.len() != (offset + pixels - 1) / step + 1 {
            return Err(geometry_error());
        }
        let mut remaining = pixels;
        let mut position = offset;
        let mut cell = 0;
        while remaining != 0 {
            let count = remaining.min(step - position);
            if dest[cell].samples > RENDER_PIXELS_MAX - count as u64 {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "Display sample accumulation exceeds its bound",
                ));
            }
            remaining -= count;
            position = 0;
            cell += 1;
        }
        // Safe constructors/additions preserve sumRGB<=65025*samples and
        // sumA<=255*samples. At67108864 samples these are far below u64::MAX.
        // The preflight above proves every count increment before this pass.
        let mut pixel = 0;
        let mut position = offset;
        let mut cell = 0;
        while pixel < pixels {
            let i = pixel * 4;
            let alpha = u64::from(rgba[i + 3]);
            let value = &mut dest[cell];
            value.sum.0[0] += u64::from(rgba[i]) * alpha;
            value.sum.0[1] += u64::from(rgba[i + 1]) * alpha;
            value.sum.0[2] += u64::from(rgba[i + 2]) * alpha;
            value.sum.0[3] += alpha;
            value.samples += 1;
            position += 1;
            if position == step {
                position = 0;
                cell += 1;
            }
            pixel += 1;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "native_row/tests.rs"]
mod tests;
