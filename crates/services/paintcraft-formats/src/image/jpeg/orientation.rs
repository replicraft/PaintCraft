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
    let (out_width, out_height, result) =
        super::super::orientation::normalize(width, height, source.orientation, rgba, check)?;
    Ok(OrientedRgba {
        width: out_width,
        height: out_height,
        rgba: result,
        original_orientation: source.orientation,
        normalized_orientation: 1,
        normalization_policy: NORMALIZATION_POLICY,
    })
}
