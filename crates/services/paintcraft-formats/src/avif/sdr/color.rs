//! Shared existing HEIF matrix/range/bilinear conversion, with explicit AV1 origins.
//! Invoke only under the isolated helper's charged heap/deadline. No C decoding.
use super::*;
use heic_decoder::{AlphaPlane, ChromaSampling, ColorInfo, DecodedImage};
use paintcraft_core::types::{Error, ErrorCode};
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn samples(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<Vec<i32>> {
    check()?;
    let mut output = Vec::new();
    output.try_reserve_exact(bytes.len()).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "AVIF color sample staging allocation",
        )
    })?;
    for chunk in bytes.chunks(4096) {
        check()?;
        output.extend(chunk.iter().copied().map(i32::from));
    }
    Ok(output)
}
/// Reconstruct an already admitted SDR candidate from decoder-verified native
/// planes in three full-area slots and optional full-area straight alpha.
///
/// The same selected converter used by HEIF applies explicit matrix/range,
/// bilinear interpolation, outermost-sample extension and RGB8 rounding/clipping.
/// AV1 CSP_VERTICAL equals HEIF location 0 geometrically; CSP_COLOCATED equals
/// HEIF location 2. UNKNOWN never gets an inferred origin. Calls are cancellable
/// around staging and conversion, not inside the selected conversion routine.
pub fn reconstruct(
    admitted: &Admitted<'_>,
    source_bytes: u64,
    planes: &[u8],
    alpha: Option<&[u8]>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    super::work_layout(admitted, source_bytes)?;
    let primary = &admitted.primary;
    let sequence = &primary.sequence;
    let (width, height) = (primary.width as usize, primary.height as usize);
    let count = width
        .checked_mul(height)
        .ok_or_else(|| invalid("AVIF color geometry overflow"))?;
    if width == 0
        || height == 0
        || width > 65536
        || height > 65536
        || count as u64 > crate::avif::INSPECT_PIXELS_MAX
    {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "AVIF color geometry exceeds allowance",
        ));
    }
    let (sub, origin_y) = match (
        sequence.profile,
        sequence.subsampling_x,
        sequence.subsampling_y,
        sequence.chroma_position,
    ) {
        (1, false, false, 0) => (1, 0.0),
        (0, true, true, 1) => (2, 0.5),
        (0, true, true, 2) => (2, 0.0),
        _ => {
            return Err(invalid(
                "AVIF color sampling does not match the admitted SDR candidate",
            ));
        }
    };
    if sequence.monochrome
        || sequence.bit_depth != 8
        || (sequence.primaries, sequence.transfer, sequence.matrix) != (1, 13, 1)
        || (sequence.width, sequence.height) != (primary.width, primary.height)
        || planes.len() != count * 3
        || alpha.is_some() != admitted.alpha.is_some()
        || alpha.is_some_and(|values| values.len() != count)
    {
        return Err(invalid("AVIF color plane/profile declarations changed"));
    }
    if let Some(value) = &admitted.alpha {
        let a = &value.sequence;
        if (value.width, value.height) != (primary.width, primary.height)
            || (a.width, a.height) != (primary.width, primary.height)
            || !a.monochrome
            || a.profile != 0
            || a.bit_depth != 8
            || !a.full_range
            || !a.subsampling_x
            || !a.subsampling_y
            || a.chroma_position != 0
            || (a.primaries, a.transfer, a.matrix) != (2, 2, 2)
        {
            return Err(invalid("AVIF color alpha profile declarations changed"));
        }
    }
    let (cw, ch) = (width.div_ceil(sub), height.div_ceil(sub));
    let chroma = cw * ch;
    let image = DecodedImage {
        width,
        height,
        bit_depth_luma: 8,
        bit_depth_chroma: 8,
        chroma_format_idc: if sub == 2 { 1 } else { 3 },
        color_info: Some(ColorInfo {
            color_primaries: 1,
            transfer_characteristics: 13,
            matrix_coefficients: 1,
            full_range: sequence.full_range,
        }),
        chroma_sampling: ChromaSampling {
            sub_x: sub as u32,
            sub_y: sub as u32,
            origin_x: 0.0,
            origin_y,
        },
        y: samples(&planes[..count], check)?,
        cb: samples(&planes[count..count + chroma], check)?,
        cr: samples(&planes[count * 2..count * 2 + chroma], check)?,
        cb_width: cw,
        cb_height: ch,
        alpha: alpha
            .map(|values| {
                samples(values, check).map(|samples| AlphaPlane {
                    bit_depth: 8,
                    samples,
                    premultiplied: false,
                })
            })
            .transpose()?,
        metadata: Default::default(),
    };
    check()?;
    let output = image.to_rgba8().map_err(|error| {
        let code = if matches!(error, heic_decoder::HeifError::LimitExceeded(_)) {
            ErrorCode::ResourceLimit
        } else {
            ErrorCode::CodecFailed
        };
        Error::new(code, error.to_string())
    })?;
    check()?;
    if output.width != width
        || output.height != height
        || output.data.len() != count * 4
        || output.premultiplied
    {
        return Err(invalid(
            "AVIF color converter returned unexpected geometry or alpha semantics",
        ));
    }
    Ok(output.data)
}
