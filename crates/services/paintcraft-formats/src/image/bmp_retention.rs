//! New-document BMP copies retain original ICC/density without activating them.
use super::{ImageFormat, bmp_metadata, codec};
use paintcraft_core::types::Result;
use paintcraft_document::{
    RasterConversionContext, RasterConversionKey, RasterSelection, SourceMetadata,
};
use std::io::Cursor;
mod capture;
pub use super::retained_raster::{ICC_RETAINED_UNAPPLIED, Prepared, RESOLUTION_RETAINED_INACTIVE};

pub const IDENTITY_PROFILE: &str = "bmp-source-profile-resolution-rgba8-v1";

/// Proposed acknowledgements only. This cannot admit resources, retain bytes,
/// authorize adoption or turn calibrated RGB, gamma or palette indices editable.
pub fn required_acknowledgements(
    source: &bmp_metadata::Descriptor,
    mut losses: Vec<String>,
) -> Vec<String> {
    let density = source.resolution_raw != [0, 0];
    for code in &mut losses {
        if code == "icc-profile-ignored" && source.icc.is_some() {
            *code = ICC_RETAINED_UNAPPLIED.to_owned();
        }
        if code == "resolution-metadata-ignored" && density {
            *code = RESOLUTION_RETAINED_INACTIVE.to_owned();
        }
    }
    for (needed, code) in [
        (source.icc.is_some(), ICC_RETAINED_UNAPPLIED),
        (density, RESOLUTION_RETAINED_INACTIVE),
    ] {
        if needed && !losses.iter().any(|s| s == code) {
            losses.push(code.to_owned());
        }
    }
    losses
}

/// A distinct new-document profile. Legacy image decoding, nested assets and
/// layer insertion keep their existing omission route and admission allowance.
/// The host supplies its existing metadata realm; no new private budget exists.
pub fn decode(
    bytes: &[u8],
    pixels: u64,
    metadata: crate::pcnd::MetadataAdmission<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Prepared> {
    check()?;
    let source = bmp_metadata::inspect(bytes, pixels, check)?;
    let attachment = capture::source_metadata(bytes, &source, metadata, check)?;
    check()?;
    let mut image = if super::bmp_rle::is_rle(bytes) {
        super::bmp_rle::decode(bytes, pixels, check)?
    } else {
        // Preserve the same strict index/mask profile and selected image decoder.
        // Do not fetch a second allocated ICC through ImageDecoder's accessor.
        super::admit_bmp(bytes, pixels)?;
        let mut reader =
            ::image::ImageReader::with_format(Cursor::new(bytes), ::image::ImageFormat::Bmp);
        reader.limits(super::decode_limits(pixels));
        let decoder = reader.into_decoder().map_err(super::image_error)?;
        check()?;
        let image = super::decode_pixels_metadata(
            decoder,
            ImageFormat::Bmp,
            source.losses().into_iter().map(str::to_owned).collect(),
            pixels,
            false,
        )?;
        // This opaque decoder call is not internally preemptible.
        check()?;
        image
    };
    if image.format != ImageFormat::Bmp
        || (image.width, image.height) != (source.width, source.height)
    {
        return Err(codec("Retained BMP geometry differs from inspected source"));
    }
    image.losses = required_acknowledgements(&source, image.losses);
    let document_key = RasterConversionKey::checked(
        RasterConversionContext {
            source_sha256: source.source.sha256,
            profile: IDENTITY_PROFILE,
            selection: RasterSelection::Image,
            assessment_sha256: attachment.as_ref().map(SourceMetadata::digest),
        },
        image.width,
        image.height,
        &image.rgba,
        pixels,
        check,
    )?;
    check()?;
    Ok(Prepared {
        image,
        attachment,
        document_key,
    })
}

#[cfg(test)]
mod tests;
