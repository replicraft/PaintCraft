//! Explicit raw gray+straight-alpha mapping for TIFF0.11's Multiband result.
//! Associated alpha remains refused. WhiteIsZero inverts gray only, never alpha.
use super::*;
use ::tiff::{
    ColorType,
    decoder::{Decoder, DecodingResult, Limits},
};

pub(super) fn decode(
    bytes: &[u8],
    page: &Page,
    mut losses: Vec<String>,
    check: &dyn Fn() -> Result<()>,
) -> Result<crate::image::Decoded> {
    let d = page.descriptor;
    let (reader, metadata) = view::Selected::gray_alpha(bytes, d, check)?;
    let result = Decoder::new(reader);
    metadata.set(false);
    check()?;
    let mut limits = Limits::default();
    limits.decoding_buffer_size = (PIXELS_MAX * 8) as usize;
    limits.ifd_value_size = METADATA_PER_PAGE_MAX;
    limits.intermediate_buffer_size = (PIXELS_MAX * 8) as usize;
    let mut decoder = result.map_err(error)?.with_limits(limits);
    if decoder.dimensions().map_err(error)? != (page.width, page.height)
        || decoder.colortype().map_err(error)?
            != (ColorType::Multiband {
                bit_depth: d.bits_per_sample.values[0] as u8,
                num_samples: 2,
            })
    {
        return Err(malformed(
            "TIFF raw grayscale-alpha interpretation differs from admission",
        ));
    }
    let result = decoder.read_image();
    check()?;
    let raw = result.map_err(error)?;
    let pixels = u64::from(page.width) * u64::from(page.height);
    let mut rgba = Vec::new();
    rgba.try_reserve_exact((pixels * 4) as usize)
        .map_err(|_| resource("TIFF grayscale-alpha output allocation"))?;
    match raw {
        DecodingResult::U8(values)
            if d.bits_per_sample.values[0] == 8 && values.len() as u64 == pixels * 2 =>
        {
            for (index, [gray, alpha]) in values.as_chunks::<2>().0.iter().enumerate() {
                if index.is_multiple_of(page.width as usize) {
                    check()?;
                }
                let gray = if d.photometric == 0 {
                    255 - gray
                } else {
                    *gray
                };
                rgba.extend_from_slice(&[gray, gray, gray, *alpha]);
            }
        }
        DecodingResult::U16(values)
            if d.bits_per_sample.values[0] == 16 && values.len() as u64 == pixels * 2 =>
        {
            losses.insert(0, "depth-16-reduced-to-8".into());
            for (index, [gray, alpha]) in values.as_chunks::<2>().0.iter().enumerate() {
                if index.is_multiple_of(page.width as usize) {
                    check()?;
                }
                let gray = if d.photometric == 0 {
                    u16::MAX - gray
                } else {
                    *gray
                };
                let gray = (gray >> 8) as u8;
                rgba.extend_from_slice(&[gray, gray, gray, (alpha >> 8) as u8]);
            }
        }
        _ => {
            return Err(malformed(
                "TIFF raw grayscale-alpha sample type/length differs",
            ));
        }
    }
    check()?;
    Ok(crate::image::Decoded {
        format: crate::image::ImageFormat::Tiff,
        width: page.width,
        height: page.height,
        rgba,
        losses,
    })
}
fn error(error: ::tiff::TiffError) -> Error {
    let code = match error {
        ::tiff::TiffError::LimitsExceeded => ErrorCode::ResourceLimit,
        ::tiff::TiffError::UnsupportedError(_) => ErrorCode::CapabilityUnsupported,
        _ => ErrorCode::CodecFailed,
    };
    Error::new(code, format!("TIFF raw grayscale-alpha decoder: {error}"))
}
