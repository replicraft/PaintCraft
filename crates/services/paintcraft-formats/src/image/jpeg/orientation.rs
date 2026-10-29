use super::*;

/// JPEG decoding leaves source orientation unapplied. This consumes that raw
/// buffer, permutes whole straight RGBA samples once and emits normalized pixels.
/// The original EXIF transform is provenance, never a second display transform.
pub fn materialize_orientation(
    source: &Source,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    check: &dyn Fn() -> Result<()>,
) -> Result<OrientedRgba> {
    check()?;
    if (width, height) != source.dimensions() {
        return Err(malformed("JPEG raw decoder geometry changed"));
    }
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| resource("JPEG orientation byte count overflows"))?;
    if rgba.len() as u64 != bytes {
        return Err(malformed(
            "JPEG raw decoder geometry or RGBA length changed",
        ));
    }
    if bytes > isize::MAX as u64 {
        return Err(resource(
            "JPEG orientation buffer exceeds addressable allocation",
        ));
    }
    let (out_width, out_height) = source.oriented_dimensions();
    let result = if source.orientation == 1 {
        rgba
    } else {
        let mut output = Vec::new();
        output
            .try_reserve_exact(rgba.len())
            .map_err(|_| resource("JPEG orientation allocation refused"))?;
        for chunk in rgba.chunks(65_536) {
            check()?;
            output.extend(std::iter::repeat_n(0, chunk.len()));
        }
        let (w, h, ow) = (width as usize, height as usize, out_width as usize);
        for y in 0..h {
            check()?;
            for x in 0..w {
                if x % 4096 == 0 {
                    check()?;
                }
                let (dx, dy) = match source.orientation {
                    2 => (w - 1 - x, y),
                    3 => (w - 1 - x, h - 1 - y),
                    4 => (x, h - 1 - y),
                    5 => (y, x),
                    6 => (h - 1 - y, x),
                    7 => (h - 1 - y, w - 1 - x),
                    8 => (y, w - 1 - x),
                    _ => return Err(malformed("JPEG orientation is outside 1 through 8")),
                };
                let from = (y * w + x) * 4;
                let to = (dy * ow + dx) * 4;
                output[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
            }
        }
        output
    };
    check()?;
    Ok(OrientedRgba {
        width: out_width,
        height: out_height,
        rgba: result,
        original_orientation: source.orientation,
        normalized_orientation: 1,
        normalization_policy: NORMALIZATION_POLICY,
    })
}
