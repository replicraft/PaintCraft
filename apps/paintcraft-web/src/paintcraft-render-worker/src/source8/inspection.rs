use super::*;
/// Descriptive scalar output, never source/adoption/publication authority.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source8Inspection {
    pub(super) source_sha256: [u8; 32],
    pub(super) document_id: [u8; 16],
    pub(super) state_id: [u8; 16],
    pub(super) canvas: [u32; 2],
    pub(super) layers: u64,
    pub(super) tiles: u64,
    pub(super) filters: u64,
    pub(super) masks_present: u64,
    pub(super) masks_nonempty: u64,
    pub(super) bands: u64,
    pub(super) runs: u64,
    pub(super) metadata_kind: u64,
    pub(super) profile_bytes: u64,
}
impl Source8Inspection {
    pub fn source_sha256(&self) -> [u8; 32] {
        self.source_sha256
    }
    pub fn document_id(&self) -> [u8; 16] {
        self.document_id
    }
    pub fn state_id(&self) -> [u8; 16] {
        self.state_id
    }
    pub fn canvas(&self) -> [u32; 2] {
        self.canvas
    }
    pub fn layers(&self) -> u64 {
        self.layers
    }
    pub fn tiles(&self) -> u64 {
        self.tiles
    }
    pub fn filters(&self) -> u64 {
        self.filters
    }
    pub fn masks_present(&self) -> u64 {
        self.masks_present
    }
    pub fn masks_nonempty(&self) -> u64 {
        self.masks_nonempty
    }
    pub fn bands(&self) -> u64 {
        self.bands
    }
    pub fn runs(&self) -> u64 {
        self.runs
    }
    /// 0 absent; 1 metadata without profile; 2 metadata with profile.
    pub fn metadata_kind(&self) -> u64 {
        self.metadata_kind
    }
    pub fn profile_bytes(&self) -> u64 {
        self.profile_bytes
    }
}
/// Load, retain-adopt, inspect scalars, and retire a closed portable Source8.
///
/// The Rust host supplies its original attempt factories and an actual installed
/// Render caller. No default realm, job root, TLS install or JS route is created.
/// Plan-local work excludes authentication/ownership/assembly/adoption; all use
/// the SAME original cumulative caller and can exhaust its tighter ceiling.
///
/// Callback Errors are moved unchanged. Arbitrary/inherited/escaped diagnostic
/// heap and bootstrap stack are not fully funded by the static error shadow.
/// This is not a general PCND8/Zstd import, Worker8 activation or full-heap claim.
/// ```compile_fail
/// use paintcraft_web_render_worker::source8::Source8Inspection;
/// fn escape(value: Source8Inspection) { let _ = value.document(); }
/// ```
pub fn inspect_portable_source8_checked(
    bytes: &[u8],
    request: Source8InspectionRequest,
    realms: Source8ReadRealms<'_>,
    raw_check: &dyn Fn() -> Result<()>,
) -> Result<Source8Inspection> {
    let mut entry = entry::Entry8::new(request, realms, raw_check)?;
    entry.read_into_slot_checked(bytes)?;
    let result = entry.inspect_checked()?;
    drop(entry);
    Ok(result)
}
