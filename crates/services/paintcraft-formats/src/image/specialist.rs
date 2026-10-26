//! The common RGBA8 facade delegates to already bounded specialist adapters.
use super::*;

pub(super) fn sniff_avif(bytes: &[u8]) -> bool {
    if bytes.get(4..8) != Some(b"ftyp") {
        return false;
    }
    let Some(size) = bytes
        .get(..4)
        .map(|v| u32::from_be_bytes(v.try_into().unwrap()) as usize)
    else {
        return false;
    };
    if size < 16 || size > bytes.len() || !size.is_multiple_of(4) {
        return false;
    }
    matches!(bytes.get(8..12), Some(b"avif" | b"avis"))
        || bytes[16..size]
            .as_chunks::<4>()
            .0
            .iter()
            .any(|brand| matches!(brand, b"avif" | b"avis"))
}
pub(super) fn decode_dds(bytes: &[u8], pixels: u64) -> Result<Decoded> {
    let info = crate::dds::inspect_top_mip_checked(bytes, pixels, &|| Ok(()))?.image;
    if info.transfer != crate::dds::Transfer::Srgb {
        return Err(unsupported(
            "DDS image import requires explicit sRGB transfer; linear or unspecified transfer needs conversion",
        ));
    }
    if !matches!(
        info.alpha,
        crate::dds::SourceAlpha::Straight | crate::dds::SourceAlpha::Opaque
    ) {
        return Err(unsupported(
            "DDS image import requires explicit straight or opaque alpha",
        ));
    }
    let decoded = crate::dds::decode_top_mip_checked(bytes, pixels, &|| Ok(()))?;
    let losses = decoded
        .losses()
        .iter()
        .map(|loss| (*loss).to_owned())
        .collect();
    Ok(Decoded {
        format: ImageFormat::Dds,
        width: info.width,
        height: info.height,
        rgba: decoded.rgba,
        losses,
    })
}
pub(super) fn encode(
    format: ImageFormat,
    width: u32,
    height: u32,
    rgba: &[u8],
    quality: u8,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    match format {
        ImageFormat::Jxl => crate::jxl::encode_rgba8_checked(width, height, rgba, check),
        ImageFormat::Dds => {
            crate::dds::encode_rgba8_checked(width, height, rgba, crate::dds::Transfer::Srgb, check)
        }
        ImageFormat::Avif => {
            Ok(crate::avif::encode_rgba8(width, height, rgba, quality, check)?.bytes)
        }
        _ => Err(unsupported("not a specialist image export")),
    }
}
