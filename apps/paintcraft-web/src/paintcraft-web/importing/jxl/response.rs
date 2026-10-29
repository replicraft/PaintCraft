//! Independent Rust binding checks after the JavaScript protocol validator.
use super::*;
use paintcraft_core::types::resources::SharedBudget;
pub(super) fn validate<'a>(
    source: &Source,
    owner: wire::Owner,
    nonce: Uuid,
    header: &[u8],
    response: &'a [u8],
    digest: &[u8],
) -> Result<(wire::Header, wire::Frame<'a>)> {
    let request = wire::Header::decode(header)?;
    if !matches!(
        request.codec,
        wire::Codec::JxlRgba8 | wire::Codec::JxlIntegerRgba8
    ) || request.owner != owner
        || request.nonce != nonce
        || request.length != source.length
        || u64::from(request.pixels) != wire::PIXELS_MAX
        || digest != request.sha256
    {
        return Err(Error::new(
            ErrorCode::ConflictStaleBase,
            "JPEG XL response is not bound to the captured source and owner",
        ));
    }
    let frame = wire::decode_with_header(response, &request, owner)?;
    if !matches!(frame.pixels, wire::Pixels::SrgbRgba8(_)) {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Browser document requires straight sRGB RGBA8",
        ));
    }
    if frame.heap_peak != 0 {
        return Err(Error::new(
            ErrorCode::CodecFailed,
            "Browser Worker claims native heap accounting",
        ));
    }
    Ok((request, frame))
}
pub(super) fn assemble(
    request: wire::Header,
    frame: wire::Frame<'_>,
    residency: SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Document> {
    let wire::Pixels::SrgbRgba8(rgba) = frame.pixels else {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Browser document requires straight sRGB RGBA8",
        ));
    };
    let mut admission = paintcraft_raster::Admission::with_residency(
        paintcraft_core::types::policy::STAGED_PIXELS_MAX,
        residency,
    );
    paintcraft_document::import_image_with_admission_checked(
        frame.width,
        frame.height,
        rgba,
        &paintcraft_document::hex(&request.sha256),
        &mut admission,
        check,
    )
}
#[cfg(test)]
pub(super) fn document(
    source: &Source,
    owner: wire::Owner,
    nonce: Uuid,
    header: &[u8],
    response: &[u8],
    digest: &[u8],
    residency: SharedBudget,
) -> Result<Document> {
    let (request, frame) = validate(source, owner, nonce, header, response, digest)?;
    assemble(request, frame, residency, &|| Ok(()))
}
