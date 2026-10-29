//! New-document PNG conversion with original ICC bytes/pHYs retained separately.
pub use super::retained_raster::{ICC_RETAINED_UNAPPLIED, Prepared, RESOLUTION_RETAINED_INACTIVE};
use super::{Decoded, ImageFormat, codec, png_metadata};
use paintcraft_core::types::Result;
use paintcraft_document::{
    RasterConversionContext, RasterConversionKey, RasterSelection, SourceMetadata,
};
mod capture;
mod decoder;
/// The legacy/paste route keeps its original omission acknowledgements and
/// creates no attachment, while sharing the qualified strict-checksum reader.
pub(super) fn decode_legacy_pixels(
    bytes: &[u8],
    source: &png_metadata::Descriptor,
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    decoder::decode(bytes, source, pixels, check)
}

pub const IDENTITY_PROFILE: &str = "png-source-profile-resolution-rgba8-v1";

/// Required notices for the proposed retentive conversion. This does not grant
/// resource admission, retain bytes, acknowledge notices, or adopt a document.
pub fn required_acknowledgements(
    source: &png_metadata::Descriptor,
    mut losses: Vec<String>,
) -> Vec<String> {
    for code in &mut losses {
        if code == "icc-profile-ignored" && source.icc.is_some() {
            *code = ICC_RETAINED_UNAPPLIED.to_owned();
        }
        if code == "resolution-metadata-ignored" && source.resolution.is_some() {
            *code = RESOLUTION_RETAINED_INACTIVE.to_owned();
        }
    }
    for (needed, code) in [
        (source.icc.is_some(), ICC_RETAINED_UNAPPLIED),
        (source.resolution.is_some(), RESOLUTION_RETAINED_INACTIVE),
        (source.icc.is_some(), "png-metadata-ignored"),
    ] {
        if needed && !losses.iter().any(|s| s == code) {
            losses.push(code.to_owned());
        }
    }
    losses
}

/// Capture original ICC bytes under host admission before pixel decoding. The
/// selected pixel reader skips metadata storage; it never activates this ICC.
/// This API is for new documents, not inserting a second source into a document.
pub fn decode(
    bytes: &[u8],
    pixels: u64,
    metadata: crate::pcnd::MetadataAdmission<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Prepared> {
    check()?;
    let source = png_metadata::inspect(bytes, pixels, check)?;
    let attachment = capture::source_metadata(bytes, &source, metadata, check)?;
    check()?;
    let mut image = if source.color_type == 3 {
        super::png_indexed::decode_captured_source(bytes, &source, pixels, check)?
    } else {
        decoder::decode(bytes, &source, pixels, check)?
    };
    if image.format != ImageFormat::Png
        || (image.width, image.height) != (source.width, source.height)
    {
        return Err(codec("Retained PNG geometry differs from inspected source"));
    }
    // Capture succeeded. Profile bytes are retained, not their original PNG
    // keyword/framing; unrelated actual omissions remain unchanged.
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
