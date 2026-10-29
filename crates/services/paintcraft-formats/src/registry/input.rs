//! Signature and suffix dispatch; selected conversions explicitly authorize preflight.
use super::*;

pub(super) fn admit_input(
    frontend: Frontend,
    filename: &str,
    bytes: &[u8],
    platform: Platform,
    operation: Operation,
    variant: Option<&str>,
) -> Result<Family> {
    admit(
        frontend, filename, bytes, platform, operation, variant, false,
    )
}

pub(super) fn admit_input_with_preflight(
    frontend: Frontend,
    filename: &str,
    bytes: &[u8],
    platform: Platform,
    operation: Operation,
    variant: Option<&str>,
) -> Result<Family> {
    admit(
        frontend, filename, bytes, platform, operation, variant, true,
    )
}

fn admit(
    frontend: Frontend,
    filename: &str,
    bytes: &[u8],
    platform: Platform,
    operation: Operation,
    variant: Option<&str>,
    allow_preflight: bool,
) -> Result<Family> {
    let extension = filename
        .rsplit(['/', '\\'])
        .next()
        .and_then(|name| name.rsplit_once('.').map(|(_, ext)| ext));
    let hint = extension.and_then(Family::from_extension);
    if let Some(family) = hint {
        let request = Request::new(frontend, operation, family, platform);
        // Refuse foreign projects/vectors before content acquisition or codec fallback.
        if let Some(decision) = policy::deny(request) {
            decision.require_allowed(request)?;
        }
    }
    // MIF1 is shared with HEIF. A .paint suffix chooses a conversion candidate,
    // never proves layer semantics; the strict item/pixel decoder still must run.
    let paint_candidate = operation == Operation::ImportConvert
        && hint == Some(Family::Paint)
        && bytes.len() >= 16
        && bytes.get(4..12) == Some(b"ftypmif1");
    let sniffed = if paint_candidate {
        Some(Family::Paint)
    } else {
        sniff(bytes)
    }
    .ok_or_else(|| Reason::InvalidSignature.error(hint, frontend, operation))?;
    let mut request = Request::new(frontend, operation, sniffed, platform);
    request.extension = extension;
    request.variant = variant;
    let decision = admission(request);
    if !(allow_preflight
        && decision == Decision::AllowedWithPreflight(Reason::ConvertRequiresConsent))
    {
        decision.require_allowed(request)?;
    }
    Ok(sniffed)
}

/// Recognize signatures without invoking a decoder or trusting an extension.
pub fn sniff(bytes: &[u8]) -> Option<Family> {
    if crate::ora::has_signature(bytes) {
        Some(Family::Ora)
    } else if bytes.len() >= 8 && bytes.starts_with(b"PCND") && bytes[5..8] == [13, 10, 26] {
        Some(Family::Pcnd)
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Family::Png)
    } else if bytes.starts_with(&[255, 216, 255]) {
        Some(Family::Jpeg)
    } else if bytes.starts_with(b"BM") {
        Some(Family::Bmp)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(Family::Gif)
    } else if bytes.starts_with(b"II\x2a\0") || bytes.starts_with(b"MM\0\x2a") {
        Some(Family::Tiff)
    } else if bytes.starts_with(b"PDN3") {
        Some(Family::Pdn)
    } else if bytes.starts_with(b"8BPS\0\x01") {
        Some(Family::Psd)
    } else if bytes.starts_with(b"8BPS\0\x02") {
        Some(Family::Psb)
    } else if crate::heif::has_signature(bytes) {
        Some(Family::Heif)
    } else if crate::jxr::sniff(bytes) {
        Some(Family::Jxr)
    } else if bytes.starts_with(b"DDS ") {
        Some(Family::Dds)
    } else if bytes.len() >= 8 && bytes.starts_with(b"icns") {
        Some(Family::Icns)
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        Some(Family::Ico)
    } else if bytes.starts_with(&[0, 0, 2, 0]) {
        Some(Family::Cur)
    } else if bytes.starts_with(&987_654_321u32.to_le_bytes()) {
        Some(Family::Dcx)
    } else if bytes.starts_with(b"DanM") || bytes.starts_with(b"LinS") {
        Some(Family::Msp)
    } else if bytes.starts_with(b"qoif") {
        Some(Family::Qoi)
    } else if bytes.starts_with(b"farbfeld") {
        Some(Family::Farbfeld)
    } else if matches!(bytes.get(..2), Some([b'P', b'1'..=b'7']))
        && bytes.get(2).is_some_and(u8::is_ascii_whitespace)
    {
        Some(Family::Pnm)
    } else if bytes.len() >= 128
        && bytes[0] == 0x0a
        && bytes[2] == 1
        && matches!(bytes[3], 1 | 2 | 4 | 8)
    {
        Some(Family::Pcx)
    } else if bytes.starts_with(b"%PDF-") {
        Some(Family::Pdf)
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some(Family::Webp)
    } else if crate::image::ImageFormat::sniff(bytes) == Some(crate::image::ImageFormat::Tga) {
        Some(Family::Tga)
    } else {
        None
    }
}
