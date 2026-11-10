//! Bounded display-cell patches; original native-frame APIs stay unchanged.
use super::*;
use paintcraft_core::types::Rect;
/// `patch` is a half-open rectangle in the WHOLE visible request's output cells.
/// It never changes the global sample-grid anchor and may be at most64x64 cells.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source8DisplayPatch {
    pub visible: Rect,
    pub level: u8,
    pub patch: Rect,
}
// Private original-role grouping only. It is not an admission or ancestry token.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct OriginalControl8<'a> {
    pub(super) scope: &'a BudgetScope,
    pub(super) capacity: u64,
}
// Complete-slot movement/read-pair envelope, not CPU instructions or physical RSS.
// Four slots bound caller formation, argument transport, callee working and
// destructured outputs; the two pairs bound input/destructure field reads.
pub(super) const ORIGINAL_CONTROL_MOVE_WORK: u64 =
    (4 * std::mem::size_of::<OriginalControl8<'static>>()
        + 2 * (std::mem::size_of::<&BudgetScope>() + std::mem::size_of::<u64>())) as u64;
pub fn render_portable_source8_display_checked(
    bytes: &[u8],
    request: Source8InspectionRequest,
    realms: Source8ReadRealms<'_>,
    frame: &Source8FrameRealms<'_>,
    display: Source8DisplayPatch,
    check: &dyn Fn() -> Result<()>,
) -> Result<Source8RgbaImage> {
    let mut entry = entry::Entry8::new(request, realms, check)?;
    entry.read_into_slot_checked(bytes)?;
    entry.render_display_checked(frame, display)
}
