//! Shared codec execution, ONLY for a bounded isolated native process or Wasm
//! Worker. This function supplies no allocator/deadline boundary of its own.
use super::{self as wire, invalid};
use crate::{heif::admission, jxl};
use paintcraft_core::types::{Error, ErrorCode, Result};
fn decoder_error(error: heic_decoder::HeifError) -> Error {
    match error {
        heic_decoder::HeifError::Unsupported(reason) => {
            Error::new(ErrorCode::CapabilityUnsupported, reason)
        }
        heic_decoder::HeifError::LimitExceeded(reason) => {
            Error::new(ErrorCode::ResourceLimit, reason)
        }
        _ => invalid("isolated HEVC frame decoder refused input"),
    }
}
fn plane(samples: &[i32]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.try_reserve_exact(samples.len())
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "native plane allocation"))?;
    for sample in samples {
        out.push(u8::try_from(*sample).map_err(|_| invalid("native sample outside gray8"))?);
    }
    Ok(out)
}
fn decode_heif(header: wire::Header, source: &[u8], peak: &dyn Fn() -> u64) -> Result<Vec<u8>> {
    let admission = admission::admit_gray8(source, u64::from(header.pixels), &|| Ok(()))?;
    let image = heic_decoder::decode_with(
        admission.source(),
        &heic_decoder::DecodeOptions {
            apply_transforms: false,
        },
    )
    .map_err(decoder_error)?;
    let expected = admission.size();
    let pixels = u64::from(expected.0) * u64::from(expected.1);
    if (image.width as u64, image.height as u64) != (u64::from(expected.0), u64::from(expected.1))
        || image.bit_depth_luma != 8
        || image.chroma_format_idc != 0
        || !image.cb.is_empty()
        || !image.cr.is_empty()
        || image.y.len() as u64 != pixels
        || image.alpha.is_some() != admission.alpha()
    {
        return Err(invalid(
            "decoded native geometry/profile differs from admission",
        ));
    }
    let gray = plane(&image.y)?;
    let alpha = image
        .alpha
        .as_ref()
        .map(|alpha| {
            if alpha.bit_depth != 8 || alpha.premultiplied || alpha.samples.len() as u64 != pixels {
                return Err(invalid("decoded native alpha differs from admission"));
            }
            plane(&alpha.samples)
        })
        .transpose()?;
    let encoding = admission.encoding();
    wire::success(
        header,
        wire::Success {
            width: expected.0,
            height: expected.1,
            pixels: wire::Pixels::NativeGray8 {
                gray: &gray,
                alpha: alpha.as_deref(),
                encoding: wire::NativeEncoding {
                    primaries: encoding.primaries,
                    transfer: encoding.transfer,
                    matrix: encoding.matrix,
                    full_range: encoding.full_range,
                },
            },
        },
        peak(),
    )
}
pub fn execute(header: wire::Header, source: &[u8], peak: &dyn Fn() -> u64) -> Result<Vec<u8>> {
    header.verify_source(source)?;
    match header.codec {
        wire::Codec::HeifGray8 => decode_heif(header, source, peak),
        wire::Codec::JxrRgb8 => Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "JXR requires the native private-source worker",
        )),
        wire::Codec::JxlRgba8 => {
            let image =
                jxl::decode_worker_rgba8_checked(source, u64::from(header.pixels), &|| Ok(()))?;
            let jxl::Samples::Rgba8(rgba) = image.samples else {
                return Err(invalid("worker JXL output precision changed"));
            };
            if !image.losses.is_empty() {
                return Err(invalid("worker JXL output would discard metadata"));
            }
            wire::success(
                header,
                wire::Success {
                    width: image.width,
                    height: image.height,
                    pixels: wire::Pixels::SrgbRgba8(&rgba),
                },
                peak(),
            )
        }
    }
}
