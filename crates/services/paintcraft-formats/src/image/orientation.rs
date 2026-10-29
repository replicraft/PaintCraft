//! Checked whole-pixel permutations shared by source-specific orientation policies.
use paintcraft_core::types::{Error, ErrorCode, Result};

pub(crate) fn normalize(
    width: u32,
    height: u32,
    orientation: u8,
    rgba: Vec<u8>,
    check: &dyn Fn() -> Result<()>,
) -> Result<(u32, u32, Vec<u8>)> {
    check()?;
    if !(1..=8).contains(&orientation) {
        return Err(Error::new(
            ErrorCode::CodecFailed,
            "Invalid raster orientation",
        ));
    }
    super::check_size(u64::from(width), u64::from(height))?;
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "Raster orientation extent"))?;
    if bytes != rgba.len() as u64 {
        return Err(Error::new(
            ErrorCode::CodecFailed,
            "Raster orientation sample extent",
        ));
    }
    super::admit_buffer_bytes(bytes, isize::MAX as u64)?;
    let (out_width, out_height) = if orientation >= 5 {
        (height, width)
    } else {
        (width, height)
    };
    let result = if orientation == 1 {
        rgba
    } else {
        let mut output = Vec::new();
        output.try_reserve_exact(rgba.len()).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "Raster orientation allocation refused",
            )
        })?;
        while output.len() < rgba.len() {
            check()?;
            output.resize((output.len() + 65_536).min(rgba.len()), 0);
        }
        let (w, h, ow) = (width as usize, height as usize, out_width as usize);
        for y in 0..h {
            check()?;
            for x in 0..w {
                if x.is_multiple_of(4096) {
                    check()?;
                }
                let (dx, dy) = match orientation {
                    2 => (w - 1 - x, y),
                    3 => (w - 1 - x, h - 1 - y),
                    4 => (x, h - 1 - y),
                    5 => (y, x),
                    6 => (h - 1 - y, x),
                    7 => (h - 1 - y, w - 1 - x),
                    8 => (y, w - 1 - x),
                    _ => unreachable!("validated orientation"),
                };
                let from = (y * w + x) * 4;
                let to = (dy * ow + dx) * 4;
                output[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
            }
        }
        output
    };
    check()?;
    Ok((out_width, out_height, result))
}
