//! Same pinned JPEG backend, with explicit source-model and normalization policy.
use super::*;
use zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions};

pub const IDENTITY_PROFILE: &str = "jpeg-device-color-orientation-rgba8-v1";
#[derive(Debug)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub source: Source,
    pub losses: Vec<String>,
    pub normalized_orientation: u8,
    pub identity_profile: &'static str,
}
fn buffer(length: usize, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    if length > isize::MAX as usize {
        return Err(resource("JPEG output exceeds addressable allocation"));
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| resource("JPEG pixel allocation refused"))?;
    while result.len() < length {
        check()?;
        result.extend(std::iter::repeat_n(0, (length - result.len()).min(65_536)));
    }
    Ok(result)
}
/// Materialize an explicitly inspected JPEG once. Original metadata is inventoried
/// from the immutable input; its ancillary payload is skipped by the decoder view.
/// Device-CMYK conversion is an explicit numerical approximation, not ICC color
/// management or a claim that the source was encoded as RGB/sRGB.
pub fn decode(bytes: &[u8], pixels: u64, check: &dyn Fn() -> Result<()>) -> Result<Decoded> {
    decode_with_limits(bytes, Limits::for_pixels(pixels), check)
}
pub fn decode_with_limits(
    bytes: &[u8],
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    check()?;
    let (source, projected) = inspect::projected_copy(bytes, limits, check)?;
    let (width, height) = source.dimensions();
    let raw_cmyk = source.model
        == Model::Cmyk {
            adobe_inverted: false,
        };
    let (output_space, channels) = match source.model {
        Model::Gray => (ColorSpace::Luma, 1),
        Model::Cmyk {
            adobe_inverted: false,
        } => (ColorSpace::CMYK, 4),
        _ => (ColorSpace::RGB, 3),
    };
    let options = DecoderOptions::default()
        .set_strict_mode(false)
        .set_max_width(width as usize)
        .set_max_height(height as usize)
        .jpeg_set_out_colorspace(output_space);
    let mut decoder =
        zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(projected.as_slice()), options);
    check()?;
    decoder
        .decode_headers()
        .map_err(|e| malformed(format!("JPEG header decode failed: {e}")))?;
    check()?;
    let info = decoder
        .info()
        .ok_or_else(|| malformed("JPEG decoder omitted source facts"))?;
    if (u32::from(info.width), u32::from(info.height)) != source.dimensions()
        || info.components != source.component_count
        || info.pixel_density != source.precision()
    {
        return Err(malformed(
            "JPEG decoder source geometry, precision or channels changed",
        ));
    }
    if info.exif_data.is_some()
        || info.xmp_data.is_some()
        || info.iptc_data.is_some()
        || info.extended_xmp.is_some()
        || decoder.icc_profile().is_some()
    {
        return Err(malformed(
            "JPEG decoder view unexpectedly retained omitted ancillary metadata",
        ));
    }
    let header_model = decoder
        .input_colorspace()
        .ok_or_else(|| malformed("JPEG decoder omitted its input model"))?;
    let model_agrees = match source.model {
        Model::Gray => header_model == ColorSpace::Luma,
        Model::Rgb => {
            header_model == ColorSpace::RGB
                || (header_model == ColorSpace::CMYK && source.adobe_transform == Some(0))
        }
        Model::YCbCr => header_model == ColorSpace::YCbCr,
        Model::Cmyk { .. } => header_model == ColorSpace::CMYK,
        Model::Ycck => header_model == ColorSpace::YCCK,
    };
    if !model_agrees {
        return Err(malformed(
            "JPEG decoder input model differs from inspected conventions",
        ));
    }
    let count = u64::from(width) * u64::from(height);
    let raw_length = usize::try_from(
        count
            .checked_mul(channels)
            .ok_or_else(|| resource("JPEG raw size overflows"))?,
    )
    .map_err(|_| resource("JPEG raw buffer is not addressable"))?;
    if decoder.output_buffer_size() != Some(raw_length) {
        return Err(malformed("JPEG decoder raw output length changed"));
    }
    let mut raw = buffer(raw_length, check)?;
    // The selected library's decoding call is opaque. Cancellation is checked
    // before/after it, not claimed as a hard interruption/RSS guarantee.
    check()?;
    decoder
        .decode_into(&mut raw)
        .map_err(|e| malformed(format!("JPEG pixel decode failed: {e}")))?;
    check()?;
    let expected_input = match source.model {
        Model::Gray => ColorSpace::Luma,
        Model::Rgb => ColorSpace::RGB,
        Model::YCbCr => ColorSpace::YCbCr,
        Model::Cmyk { .. } => ColorSpace::CMYK,
        Model::Ycck => ColorSpace::YCCK,
    };
    if decoder.input_colorspace() != Some(expected_input) {
        return Err(malformed(
            "JPEG final decoded input model differs from inspected conventions",
        ));
    }
    if decoder.output_colorspace() != Some(output_space) {
        return Err(malformed("JPEG decoder output model changed"));
    }
    drop(decoder);
    drop(projected);
    let rgba_length = usize::try_from(
        count
            .checked_mul(4)
            .ok_or_else(|| resource("JPEG RGBA size overflows"))?,
    )
    .map_err(|_| resource("JPEG RGBA buffer is not addressable"))?;
    let mut rgba = buffer(rgba_length, check)?;
    for (row, destination) in rgba.chunks_mut(width as usize * 4).enumerate() {
        check()?;
        for (x, pixel) in destination.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if x.is_multiple_of(4096) {
                check()?;
            }
            let at = (row * width as usize + x) * channels as usize;
            if raw_cmyk {
                let k = u32::from(255 - raw[at + 3]);
                for channel in 0..3 {
                    pixel[channel] = ((u32::from(255 - raw[at + channel]) * k + 127) / 255) as u8;
                }
            } else if channels == 1 {
                pixel[..3].fill(raw[at]);
            } else {
                pixel[..3].copy_from_slice(&raw[at..at + 3]);
            }
            pixel[3] = 255;
        }
    }
    drop(raw);
    let oriented = materialize_orientation(&source, width, height, rgba, check)?;
    check()?;
    let losses = source.losses();
    Ok(Decoded {
        width: oriented.width,
        height: oriented.height,
        rgba: oriented.rgba,
        normalized_orientation: oriented.normalized_orientation,
        source,
        losses,
        identity_profile: IDENTITY_PROFILE,
    })
}
