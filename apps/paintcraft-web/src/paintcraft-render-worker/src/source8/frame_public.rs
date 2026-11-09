//! Synchronous, real read-to-pixel consumer. Old inspection/routes stay exact.
use super::*;
use paintcraft_core::types::Rect;
/// Decode one private Source8, render a canvas-contained ROI and return immutable
/// accounted straight RGBA. Source ownership and prepared jobs stay private.
/// This finite CPU gate preserves inherited prepared-bookkeeping exclusions;
/// it does not activate a browser endpoint or claim complete physical D1.
pub fn render_portable_source8_checked(
    bytes: &[u8],
    request: Source8InspectionRequest,
    realms: Source8ReadRealms<'_>,
    frame: &Source8FrameRealms<'_>,
    region: Rect,
    check: &dyn Fn() -> Result<()>,
) -> Result<Source8RgbaImage> {
    let mut entry = entry::Entry8::new(request, realms, check)?;
    entry.read_into_slot_checked(bytes)?;
    entry.render_closed_checked(frame, region)
}
