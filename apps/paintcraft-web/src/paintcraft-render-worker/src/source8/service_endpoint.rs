//! Crate-only facade for the real compiled dispatcher. No root/TLS/fallback here.
use super::*;
use paintcraft_core::types::resources::BudgetScope;
use service_bridge::{ServiceOriginals8, ServiceSource8, ServiceView8};
#[repr(transparent)]
pub(crate) struct LoadedSource8(ServiceSource8);
#[repr(transparent)]
pub(crate) struct LoadedView8(ServiceView8);
#[repr(C)]
pub(crate) struct Expected8 {
    pub source_sha: [u8; 32],
    pub document: [u8; 16],
    pub state: [u8; 16],
    pub canvas: [u32; 2],
    pub stored_noise: bool,
}
#[repr(C)]
pub(crate) struct Originals8<'a> {
    pub encoded: &'a BudgetScope,
    pub control: &'a BudgetScope,
    pub encoded_capacity: u64,
    pub control_capacity: u64,
}
impl LoadedSource8 {
    /// Already inside ONLY the accepted dispatch job; Entry captures actual Some once.
    pub(crate) fn open_checked(
        bytes: &[u8],
        request: Source8InspectionRequest,
        realms: Source8ReadRealms<'_>,
        expected: &Expected8,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<Self> {
        spend(8192)?;
        let mut entry = entry::Entry8::new(request, realms, check)?;
        entry.read_into_slot_checked(bytes)?;
        entry.service_match_checked(expected)?;
        Ok(Self(entry.take_service_source_checked()?))
    }
    pub(crate) fn begin_checked(
        &self,
        request: &Source8InspectionRequest,
        originals: &Originals8<'_>,
        frame: &Source8FrameRealms<'_>,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<LoadedView8> {
        spend(8192)?;
        let required = caller::RequiredCaller8::capture(request)?;
        let originals = ServiceOriginals8 {
            encoded: originals.encoded,
            control: originals.control,
            encoded_capacity: originals.encoded_capacity,
            control_capacity: originals.control_capacity,
        };
        Ok(LoadedView8(
            self.0
                .begin_view_checked(&required, &originals, frame, check)?,
        ))
    }
}
impl LoadedView8 {
    pub(crate) fn render_checked(
        &mut self,
        source: &LoadedSource8,
        frame: &Source8FrameRealms<'_>,
        display: &Source8DisplayPatch,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Source8RgbaImage> {
        self.0
            .render_patch_checked(&source.0, frame, display, check)
    }
}
