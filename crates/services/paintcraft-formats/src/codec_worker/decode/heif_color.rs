//! Selected HEIF color decode, invoked only within the isolated native worker.
use super::{decoder_error, invalid, wire};
use crate::heif::admission::{ColorAdmitted, admit_srgb8};
use heic_decoder::{ChromaSampling, ColorInfo, DecodedImage};
use paintcraft_core::types::{Error, ErrorCode, Result};
fn validate(image: &DecodedImage, admitted: &ColorAdmitted<'_>) -> Result<()> {
    let (width, height) = admitted.size();
    let chroma = admitted.chroma();
    let (sub_x, sub_y, origin_y) = if chroma == 1 {
        (2, 2, 0.5)
    } else {
        (1, 1, 0.0)
    };
    let cw = (width as usize).div_ceil(sub_x as usize);
    let ch = (height as usize).div_ceil(sub_y as usize);
    let pixels = width as usize * height as usize;
    let encoding = admitted.encoding();
    let expected = ColorInfo {
        color_primaries: encoding.primaries,
        transfer_characteristics: encoding.transfer,
        matrix_coefficients: encoding.matrix,
        full_range: encoding.full_range,
    };
    if image.width != width as usize
        || image.height != height as usize
        || image.bit_depth_luma != 8
        || image.bit_depth_chroma != 8
        || image.chroma_format_idc != chroma
        || image.color_info != Some(expected)
        || image.metadata.color_info != Some(expected)
        || image.metadata.icc_profile.is_some()
        || image.metadata.exif.is_some()
        || image.metadata.xmp.is_some()
        || image.cb_width != cw
        || image.cb_height != ch
        || image.y.len() != pixels
        || image.cb.len() != cw * ch
        || image.cr.len() != cw * ch
        || image.chroma_sampling
            != (ChromaSampling {
                sub_x,
                sub_y,
                origin_x: 0.0,
                origin_y,
            })
        || image.alpha.is_some() != admitted.alpha()
    {
        return Err(invalid(
            "decoded HEIF color declarations differ from admitted source",
        ));
    }
    for plane in [&image.y, &image.cb, &image.cr] {
        if plane.iter().any(|sample| !(0..=255).contains(sample)) {
            return Err(invalid("decoded HEIF color sample exceeds unsigned8"));
        }
    }
    if let Some(alpha) = &image.alpha
        && (alpha.bit_depth != 8
            || alpha.premultiplied
            || alpha.samples.len() != pixels
            || alpha
                .samples
                .iter()
                .any(|sample| !(0..=255).contains(sample)))
    {
        return Err(invalid(
            "decoded HEIF alpha differs from admitted straight8 source",
        ));
    }
    Ok(())
}
pub(super) fn execute(
    header: wire::Header,
    source: &[u8],
    peak: &dyn Fn() -> u64,
) -> Result<Vec<u8>> {
    if cfg!(target_arch = "wasm32") {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "HEIF color worker is native-only",
        ));
    }
    let admitted = admit_srgb8(source, u64::from(header.pixels), &|| Ok(()))?;
    let image = heic_decoder::decode_with(
        admitted.source(),
        &heic_decoder::DecodeOptions {
            apply_transforms: false,
        },
    )
    .map_err(decoder_error)?;
    validate(&image, &admitted)?;
    let rgba = image.to_rgba8().map_err(decoder_error)?;
    let (width, height) = admitted.size();
    if rgba.width != width as usize
        || rgba.height != height as usize
        || rgba.premultiplied
        || rgba.data.len() != width as usize * height as usize * 4
    {
        return Err(invalid(
            "converted HEIF RGBA extent differs from admitted source",
        ));
    }
    wire::success(
        header,
        wire::Success {
            width,
            height,
            pixels: wire::Pixels::SrgbRgba8(&rgba.data),
        },
        peak(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &[u8] =
        include_bytes!("../../../tests/heif-color-fixtures/yuv420-full-alpha.heic");
    #[test]
    fn actual_decoded_profile_cannot_change_geometry_color_planes_or_alpha() {
        let admitted = admit_srgb8(SOURCE, wire::PIXELS_MAX, &|| Ok(())).unwrap();
        for mutation in 0..12 {
            let mut image = heic_decoder::decode_with(
                SOURCE,
                &heic_decoder::DecodeOptions {
                    apply_transforms: false,
                },
            )
            .unwrap();
            validate(&image, &admitted).unwrap();
            match mutation {
                0 => image.width += 1,
                1 => image.bit_depth_luma = 10,
                2 => image.color_info = None,
                3 => image.metadata.color_info = None,
                4 => image.cb.pop().map(|_| ()).unwrap(),
                5 => image.cr[0] = 256,
                6 => image.chroma_sampling.origin_x = 0.5,
                7 => image.alpha = None,
                8 => image.alpha.as_mut().unwrap().premultiplied = true,
                9 => image.alpha.as_mut().unwrap().samples[0] = -1,
                10 => image.metadata.icc_profile = Some(vec![0]),
                _ => image.metadata.exif = Some(vec![0]),
            }
            assert!(validate(&image, &admitted).is_err(), "mutation{mutation}");
        }
    }
}
