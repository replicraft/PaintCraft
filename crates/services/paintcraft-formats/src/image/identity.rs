//! Conversion identities are separate from raw file hashes and source authority.
use super::{Decoded, ImageFormat, jpeg};
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::{RasterConversionContext, RasterConversionKey, RasterSelection};

/// JPEG display orientation and device-color interpretation are materialized
/// before constructing a document. A policy change must use a new profile.
pub const JPEG_IDENTITY_PROFILE: &str = jpeg::IDENTITY_PROFILE;

/// Bind normalized JPEG pixels and their conversion policy to document IDs.
/// Other formats retain their existing construction keys. This does not grant
/// admission, metadata retention, loss consent or filesystem authority.
pub fn document_conversion_key(
    bytes: &[u8],
    decoded: &Decoded,
    check: &dyn Fn() -> Result<()>,
) -> Result<Option<RasterConversionKey>> {
    if decoded.format != ImageFormat::Jpeg {
        return Ok(None);
    }
    let source = jpeg::inspect(bytes, super::DECODE_PIXELS_MAX, check)?;
    if source.oriented_dimensions() != (decoded.width, decoded.height) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "JPEG converted document geometry differs from normalized source",
        ));
    }
    RasterConversionKey::checked(
        RasterConversionContext {
            source_sha256: source.source_sha256(),
            profile: JPEG_IDENTITY_PROFILE,
            selection: RasterSelection::Image,
            assessment_sha256: None,
        },
        decoded.width,
        decoded.height,
        &decoded.rgba,
        super::DECODE_PIXELS_MAX,
        check,
    )
    .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    const SOURCE: &[u8] = include_bytes!("../../tests/jpeg-fixtures/rgb-orientation-6-II.jpg");
    const PIXELS: &[u8] = include_bytes!("../../tests/jpeg-fixtures/rgb-orientation-6-II.rgba");
    fn converted() -> Decoded {
        Decoded {
            format: ImageFormat::Jpeg,
            width: 16,
            height: 24,
            rgba: PIXELS.to_vec(),
            losses: vec!["exif-metadata-ignored".into()],
        }
    }
    #[test]
    fn normalized_jpeg_keys_bind_samples_and_differ_from_legacy_raw_source_identity() {
        let mut decoded = converted();
        let key = document_conversion_key(SOURCE, &decoded, &|| Ok(()))
            .unwrap()
            .unwrap();
        assert_eq!(
            Some(key),
            document_conversion_key(SOURCE, &decoded, &|| Ok(())).unwrap()
        );
        let raw = paintcraft_core::types::policy::hex(&Sha256::digest(SOURCE));
        let legacy = paintcraft_document::import_image(16, 24, PIXELS, &raw).unwrap();
        let normalized = paintcraft_document::import_image(16, 24, PIXELS, &key.hex()).unwrap();
        assert_ne!(legacy.document_id, normalized.document_id);
        decoded.rgba[0] ^= 1;
        assert_ne!(
            Some(key),
            document_conversion_key(SOURCE, &decoded, &|| Ok(())).unwrap()
        );
        (decoded.width, decoded.height) = (24, 16);
        assert_eq!(
            document_conversion_key(SOURCE, &decoded, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
}
