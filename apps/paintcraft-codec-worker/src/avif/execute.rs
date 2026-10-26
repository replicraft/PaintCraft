//! Private AVIF execution through bounded Rust admission and charged static C.
use super::ffi;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{avif, codec_worker as wire};
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
pub(super) fn decode_item(item: &avif::CodedItem<'_>) -> Result<ffi::Decoded> {
    let mut coded = Vec::new();
    coded.try_reserve_exact(item.byte_len).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "Native AVIF coded staging allocation",
        )
    })?;
    for part in &item.parts {
        if coded
            .len()
            .checked_add(part.len())
            .is_none_or(|n| n > item.byte_len)
        {
            return Err(invalid("AVIF coded extent aggregate changed"));
        }
        coded.extend_from_slice(part);
    }
    if coded.len() != item.byte_len {
        return Err(invalid("AVIF coded extent length changed"));
    }
    let decoded = ffi::decode(&coded, item.width, item.height)?;
    drop(coded);
    let actual = decoded.info;
    let sequence = &item.sequence;
    let layout = if sequence.monochrome {
        0
    } else if sequence.subsampling_y {
        1
    } else if sequence.subsampling_x {
        2
    } else {
        3
    };
    if actual.layout != layout
        || actual.depth != u32::from(sequence.bit_depth)
        || actual.profile != u32::from(sequence.profile)
        || actual.level != u32::from(sequence.level)
        || (actual.max_width, actual.max_height) != (sequence.width, sequence.height)
        || actual.primaries != u32::from(sequence.primaries)
        || actual.transfer != u32::from(sequence.transfer)
        || actual.matrix != u32::from(sequence.matrix)
        || actual.full_range != u32::from(sequence.full_range)
        || actual.chroma_position != u32::from(sequence.chroma_position)
        || actual.still_picture != 1
        || actual.reduced_header != 1
    {
        return Err(invalid(
            "Native AV1 output disagrees with admitted AVIF coded profile",
        ));
    }
    Ok(decoded)
}
pub(crate) fn execute(
    header: wire::Header,
    source: &[u8],
    peak: &dyn Fn() -> u64,
) -> Result<Vec<u8>> {
    if header.codec != wire::Codec::AvifSrgb8 {
        return Err(invalid("AVIF execution requires the private code5 request"));
    }
    header.verify_source(source)?;
    let admitted = avif::admit_srgb8(source, u64::from(header.pixels), &|| Ok(()))?;
    let rgb = decode_item(&admitted.primary)?;
    let alpha = admitted.alpha.as_ref().map(decode_item).transpose()?;
    let (width, height) = (admitted.primary.width, admitted.primary.height);
    let count = width as usize * height as usize;
    let mut rgba = vec![0; count * 4];
    for (i, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        // AV1 identity matrix stores G,B,R planes. Preserve coded RGB even
        // at zero alpha; there is no implicit matte or invisible-RGB cleanup.
        *pixel = [
            rgb.planes[count * 2 + i],
            rgb.planes[i],
            rgb.planes[count + i],
            alpha.as_ref().map_or(255, |a| a.planes[i]),
        ];
    }
    drop(rgb);
    drop(alpha);
    wire::success(
        header,
        wire::Success {
            width,
            height,
            pixels: wire::Pixels::SrgbRgba8(&rgba),
        },
        peak(),
    )
}
