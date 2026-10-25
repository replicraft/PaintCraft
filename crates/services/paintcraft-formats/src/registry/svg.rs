use super::*;

/// A separate route for explicit size/DPI rasterization. Generic import and
/// stock Open never choose SVG parameters or invoke a vector renderer.
pub fn admit_import_svg(
    frontend: Frontend,
    filename: &str,
    bytes: &[u8],
    platform: Platform,
) -> Result<Family> {
    let mut request = Request::new(frontend, Operation::ImportConvert, Family::Svg, platform);
    request.extension = std::path::Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str());
    request.variant = Some(crate::svg::IMPORT_VARIANT);
    let decision = admission(request);
    if !decision.available() {
        decision.require_allowed(request)?;
    }
    let compressed = bytes.starts_with(&[31, 139]);
    if request
        .extension
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svgz") != compressed)
    {
        return Err(Error::new(
            ErrorCode::CodecFailed,
            "SVG/SVGZ filename disagrees with the source envelope",
        ));
    }
    crate::svg::inspect_source(bytes, &|| Ok(()))?;
    Ok(Family::Svg)
}
